//! Canevas vectoriel : zoom, déplacement, saisie du stylo et rendu des traits.

use calame_core::{Brush, BrushKind, History, Page, Point2, Stroke, StrokePoint};
use egui::{epaint::Vertex, Color32, Mesh, Pos2, Rect, Sense, Shape, Vec2};

/// Vue sur la page infinie : `offset` est le point de page affiché au coin haut-gauche.
#[derive(Clone, Copy)]
pub struct View {
    pub offset: Vec2,
    pub zoom: f32,
}

impl Default for View {
    fn default() -> Self {
        Self {
            offset: Vec2::ZERO,
            zoom: 1.0,
        }
    }
}

impl View {
    fn to_screen(self, origin: Pos2, p: Point2) -> Pos2 {
        origin + (Vec2::new(p.x, p.y) - self.offset) * self.zoom
    }

    fn to_page(self, origin: Pos2, s: Pos2) -> Point2 {
        let v = (s - origin) / self.zoom + self.offset;
        Point2::new(v.x, v.y)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    Mouse,
    Touch,
}

struct Drawing {
    stroke: Stroke,
    source: Source,
}

#[derive(Default)]
pub struct Canvas {
    pub view: View,
    drawing: Option<Drawing>,
}

const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 8.0;

impl Canvas {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        page: &mut Page,
        history: &mut History,
        brush: Brush,
    ) {
        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        let origin = rect.min;

        self.navigate(ui, &response, origin);
        self.capture(ui, &response, rect, page, history, brush);

        painter.rect_filled(rect, 0.0, Color32::WHITE);
        draw_grid(&painter, rect, self.view);
        for stroke in &page.strokes {
            paint_stroke(&painter, stroke, &stroke.smoothed(6), self.view, origin);
        }
        if let Some(d) = &self.drawing {
            paint_stroke(
                &painter,
                &d.stroke,
                &d.stroke.smoothed(6),
                self.view,
                origin,
            );
        }
        if self.drawing.is_some() {
            ui.ctx().request_repaint();
        }
    }

    /// Zoom (Ctrl + molette ou pincement), défilement (molette), déplacement (bouton du milieu).
    fn navigate(&mut self, ui: &egui::Ui, response: &egui::Response, origin: Pos2) {
        if response.hovered() {
            let (zoom_delta, scroll) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta));
            if (zoom_delta - 1.0).abs() > f32::EPSILON {
                if let Some(cursor) = response.hover_pos() {
                    let before = self.view.to_page(origin, cursor);
                    self.view.zoom = (self.view.zoom * zoom_delta).clamp(MIN_ZOOM, MAX_ZOOM);
                    let after = self.view.to_page(origin, cursor);
                    self.view.offset += Vec2::new(before.x - after.x, before.y - after.y);
                }
            } else if scroll != Vec2::ZERO {
                self.view.offset -= scroll / self.view.zoom;
            }
        }
        if response.dragged_by(egui::PointerButton::Middle) {
            self.view.offset -= response.drag_delta() / self.view.zoom;
        }
    }

    /// Transforme les événements pointeur/tactile en tracé. Le tactile transporte la pression
    /// du stylet ; la souris utilise une pression constante.
    fn capture(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: Rect,
        page: &mut Page,
        history: &mut History,
        brush: Brush,
    ) {
        let events = ui.input(|i| i.events.clone());
        let origin = rect.min;
        for event in events {
            match event {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    ..
                } => {
                    if pressed && response.contains_pointer() && self.drawing.is_none() {
                        self.begin(Source::Mouse, pos, 0.5, brush, origin);
                    } else if !pressed {
                        self.finish_if(Source::Mouse, page, history);
                    }
                }
                egui::Event::PointerMoved(pos) => self.extend(Source::Mouse, pos, 0.5, origin),
                egui::Event::Touch {
                    phase, pos, force, ..
                } => {
                    let pressure = force.map_or(0.5, |f| f.clamp(0.0, 1.0));
                    match phase {
                        egui::TouchPhase::Start => {
                            if rect.contains(pos) {
                                // Une souris émulée par le tactile peut avoir démarré le trait
                                // juste avant : le tactile, plus riche, prend le relais.
                                self.drawing = None;
                                self.begin(Source::Touch, pos, pressure, brush, origin);
                            }
                        }
                        egui::TouchPhase::Move => self.extend(Source::Touch, pos, pressure, origin),
                        egui::TouchPhase::End | egui::TouchPhase::Cancel => {
                            self.extend(Source::Touch, pos, pressure, origin);
                            self.finish_if(Source::Touch, page, history);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn begin(&mut self, source: Source, pos: Pos2, pressure: f32, brush: Brush, origin: Pos2) {
        let mut stroke = Stroke::new(brush);
        stroke.push(StrokePoint {
            pos: self.view.to_page(origin, pos),
            pressure,
            tilt: None,
        });
        self.drawing = Some(Drawing { stroke, source });
    }

    fn extend(&mut self, source: Source, pos: Pos2, pressure: f32, origin: Pos2) {
        let view = self.view;
        let Some(d) = self.drawing.as_mut().filter(|d| d.source == source) else {
            return;
        };
        let p = view.to_page(origin, pos);
        // Ignore les points quasi identiques pour garder des tracés légers.
        if d.stroke
            .points
            .last()
            .is_some_and(|last| last.pos.distance(p) < 0.5 / view.zoom)
        {
            return;
        }
        d.stroke.push(StrokePoint {
            pos: p,
            pressure,
            tilt: None,
        });
    }

    fn finish_if(&mut self, source: Source, page: &mut Page, history: &mut History) {
        if self.drawing.as_ref().is_some_and(|d| d.source == source) {
            if let Some(d) = self.drawing.take() {
                history.add_stroke(page, d.stroke);
            }
        }
    }
}

fn draw_grid(painter: &egui::Painter, rect: Rect, view: View) {
    let step = 32.0;
    let color = Color32::from_gray(230);
    let first_x = (view.offset.x / step).floor() * step;
    let first_y = (view.offset.y / step).floor() * step;
    let mut x = first_x;
    while (x - view.offset.x) * view.zoom < rect.width() {
        let sx = rect.min.x + (x - view.offset.x) * view.zoom;
        painter.line_segment(
            [Pos2::new(sx, rect.min.y), Pos2::new(sx, rect.max.y)],
            (1.0, color),
        );
        x += step;
    }
    let mut y = first_y;
    while (y - view.offset.y) * view.zoom < rect.height() {
        let sy = rect.min.y + (y - view.offset.y) * view.zoom;
        painter.line_segment(
            [Pos2::new(rect.min.x, sy), Pos2::new(rect.max.x, sy)],
            (1.0, color),
        );
        y += step;
    }
}

/// Dessine un trait comme une bande de triangles d'épaisseur variable, avec des extrémités rondes.
fn paint_stroke(
    painter: &egui::Painter,
    stroke: &Stroke,
    points: &[StrokePoint],
    view: View,
    origin: Pos2,
) {
    let c = stroke.brush.color;
    let color = Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a);
    let screen: Vec<(Pos2, f32)> = points
        .iter()
        .map(|p| {
            (
                view.to_screen(origin, p.pos),
                (stroke.width_at(p) * view.zoom).max(0.5) * 0.5,
            )
        })
        .collect();
    let Some(&(first, first_r)) = screen.first() else {
        return;
    };
    if screen.len() == 1 {
        painter.circle_filled(first, first_r, color);
        return;
    }
    // Un surligneur translucide ne doit pas s'assombrir là où les cercles se recouvrent :
    // on ne met donc des extrémités rondes que pour le stylo.
    let round_caps = stroke.brush.kind == BrushKind::Pen;
    let mut mesh = Mesh::default();
    let uv = egui::epaint::WHITE_UV;
    for (i, &(p, r)) in screen.iter().enumerate() {
        let prev = screen[i.saturating_sub(1)].0;
        let next = screen[(i + 1).min(screen.len() - 1)].0;
        let dir = (next - prev).normalized();
        let normal = Vec2::new(-dir.y, dir.x) * r;
        mesh.vertices.push(Vertex {
            pos: p + normal,
            uv,
            color,
        });
        mesh.vertices.push(Vertex {
            pos: p - normal,
            uv,
            color,
        });
        if i > 0 {
            let b = (i * 2) as u32;
            mesh.indices
                .extend_from_slice(&[b - 2, b - 1, b, b - 1, b + 1, b]);
        }
    }
    painter.add(Shape::mesh(mesh));
    if round_caps {
        painter.circle_filled(first, first_r, color);
        if let Some(&(last, last_r)) = screen.last() {
            painter.circle_filled(last, last_r, color);
        }
    }
}
