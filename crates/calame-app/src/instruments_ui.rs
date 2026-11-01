//! Dessin des instruments de géométrie : corps translucide, graduations et poignées.

use std::f32::consts::PI;

use calame_core::{Handle, Instrument, Point2, Pose, UNITS_PER_CM, UNITS_PER_MM};
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, Vec2};

const FILL: Color32 = Color32::from_rgba_premultiplied(24, 35, 55, 55);
const BORDER: Color32 = Color32::from_rgb(60, 110, 200);
const INK: Color32 = Color32::from_rgb(25, 55, 115);

/// Convertit un point de page en position d'écran.
pub type ToScreen<'a> = &'a dyn Fn(Point2) -> Pos2;

pub fn paint(
    painter: &Painter,
    instruments: &[Instrument],
    active: Option<usize>,
    show_handles: bool,
    to_screen: ToScreen<'_>,
    zoom: f32,
) {
    for (index, instrument) in instruments.iter().enumerate() {
        match instrument {
            Instrument::Ruler { pose, length, .. } => {
                body(painter, instrument, to_screen);
                scale(painter, to_screen, zoom, *pose, *length, false, true);
            }
            Instrument::SetSquare { pose, leg_x, leg_y } => {
                body(painter, instrument, to_screen);
                scale(painter, to_screen, zoom, *pose, *leg_x, false, true);
                scale(painter, to_screen, zoom, *pose, *leg_y, true, false);
            }
            Instrument::Protractor { pose, radius } => {
                body(painter, instrument, to_screen);
                protractor_marks(painter, to_screen, zoom, *pose, *radius);
            }
            Instrument::Compass { pivot, pencil } => {
                compass(painter, to_screen, zoom, *pivot, *pencil)
            }
        }
        if show_handles && active == Some(index) {
            for (handle, position) in instrument.handles() {
                draw_handle(painter, handle, to_screen(position));
            }
        }
    }
}

fn body(painter: &Painter, instrument: &Instrument, to_screen: ToScreen<'_>) {
    let points: Vec<Pos2> = instrument.outline().into_iter().map(to_screen).collect();
    painter.add(Shape::convex_polygon(
        points,
        FILL,
        Stroke::new(1.5, BORDER),
    ));
}

/// Graduations en millimètres le long d'un côté de longueur `length`.
///
/// `vertical` : le côté suit l'axe y local plutôt que l'axe x. Les traits entrent dans le corps de
/// l'instrument. Les graduations s'espacent quand on dézoome pour rester lisibles.
fn scale(
    painter: &Painter,
    to_screen: ToScreen<'_>,
    zoom: f32,
    pose: Pose,
    length: f32,
    vertical: bool,
    labels: bool,
) {
    let px_per_mm = UNITS_PER_MM * zoom;
    let mm_step = if px_per_mm >= 2.5 {
        1
    } else if px_per_mm * 5.0 >= 4.0 {
        5
    } else {
        10
    };
    let label_step = if px_per_mm * 10.0 >= 28.0 {
        1
    } else if px_per_mm * 50.0 >= 28.0 {
        5
    } else {
        10
    };
    let at = |along: f32, across: f32| {
        let local = if vertical {
            Point2::new(across, along)
        } else {
            Point2::new(along, across)
        };
        to_screen(pose.to_page(local))
    };
    let count = (length / UNITS_PER_MM).floor() as i32;
    for i in (0..=count).filter(|i| i % mm_step == 0) {
        let along = i as f32 * UNITS_PER_MM;
        let tick = match (i % 10 == 0, i % 5 == 0) {
            (true, _) => 14.0,
            (false, true) => 10.0,
            _ => 6.0,
        };
        painter.line_segment([at(along, 0.0), at(along, tick)], Stroke::new(1.0, INK));
        if labels && i % 10 == 0 && (i / 10) % label_step == 0 {
            painter.text(
                at(along, 24.0),
                Align2::CENTER_CENTER,
                (i / 10).to_string(),
                FontId::proportional(10.0),
                INK,
            );
        }
    }
}

/// Graduations en degrés du rapporteur : 0° à droite, 180° à gauche.
fn protractor_marks(
    painter: &Painter,
    to_screen: ToScreen<'_>,
    zoom: f32,
    pose: Pose,
    radius: f32,
) {
    let px_per_deg = radius * zoom * PI / 180.0;
    let step = if px_per_deg >= 3.0 {
        1
    } else if px_per_deg * 5.0 >= 3.0 {
        5
    } else {
        10
    };
    let at = |deg: f32, r: f32| {
        let a = deg * PI / 180.0;
        to_screen(pose.to_page(Point2::new(r * a.cos(), -r * a.sin())))
    };
    for deg in (0..=180).filter(|d| d % step == 0) {
        let tick = match (deg % 10 == 0, deg % 5 == 0) {
            (true, _) => 14.0,
            (false, true) => 10.0,
            _ => 6.0,
        };
        let d = deg as f32;
        painter.line_segment([at(d, radius), at(d, radius - tick)], Stroke::new(1.0, INK));
        if deg % 10 == 0 && radius * zoom >= 80.0 {
            painter.text(
                at(d, radius - 26.0),
                Align2::CENTER_CENTER,
                deg.to_string(),
                FontId::proportional(9.0),
                INK,
            );
        }
    }
    // Centre : petite croix pour viser le sommet de l'angle à mesurer.
    let c = to_screen(pose.origin);
    painter.line_segment(
        [c - Vec2::new(6.0, 0.0), c + Vec2::new(6.0, 0.0)],
        Stroke::new(1.0, INK),
    );
    painter.line_segment(
        [c - Vec2::new(0.0, 6.0), c + Vec2::new(0.0, 6.0)],
        Stroke::new(1.0, INK),
    );
}

fn compass(painter: &Painter, to_screen: ToScreen<'_>, zoom: f32, pivot: Point2, pencil: Point2) {
    let radius = pivot.distance(pencil);
    let (p, q) = (to_screen(pivot), to_screen(pencil));
    // Cercle que le compas tracerait, en filigrane.
    painter.circle_stroke(
        p,
        radius * zoom,
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(60, 110, 200, 80)),
    );
    // Charnière placée perpendiculairement au milieu de l'écartement, pour dessiner un « V ».
    let dir = unit(pivot, pencil);
    let hinge_page = Point2::new(
        (pivot.x + pencil.x) / 2.0 - dir.1 * radius * 0.8,
        (pivot.y + pencil.y) / 2.0 + dir.0 * radius * 0.8,
    );
    let h = to_screen(hinge_page);
    let leg = Stroke::new(3.0, Color32::from_rgb(80, 80, 95));
    painter.line_segment([p, h], leg);
    painter.line_segment([h, q], leg);
    painter.circle_filled(h, 5.0, Color32::from_rgb(120, 120, 135));
    painter.circle_filled(p, 3.0, INK);
    painter.circle_filled(q, 4.0, Color32::from_rgb(200, 60, 50));
    painter.text(
        q + Vec2::new(10.0, -10.0),
        Align2::LEFT_BOTTOM,
        format!("{:.1} cm", radius / UNITS_PER_CM).replace('.', ","),
        FontId::proportional(11.0),
        INK,
    );
}

/// Vecteur unitaire de `from` vers `to` ; `(1, 0)` si les deux points sont confondus.
fn unit(from: Point2, to: Point2) -> (f32, f32) {
    let d = from.distance(to);
    if d <= f32::EPSILON {
        (1.0, 0.0)
    } else {
        ((to.x - from.x) / d, (to.y - from.y) / d)
    }
}

fn draw_handle(painter: &Painter, handle: Handle, at: Pos2) {
    let outline = Stroke::new(1.5, Color32::WHITE);
    match handle {
        Handle::Rotate => {
            painter.circle_filled(at, 8.0, Color32::from_rgb(240, 150, 40));
            painter.circle_stroke(at, 8.0, outline);
        }
        Handle::Resize => {
            let r = Rect::from_center_size(at, Vec2::splat(13.0));
            painter.rect_filled(r, 2.0, Color32::from_rgb(60, 170, 90));
            painter.rect_stroke(r, 2.0, outline, egui::StrokeKind::Middle);
        }
        Handle::Pivot => {
            painter.circle_filled(at, 8.0, Color32::from_rgb(60, 110, 200));
            painter.circle_stroke(at, 8.0, outline);
        }
        Handle::Pencil => {
            painter.circle_filled(at, 8.0, Color32::from_rgb(200, 60, 50));
            painter.circle_stroke(at, 8.0, outline);
        }
    }
}
