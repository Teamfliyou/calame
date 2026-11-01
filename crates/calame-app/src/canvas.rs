//! Canevas vectoriel : zoom, déplacement, outils de dessin, gommes, lasso, texte et pipette.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use calame_core::{
    snap, Background, BrushKind, Color, Edge, Handle, History, Instrument, InstrumentKind, Item,
    Page, Point2, Rect2, Similarity, Stroke, StrokePoint, TextBox, UNITS_PER_MM,
};
use calame_shapes::{recognize_stroke, Recognized, ShapeKind};
use egui::epaint::{TextShape, Vertex};
use egui::{Color32, FontId, Mesh, Painter, Pos2, Rect, Sense, Shape, Vec2};
use uuid::Uuid;

use crate::instruments_ui;
use crate::render_cache::{RenderCache, SMOOTHING};
use crate::tools::{Tool, ToolSettings};

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

enum TransformOp {
    Move { start: Point2 },
    Scale { pivot: Point2, start_dist: f32 },
    Rotate { pivot: Point2, start_angle: f32 },
}

enum InstrumentOp {
    Move { last: Point2 },
    Handle(Handle),
}

enum Gesture {
    None,
    Draw {
        stroke: Stroke,
        /// Bord d'instrument sur lequel le trait est guidé, s'il a commencé à son contact.
        lock: Option<Edge>,
        /// Dernier point où le stylet a réellement bougé, et quand : sert à détecter le maintien.
        anchor: Point2,
        last_move: Instant,
        /// Forme reconnue pendant un maintien immobile, qui remplacera le trait au relâchement.
        snapped: Option<Recognized>,
        source: Source,
    },
    Instrument {
        index: usize,
        op: InstrumentOp,
        source: Source,
    },
    EraseStroke {
        hits: HashSet<Uuid>,
        last: Point2,
        source: Source,
    },
    EraseArea {
        /// Pour chaque trait touché (clé : identifiant d'origine), les morceaux restants.
        pieces: HashMap<Uuid, Vec<Stroke>>,
        last: Point2,
        source: Source,
    },
    Lasso {
        points: Vec<Point2>,
        source: Source,
    },
    Transform {
        op: TransformOp,
        current: Similarity,
        source: Source,
    },
}

impl Gesture {
    fn source(&self) -> Option<Source> {
        match self {
            Self::None => None,
            Self::Draw { source, .. }
            | Self::EraseStroke { source, .. }
            | Self::EraseArea { source, .. }
            | Self::Lasso { source, .. }
            | Self::Instrument { source, .. }
            | Self::Transform { source, .. } => Some(*source),
        }
    }
}

/// Zone de texte en cours de saisie. Texte vide à la validation : l'élément disparaît.
struct TextEdit {
    /// `Some` si l'on modifie un texte existant.
    id: Option<Uuid>,
    pos: Point2,
    text: String,
    font_size: f32,
    color: Color,
    angle: f32,
    /// Vrai avant la première image, pour donner le focus au champ.
    fresh: bool,
}

enum Event {
    Down(Source, Pos2, f32),
    Move(Source, Pos2, f32),
    Up(Source, Pos2),
}

/// Ce dont les gestes ont besoin en dehors du canevas.
struct Cx<'a> {
    page: &'a mut Page,
    history: &'a mut History,
    tools: &'a mut ToolSettings,
    painter: &'a Painter,
}

pub struct Canvas {
    pub view: View,
    gesture: Gesture,
    selection: HashSet<Uuid>,
    text_edit: Option<TextEdit>,
    last_tool: Tool,
    /// Règle, équerre, rapporteur et compas posés sur la page (non enregistrés avec elle).
    pub instruments: Vec<Instrument>,
    active_instrument: Option<usize>,
    /// Centre de la zone visible, en coordonnées de page : là où apparaissent les nouveaux instruments.
    viewport_center: Point2,
    /// Lissage et rectangles englobants des traits, calculés une fois (voir `render_cache`).
    render_cache: RenderCache,
    /// Dernière forme reconnue, affichée un instant pour indiquer comment l'annuler.
    notice: Option<(ShapeKind, Instant)>,
}

impl Default for Canvas {
    fn default() -> Self {
        Self {
            view: View::default(),
            gesture: Gesture::None,
            selection: HashSet::new(),
            text_edit: None,
            last_tool: Tool::Pen,
            instruments: Vec::new(),
            active_instrument: None,
            viewport_center: Point2::default(),
            render_cache: RenderCache::default(),
            notice: None,
        }
    }
}

const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 8.0;
/// Rayon de capture des poignées de la sélection, en pixels d'écran.
const HANDLE_RADIUS: f32 = 12.0;
const ROTATE_HANDLE_OFFSET: f32 = 28.0;
/// Décalage d'un collage par rapport à l'original, en unités de page.
const PASTE_OFFSET: f32 = 24.0;
/// Distance à partir de laquelle un trait commençant près d'un bord s'y accroche, en pixels d'écran.
const SNAP_PX: f32 = 16.0;
/// Durée d'immobilité du stylet, trait posé, qui déclenche la reconnaissance de forme.
const HOLD_DELAY: Duration = Duration::from_millis(600);
/// Déplacement toléré pendant le maintien, en pixels d'écran.
const HOLD_SLOP_PX: f32 = 3.0;
const NOTICE_DURATION: Duration = Duration::from_millis(2500);

fn color32(c: Color) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

impl Canvas {
    pub fn has_selection(&self) -> bool {
        !self.selection.is_empty()
    }

    /// Informe le canevas d'un changement d'outil voulu par l'application : la sélection est conservée.
    pub fn sync_tool(&mut self, tool: Tool) {
        self.last_tool = tool;
    }

    pub fn add_instrument(&mut self, kind: InstrumentKind) {
        self.instruments
            .push(Instrument::new(kind, self.viewport_center));
        self.active_instrument = Some(self.instruments.len() - 1);
    }

    pub fn remove_active_instrument(&mut self) {
        if let Some(i) = self.active_instrument.take() {
            if i < self.instruments.len() {
                self.instruments.remove(i);
            }
        }
    }

    pub fn clear_instruments(&mut self) {
        self.instruments.clear();
        self.active_instrument = None;
    }

    pub fn instrument_count(&self) -> usize {
        self.instruments.len()
    }

    pub fn has_active_instrument(&self) -> bool {
        self.active_instrument.is_some()
    }

    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }

    pub fn select_all(&mut self, page: &Page) {
        self.selection = page.items.iter().map(Item::id).collect();
    }

    /// Copie des éléments sélectionnés, dans l'ordre de la page.
    pub fn selected_items(&self, page: &Page) -> Vec<Item> {
        page.items
            .iter()
            .filter(|i| self.selection.contains(&i.id()))
            .cloned()
            .collect()
    }

    pub fn delete_selection(&mut self, page: &mut Page, history: &mut History) {
        let ids: Vec<Uuid> = self.selection.drain().collect();
        history.remove_items(page, &ids);
    }

    /// Applique `color` aux éléments sélectionnés (la transparence du surligneur est conservée).
    pub fn recolor_selection(&self, page: &mut Page, history: &mut History, color: Color) {
        let replacements = self
            .selected_items(page)
            .into_iter()
            .map(|mut item| {
                let id = item.id();
                item.set_color(color);
                (id, vec![item])
            })
            .collect();
        history.replace_items(page, replacements);
    }

    /// Colle des éléments copiés (avec de nouveaux identifiants) et les sélectionne.
    pub fn paste(&mut self, page: &mut Page, history: &mut History, items: &[Item]) {
        let shift = Similarity::translation(PASTE_OFFSET, PASTE_OFFSET);
        let pasted: Vec<Item> = items
            .iter()
            .map(|i| i.duplicated().transformed(&shift))
            .collect();
        self.selection = pasted.iter().map(Item::id).collect();
        history.add_items(page, pasted);
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        page: &mut Page,
        history: &mut History,
        tools: &mut ToolSettings,
        backdrop: &Backdrop,
    ) {
        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        let origin = rect.min;

        self.navigate(ui, &response, origin);
        self.viewport_center = self.view.to_page(origin, rect.center());

        let mut cx = Cx {
            page,
            history,
            tools,
            painter: &painter,
        };
        if cx.tools.tool != self.last_tool {
            self.last_tool = cx.tools.tool;
            self.commit_text(&mut cx);
            self.gesture = Gesture::None;
            self.selection.clear();
        }
        self.capture(ui, &response, rect, &mut cx);
        self.detect_hold();
        self.paint(ui, &response, rect, &cx, backdrop);
        self.text_editor(ui, origin, &mut cx);

        if !matches!(self.gesture, Gesture::None) {
            ui.ctx().request_repaint();
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(match cx.tools.tool {
                Tool::Text => egui::CursorIcon::Text,
                Tool::Lasso | Tool::Instruments => egui::CursorIcon::Default,
                Tool::EraserStroke | Tool::EraserArea => egui::CursorIcon::None,
                _ => egui::CursorIcon::Crosshair,
            });
        }
    }

    /// Reconnaît la forme d'un trait quand le stylet reste immobile, trait posé.
    fn detect_hold(&mut self) {
        let Gesture::Draw {
            stroke,
            lock,
            last_move,
            snapped,
            ..
        } = &mut self.gesture
        else {
            return;
        };
        if snapped.is_some() || lock.is_some() || last_move.elapsed() < HOLD_DELAY {
            return;
        }
        // Un maintien est une demande explicite : on accepte aussi le simple lissage.
        *snapped = recognize_stroke(stroke, true);
        match snapped {
            Some(shape) => self.notice = Some((shape.kind, Instant::now())),
            // Rien de reconnaissable : on réessaiera après un nouveau délai d'immobilité.
            None => *last_move = Instant::now(),
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

    // ---- Entrées ------------------------------------------------------------------------

    /// Transforme les événements pointeur/tactile en gestes. Le tactile transporte la pression
    /// du stylet ; la souris utilise une pression constante.
    fn capture(&mut self, ui: &egui::Ui, response: &egui::Response, rect: Rect, cx: &mut Cx<'_>) {
        let events = ui.input(|i| i.events.clone());
        for event in events {
            let ev = match event {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    ..
                } if response.contains_pointer() => Event::Down(Source::Mouse, pos, 0.5),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    ..
                } => Event::Up(Source::Mouse, pos),
                egui::Event::PointerMoved(pos) => Event::Move(Source::Mouse, pos, 0.5),
                egui::Event::Touch {
                    phase, pos, force, ..
                } => {
                    let pressure = force.map_or(0.5, |f| f.clamp(0.0, 1.0));
                    match phase {
                        egui::TouchPhase::Start if rect.contains(pos) => {
                            Event::Down(Source::Touch, pos, pressure)
                        }
                        egui::TouchPhase::Start => continue,
                        egui::TouchPhase::Move => Event::Move(Source::Touch, pos, pressure),
                        egui::TouchPhase::End | egui::TouchPhase::Cancel => {
                            Event::Up(Source::Touch, pos)
                        }
                    }
                }
                _ => continue,
            };
            self.handle(ev, rect.min, cx);
        }
    }

    fn handle(&mut self, event: Event, origin: Pos2, cx: &mut Cx<'_>) {
        match event {
            Event::Down(source, pos, pressure) => {
                if let Some(active) = self.gesture.source() {
                    // Une souris émulée par le tactile peut avoir démarré le geste juste avant :
                    // le tactile, plus riche (pression), prend le relais.
                    if active == Source::Mouse && source == Source::Touch {
                        self.gesture = Gesture::None;
                    } else {
                        return;
                    }
                }
                self.on_down(source, pos, pressure, origin, cx);
            }
            Event::Move(source, pos, pressure) => {
                if self.gesture.source() == Some(source) {
                    self.on_move(pos, pressure, origin, cx);
                }
            }
            Event::Up(source, pos) => {
                if self.gesture.source() == Some(source) {
                    self.on_move(pos, 0.5, origin, cx);
                    self.on_up(cx);
                }
            }
        }
    }

    fn on_down(&mut self, source: Source, pos: Pos2, pressure: f32, origin: Pos2, cx: &mut Cx<'_>) {
        // Toucher la page valide un texte en cours de saisie.
        self.commit_text(cx);
        let p = self.view.to_page(origin, pos);
        match cx.tools.tool {
            Tool::Pen | Tool::Highlighter => {
                self.selection.clear();
                if let Some(brush) = cx.tools.brush() {
                    // Un trait qui commence au contact d'un instrument suit son bord.
                    let lock = snap(&self.instruments, p, SNAP_PX / self.view.zoom);
                    let mut stroke = Stroke::new(brush);
                    stroke.push(StrokePoint {
                        pos: lock.map_or(p, |edge| edge.project(p)),
                        pressure,
                        tilt: None,
                    });
                    self.gesture = Gesture::Draw {
                        stroke,
                        lock,
                        anchor: p,
                        last_move: Instant::now(),
                        snapped: None,
                        source,
                    };
                }
            }
            Tool::EraserStroke => {
                self.selection.clear();
                self.gesture = Gesture::EraseStroke {
                    hits: HashSet::new(),
                    last: p,
                    source,
                };
                self.on_move(pos, pressure, origin, cx);
            }
            Tool::EraserArea => {
                self.selection.clear();
                self.gesture = Gesture::EraseArea {
                    pieces: HashMap::new(),
                    last: p,
                    source,
                };
                self.on_move(pos, pressure, origin, cx);
            }
            Tool::Lasso => self.lasso_down(source, pos, origin, cx),
            Tool::Instruments => self.instrument_down(source, pos, origin),
            Tool::Text => {
                self.selection.clear();
                self.text_down(p, cx);
            }
            Tool::Pipette => {
                let reach = 4.0 / self.view.zoom;
                if let Some(item) = cx.page.items.iter().rev().find(|i| i.hit(p, reach)) {
                    cx.tools.color = item.color();
                    cx.tools.tool = cx.tools.previous;
                }
            }
        }
    }

    fn instrument_down(&mut self, source: Source, pos: Pos2, origin: Pos2) {
        let view = self.view;
        let p = view.to_page(origin, pos);
        if let Some(index) = self.active_instrument {
            let handle = self.instruments.get(index).and_then(|instrument| {
                instrument
                    .handles()
                    .into_iter()
                    .find(|(_, at)| pos.distance(view.to_screen(origin, *at)) <= HANDLE_RADIUS)
                    .map(|(handle, _)| handle)
            });
            if let Some(handle) = handle {
                self.gesture = Gesture::Instrument {
                    index,
                    op: InstrumentOp::Handle(handle),
                    source,
                };
                return;
            }
        }
        match self.instruments.iter().rposition(|i| i.contains(p)) {
            Some(i) => {
                // L'instrument saisi passe au premier plan.
                let instrument = self.instruments.remove(i);
                self.instruments.push(instrument);
                let top = self.instruments.len() - 1;
                self.active_instrument = Some(top);
                self.gesture = Gesture::Instrument {
                    index: top,
                    op: InstrumentOp::Move { last: p },
                    source,
                };
            }
            None => self.active_instrument = None,
        }
    }

    fn lasso_down(&mut self, source: Source, pos: Pos2, origin: Pos2, cx: &mut Cx<'_>) {
        let view = self.view;
        let p = view.to_page(origin, pos);
        if let Some(bounds) = self.selection_bounds(cx.page, None) {
            let corners = corners_of(bounds);
            let center = bounds.center();
            let rotate_handle = view.to_screen(origin, Point2::new(center.x, bounds.min.y))
                - Vec2::new(0.0, ROTATE_HANDLE_OFFSET);
            let op = if pos.distance(rotate_handle) <= HANDLE_RADIUS {
                Some(TransformOp::Rotate {
                    pivot: center,
                    start_angle: (p.y - center.y).atan2(p.x - center.x),
                })
            } else if let Some(i) = corners
                .iter()
                .position(|c| pos.distance(view.to_screen(origin, *c)) <= HANDLE_RADIUS)
            {
                let pivot = corners[(i + 2) % 4];
                Some(TransformOp::Scale {
                    pivot,
                    start_dist: p.distance(pivot).max(1e-3),
                })
            } else if bounds.contains(p) {
                Some(TransformOp::Move { start: p })
            } else {
                None
            };
            if let Some(op) = op {
                self.gesture = Gesture::Transform {
                    op,
                    current: Similarity::identity(),
                    source,
                };
                return;
            }
        }
        self.selection.clear();
        self.gesture = Gesture::Lasso {
            points: vec![p],
            source,
        };
    }

    fn text_down(&mut self, p: Point2, cx: &mut Cx<'_>) {
        let reach = 2.0 / self.view.zoom;
        let existing = cx.page.items.iter().rev().find_map(|item| match item {
            Item::Text(t) if item.hit(p, reach) => Some(t.clone()),
            _ => None,
        });
        self.text_edit = Some(match existing {
            Some(t) => TextEdit {
                id: Some(t.id),
                pos: t.pos,
                text: t.text,
                font_size: t.font_size,
                color: t.color,
                angle: t.angle,
                fresh: true,
            },
            None => TextEdit {
                id: None,
                pos: p,
                text: String::new(),
                font_size: cx.tools.text_size,
                color: cx.tools.color,
                angle: 0.0,
                fresh: true,
            },
        });
    }

    fn on_move(&mut self, pos: Pos2, pressure: f32, origin: Pos2, cx: &mut Cx<'_>) {
        let view = self.view;
        let p = view.to_page(origin, pos);
        let radius = cx.tools.eraser_size * 0.5;
        match &mut self.gesture {
            Gesture::None => {}
            Gesture::Draw {
                stroke,
                lock,
                anchor,
                last_move,
                snapped,
                ..
            } => {
                // Une forme déjà reconnue pendant le maintien fige le trait jusqu'au relâchement.
                if snapped.is_some() {
                    return;
                }
                if anchor.distance(p) > HOLD_SLOP_PX / view.zoom {
                    *anchor = p;
                    *last_move = Instant::now();
                }
                let previous = stroke.points.last().map(|last| last.pos);
                let next = match lock {
                    Some(edge) => edge.extend(previous, p),
                    None => vec![p],
                };
                for q in next {
                    // Ignore les points quasi identiques pour garder des tracés légers.
                    if stroke
                        .points
                        .last()
                        .is_some_and(|last| last.pos.distance(q) < 0.5 / view.zoom)
                    {
                        continue;
                    }
                    stroke.push(StrokePoint {
                        pos: q,
                        pressure,
                        tilt: None,
                    });
                }
            }
            Gesture::Instrument { index, op, .. } => {
                if let Some(instrument) = self.instruments.get_mut(*index) {
                    match op {
                        InstrumentOp::Move { last } => {
                            instrument.translate(p.x - last.x, p.y - last.y);
                            *last = p;
                        }
                        InstrumentOp::Handle(handle) => instrument.drag_handle(*handle, p),
                    }
                }
            }
            Gesture::EraseStroke { hits, last, .. } => {
                for q in sweep(*last, p, radius * 0.5) {
                    for item in &cx.page.items {
                        // Rejet rapide par le rectangle englobant, avant le test précis.
                        let near = self
                            .render_cache
                            .bounds(item)
                            .map_or(true, |b| b.expand(radius).contains(q));
                        if near && item.hit(q, radius) {
                            hits.insert(item.id());
                        }
                    }
                }
                *last = p;
            }
            Gesture::EraseArea { pieces, last, .. } => {
                for q in sweep(*last, p, radius * 0.5) {
                    erase_area_at(cx.page, &self.render_cache, pieces, q, radius);
                }
                *last = p;
            }
            Gesture::Lasso { points, .. } => {
                if points
                    .last()
                    .map_or(true, |l| l.distance(p) >= 2.0 / view.zoom)
                {
                    points.push(p);
                }
            }
            Gesture::Transform { op, current, .. } => {
                *current = match *op {
                    TransformOp::Move { start } => {
                        Similarity::translation(p.x - start.x, p.y - start.y)
                    }
                    TransformOp::Scale { pivot, start_dist } => Similarity::scaling(
                        pivot,
                        (p.distance(pivot) / start_dist).clamp(0.05, 50.0),
                    ),
                    TransformOp::Rotate { pivot, start_angle } => Similarity::rotation(
                        pivot,
                        (p.y - pivot.y).atan2(p.x - pivot.x) - start_angle,
                    ),
                };
            }
        }
    }

    fn on_up(&mut self, cx: &mut Cx<'_>) {
        match std::mem::replace(&mut self.gesture, Gesture::None) {
            Gesture::None | Gesture::Instrument { .. } => {}
            Gesture::Draw {
                stroke,
                lock,
                snapped,
                ..
            } => {
                // Avec « formes automatiques », chaque trait est reconnu au relâchement ; sinon
                // seulement celui qu'un maintien a déjà reconnu. Le trait d'origine est ajouté
                // d'abord, la correction ensuite : un seul « Annuler » le retrouve.
                let corrected = snapped.or_else(|| {
                    (cx.tools.shape_recognition && lock.is_none())
                        .then(|| recognize_stroke(&stroke, false))
                        .flatten()
                });
                let id = stroke.id;
                cx.history.add_item(cx.page, Item::Stroke(stroke));
                if let Some(shape) = corrected {
                    self.notice = Some((shape.kind, Instant::now()));
                    cx.history.replace_items(
                        cx.page,
                        HashMap::from([(id, vec![Item::Stroke(shape.stroke)])]),
                    );
                }
            }
            Gesture::EraseStroke { hits, .. } => {
                let ids: Vec<Uuid> = hits.into_iter().collect();
                cx.history.remove_items(cx.page, &ids);
            }
            Gesture::EraseArea { pieces, .. } => {
                let replacements = pieces
                    .into_iter()
                    .map(|(id, strokes)| (id, strokes.into_iter().map(Item::Stroke).collect()))
                    .collect();
                cx.history.replace_items(cx.page, replacements);
            }
            Gesture::Lasso { points, .. } => {
                self.selection = cx
                    .page
                    .items
                    .iter()
                    .filter(|i| i.inside_polygon(&points))
                    .map(Item::id)
                    .collect();
            }
            Gesture::Transform { current, .. } => {
                if current == Similarity::identity() {
                    return;
                }
                let replacements = self
                    .selected_items(cx.page)
                    .into_iter()
                    .map(|item| (item.id(), vec![item.transformed(&current)]))
                    .collect();
                cx.history.replace_items(cx.page, replacements);
            }
        }
    }

    // ---- Texte --------------------------------------------------------------------------

    /// Valide le texte en cours de saisie : nouveau, modifié ou supprimé s'il est vide.
    fn commit_text(&mut self, cx: &mut Cx<'_>) {
        let Some(edit) = self.text_edit.take() else {
            return;
        };
        let empty = edit.text.trim().is_empty();
        match (edit.id, empty) {
            (None, true) => {}
            (Some(id), true) => cx.history.remove_items(cx.page, &[id]),
            (id, false) => {
                let galley = cx.painter.layout(
                    edit.text.clone(),
                    FontId::proportional(edit.font_size.max(1.0)),
                    Color32::BLACK,
                    f32::INFINITY,
                );
                let size = galley.size();
                let mut text = TextBox::new(edit.pos, edit.text, edit.font_size, edit.color);
                text.angle = edit.angle;
                text.width = size.x;
                text.height = size.y;
                match id {
                    Some(id) => {
                        text.id = id;
                        let unchanged = cx.page.item(id) == Some(&Item::Text(text.clone()));
                        if !unchanged {
                            cx.history.replace_items(
                                cx.page,
                                HashMap::from([(id, vec![Item::Text(text)])]),
                            );
                        }
                    }
                    None => cx.history.add_item(cx.page, Item::Text(text)),
                }
            }
        }
    }

    fn text_editor(&mut self, ui: &egui::Ui, origin: Pos2, cx: &mut Cx<'_>) {
        let view = self.view;
        let Some(edit) = self.text_edit.as_mut() else {
            return;
        };
        let screen = view.to_screen(origin, edit.pos);
        let font = FontId::proportional((edit.font_size * view.zoom).max(6.0));
        let color = color32(edit.color);
        let mut finished = false;
        egui::Area::new(egui::Id::unique("calame-text-edit"))
            .fixed_pos(screen)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                let response = ui.add(
                    egui::TextEdit::multiline(&mut edit.text)
                        .font(font)
                        .text_color(color)
                        .desired_rows(1)
                        .desired_width(120.0)
                        .lock_focus(true),
                );
                if edit.fresh {
                    response.request_focus();
                    edit.fresh = false;
                } else if response.lost_focus() {
                    finished = true;
                }
            });
        if finished {
            self.commit_text(cx);
        }
    }

    // ---- Rendu --------------------------------------------------------------------------

    fn selection_bounds(&self, page: &Page, transform: Option<Similarity>) -> Option<Rect2> {
        page.items
            .iter()
            .filter(|i| self.selection.contains(&i.id()))
            .filter_map(|item| match transform {
                Some(t) => item.transformed(&t).bounds(),
                None => self.render_cache.bounds(item),
            })
            .reduce(Rect2::union)
    }

    fn paint(
        &self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: Rect,
        cx: &Cx<'_>,
        backdrop: &Backdrop,
    ) {
        let painter = cx.painter;
        let (view, origin) = (self.view, rect.min);
        painter.rect_filled(rect, 0.0, Color32::WHITE);
        draw_backdrop(painter, rect, view, backdrop);

        let transform = match &self.gesture {
            Gesture::Transform { current, .. } => Some(*current),
            _ => None,
        };
        let editing = self.text_edit.as_ref().and_then(|e| e.id);
        // Zone de page visible : ce qui est en dehors n'est ni lissé ni dessiné.
        let visible = Rect2 {
            min: view.to_page(origin, rect.min),
            max: view.to_page(origin, rect.max),
        };
        self.render_cache.prune(cx.page);
        for item in &cx.page.items {
            let id = item.id();
            if Some(id) == editing {
                continue;
            }
            if let Gesture::EraseArea { pieces, .. } = &self.gesture {
                if let Some(list) = pieces.get(&id) {
                    for s in list {
                        paint_stroke(painter, s, &s.smoothed(SMOOTHING), view, origin, 1.0);
                    }
                    continue;
                }
            }
            let ghost =
                matches!(&self.gesture, Gesture::EraseStroke { hits, .. } if hits.contains(&id));
            let opacity = if ghost { 0.25 } else { 1.0 };
            let moved = transform
                .filter(|_| self.selection.contains(&id))
                .map(|t| item.transformed(&t));
            match (&moved, item) {
                // Cas courant : un trait immobile, dont le lissage est en cache.
                (None, Item::Stroke(s)) => {
                    let cached = self.render_cache.get(s);
                    if cached.bounds.is_some_and(|b| b.intersects(visible)) {
                        paint_stroke(painter, s, &cached.smoothed, view, origin, opacity);
                    }
                }
                _ => {
                    let shown = moved.as_ref().unwrap_or(item);
                    if shown.bounds().map_or(true, |b| b.intersects(visible)) {
                        paint_item(painter, shown, view, origin, opacity);
                    }
                }
            }
        }

        let to_screen = |p: Point2| view.to_screen(origin, p);
        instruments_ui::paint(
            painter,
            &self.instruments,
            self.active_instrument,
            cx.tools.tool == Tool::Instruments,
            &to_screen,
            view.zoom,
        );
        // Aide à l'accrochage : un repère sur le bord où le trait s'accrocherait.
        if matches!(cx.tools.tool, Tool::Pen | Tool::Highlighter)
            && matches!(self.gesture, Gesture::None)
        {
            if let Some(hover) = response.hover_pos() {
                let p = view.to_page(origin, hover);
                if let Some(edge) = snap(&self.instruments, p, SNAP_PX / view.zoom) {
                    painter.circle_stroke(
                        to_screen(edge.project(p)),
                        6.0,
                        egui::Stroke::new(2.0, Color32::from_rgb(240, 150, 40)),
                    );
                }
            }
        }

        match &self.gesture {
            Gesture::Draw {
                stroke, snapped, ..
            } => {
                let shown = snapped.as_ref().map_or(stroke, |s| &s.stroke);
                paint_stroke(painter, shown, &shown.smoothed(6), view, origin, 1.0);
            }
            Gesture::Lasso { points, .. } => {
                let line: Vec<Pos2> = points.iter().map(|p| view.to_screen(origin, *p)).collect();
                painter.add(Shape::line(line, ui.visuals().selection.stroke));
            }
            _ => {}
        }
        self.paint_notice(ui, painter, rect);
        self.paint_selection(ui, painter, cx.page, transform, view, origin);
        if matches!(cx.tools.tool, Tool::EraserStroke | Tool::EraserArea) {
            if let Some(hover) = response.hover_pos() {
                painter.circle_stroke(
                    hover,
                    cx.tools.eraser_size * 0.5 * view.zoom,
                    egui::Stroke::new(1.5, Color32::from_gray(110)),
                );
            }
        }
    }

    /// Message passager « Cercle », avec le moyen de revenir au trait d'origine.
    fn paint_notice(&self, ui: &egui::Ui, painter: &Painter, rect: Rect) {
        let Some((kind, at)) = self.notice else {
            return;
        };
        let age = at.elapsed();
        if age >= NOTICE_DURATION {
            return;
        }
        ui.ctx().request_repaint_after(NOTICE_DURATION - age);
        let text = format!(
            "{} · {}",
            crate::i18n::t(shape_label_key(kind)),
            crate::i18n::t("shape-undo-hint")
        );
        let galley = painter.layout_no_wrap(text, FontId::proportional(14.0), Color32::WHITE);
        let area = Rect::from_min_size(
            rect.min + Vec2::new(14.0, 14.0),
            galley.size() + Vec2::splat(16.0),
        );
        painter.rect_filled(area, 6.0, Color32::from_rgba_unmultiplied(30, 30, 40, 220));
        painter.galley(area.min + Vec2::splat(8.0), galley, Color32::WHITE);
    }

    fn paint_selection(
        &self,
        ui: &egui::Ui,
        painter: &Painter,
        page: &Page,
        transform: Option<Similarity>,
        view: View,
        origin: Pos2,
    ) {
        let Some(bounds) = self.selection_bounds(page, transform) else {
            return;
        };
        let stroke = ui.visuals().selection.stroke;
        let min = view.to_screen(origin, bounds.min);
        let max = view.to_screen(origin, bounds.max);
        painter.rect_stroke(
            Rect::from_min_max(min, max),
            0.0,
            stroke,
            egui::StrokeKind::Outside,
        );
        let fill = ui.visuals().extreme_bg_color;
        for corner in corners_of(bounds) {
            let c = view.to_screen(origin, corner);
            painter.rect_filled(Rect::from_center_size(c, Vec2::splat(9.0)), 2.0, fill);
            painter.rect_stroke(
                Rect::from_center_size(c, Vec2::splat(9.0)),
                2.0,
                stroke,
                egui::StrokeKind::Middle,
            );
        }
        let top = view.to_screen(origin, Point2::new(bounds.center().x, bounds.min.y));
        let knob = top - Vec2::new(0.0, ROTATE_HANDLE_OFFSET);
        painter.line_segment([top, knob], stroke);
        painter.circle_filled(knob, 6.0, fill);
        painter.circle_stroke(knob, 6.0, stroke);
    }
}

fn shape_label_key(kind: ShapeKind) -> &'static str {
    match kind {
        ShapeKind::Line => "shape-line",
        ShapeKind::Circle => "shape-circle",
        ShapeKind::Ellipse => "shape-ellipse",
        ShapeKind::Rectangle => "shape-rectangle",
        ShapeKind::Square => "shape-square",
        ShapeKind::Triangle => "shape-triangle",
        ShapeKind::Arrow => "shape-arrow",
        ShapeKind::Polygon => "shape-polygon",
        ShapeKind::Curve => "shape-curve",
    }
}

/// Coins d'un rectangle, dans l'ordre haut-gauche, haut-droit, bas-droit, bas-gauche.
fn corners_of(r: Rect2) -> [Point2; 4] {
    [
        r.min,
        Point2::new(r.max.x, r.min.y),
        r.max,
        Point2::new(r.min.x, r.max.y),
    ]
}

/// Points régulièrement espacés de `step` entre `from` (exclu) et `to` (inclus).
fn sweep(from: Point2, to: Point2, step: f32) -> impl Iterator<Item = Point2> {
    let n = (from.distance(to) / step.max(0.5)).ceil().max(1.0) as usize;
    (1..=n).map(move |k| from.lerp(to, k as f32 / n as f32))
}

/// Applique la gomme de zone en `center` à tous les traits de la page, en cumulant dans `pieces`.
fn erase_area_at(
    page: &Page,
    cache: &RenderCache,
    pieces: &mut HashMap<Uuid, Vec<Stroke>>,
    center: Point2,
    radius: f32,
) {
    let mut updates = Vec::new();
    for item in &page.items {
        let Item::Stroke(original) = item else {
            continue;
        };
        let next = match pieces.get(&original.id) {
            // Déjà découpé pendant ce geste : on poursuit sur les morceaux restants.
            Some(current) => erase_in(current, center, radius),
            None => {
                // Trait intact : le rectangle englobant en cache évite de le toucher s'il est loin.
                let near = cache
                    .get(original)
                    .bounds
                    .is_some_and(|b| b.expand(radius).contains(center));
                if !near {
                    continue;
                }
                erase_in(std::slice::from_ref(original), center, radius)
            }
        };
        if let Some(next) = next {
            updates.push((original.id, next));
        }
    }
    pieces.extend(updates);
}

/// Morceaux restants après la gomme, ou `None` si elle ne touche aucun de ces traits.
fn erase_in(strokes: &[Stroke], center: Point2, radius: f32) -> Option<Vec<Stroke>> {
    let mut next = Vec::with_capacity(strokes.len());
    let mut changed = false;
    for stroke in strokes {
        let near = stroke
            .bounds()
            .is_some_and(|b| b.expand(radius).contains(center));
        match near.then(|| stroke.erase_area(center, radius)).flatten() {
            Some(parts) => {
                changed = true;
                next.extend(parts);
            }
            None => next.push(stroke.clone()),
        }
    }
    changed.then_some(next)
}

/// Fond de la page affichée : motif généré, ou image (page de PDF importée, par exemple).
pub struct Backdrop {
    pub background: Background,
    /// Texture de l'image de fond et sa taille en unités de page, si le fond en est une.
    pub image: Option<(egui::TextureId, Vec2)>,
}

fn draw_backdrop(painter: &Painter, rect: Rect, view: View, backdrop: &Backdrop) {
    let to_screen = |p: Point2| view.to_screen(rect.min, p);
    if let Some((texture, size)) = backdrop.image {
        let image_rect = Rect::from_min_max(
            to_screen(Point2::new(0.0, 0.0)),
            to_screen(Point2::new(size.x, size.y)),
        );
        let full = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        painter.image(texture, image_rect, full, Color32::WHITE);
        return;
    }
    // Un motif trop serré à l'écran n'est qu'un aplat de bruit : on ne le dessine pas.
    if UNITS_PER_MM * 5.0 * view.zoom < 5.0 {
        return;
    }
    let visible = Rect2 {
        min: view.to_page(rect.min, rect.min),
        max: view.to_page(rect.min, rect.max),
    };
    let pattern = backdrop.background.pattern(visible);
    let color = color32(pattern.color);
    let stroke = egui::Stroke::new(1.0, color);
    for (a, b) in &pattern.lines {
        painter.line_segment([to_screen(*a), to_screen(*b)], stroke);
    }
    let radius = (1.0 * view.zoom).clamp(0.8, 2.5);
    for dot in &pattern.dots {
        painter.circle_filled(to_screen(*dot), radius, color);
    }
}

fn paint_item(painter: &Painter, item: &Item, view: View, origin: Pos2, opacity: f32) {
    match item {
        Item::Stroke(s) => paint_stroke(painter, s, &s.smoothed(6), view, origin, opacity),
        Item::Text(t) => {
            let color = color32(t.color).gamma_multiply(opacity);
            let galley = painter.layout(
                t.text.clone(),
                FontId::proportional((t.font_size * view.zoom).max(1.0)),
                color,
                f32::INFINITY,
            );
            painter.add(
                TextShape::new(view.to_screen(origin, t.pos), galley, color).with_angle(t.angle),
            );
        }
    }
}

/// Dessine un trait comme une bande de triangles d'épaisseur variable, avec des extrémités rondes.
fn paint_stroke(
    painter: &Painter,
    stroke: &Stroke,
    points: &[StrokePoint],
    view: View,
    origin: Pos2,
    opacity: f32,
) {
    let color = color32(stroke.brush.color).gamma_multiply(opacity);
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

#[cfg(test)]
mod tests {
    use super::*;
    use calame_core::Brush;

    fn horizontal_stroke(len: i32) -> Stroke {
        let mut s = Stroke::new(Brush::default());
        for x in 0..=len {
            s.push(StrokePoint {
                pos: Point2::new(x as f32, 0.0),
                pressure: 0.5,
                tilt: None,
            });
        }
        s
    }

    #[test]
    fn sweep_covers_the_whole_segment() {
        let pts: Vec<Point2> =
            sweep(Point2::new(0.0, 0.0), Point2::new(100.0, 0.0), 10.0).collect();
        assert_eq!(pts.len(), 10);
        assert_eq!(pts.last().copied(), Some(Point2::new(100.0, 0.0)));
        // Même sans déplacement, on traite au moins le point d'arrivée.
        assert_eq!(
            sweep(Point2::new(1.0, 1.0), Point2::new(1.0, 1.0), 5.0).count(),
            1
        );
    }

    #[test]
    fn area_eraser_accumulates_pieces_across_samples() {
        let stroke = horizontal_stroke(40);
        let id = stroke.id;
        let page = Page {
            items: vec![Item::Stroke(stroke)],
        };
        let mut pieces = HashMap::new();
        let cache = RenderCache::default();
        erase_area_at(&page, &cache, &mut pieces, Point2::new(10.0, 0.0), 2.0);
        assert_eq!(pieces[&id].len(), 2);
        // Un second passage sur le morceau de droite le découpe encore.
        erase_area_at(&page, &cache, &mut pieces, Point2::new(30.0, 0.0), 2.0);
        assert_eq!(pieces[&id].len(), 3);
        // Loin du trait : rien ne change.
        erase_area_at(&page, &cache, &mut pieces, Point2::new(20.0, 80.0), 2.0);
        assert_eq!(pieces[&id].len(), 3);
    }
}

/// Mesures de la gomme sur une grande page : `cargo test --release -p calame-app -- --ignored --nocapture`.
#[cfg(test)]
mod perf {
    use std::time::Instant;

    use super::*;
    use crate::render_cache::perf::big_page;

    #[test]
    #[ignore = "mesure de performance"]
    fn eraser_on_a_large_page() {
        let page = big_page(5000, 200);
        let cache = RenderCache::default();
        for item in &page.items {
            cache.bounds(item);
        }
        // Passage de la gomme au milieu de la page, échantillon par échantillon.
        let mut pieces = HashMap::new();
        let start = Instant::now();
        let samples = 200;
        for k in 0..samples {
            let center = Point2::new(3000.0 + k as f32 * 2.0, 1200.0);
            erase_area_at(&page, &cache, &mut pieces, center, 8.0);
        }
        let per_sample = start.elapsed() / samples;
        eprintln!(
            "PERF gomme de zone sur 5000 traits : {per_sample:?} par échantillon ({} traits découpés)",
            pieces.len()
        );
        // Trait entier : même balayage avec test précis après rejet par rectangle englobant.
        let mut hits = HashSet::new();
        let start = Instant::now();
        for k in 0..samples {
            let q = Point2::new(3000.0 + k as f32 * 2.0, 1200.0);
            for item in &page.items {
                let near = cache
                    .bounds(item)
                    .map_or(true, |b| b.expand(8.0).contains(q));
                if near && item.hit(q, 8.0) {
                    hits.insert(item.id());
                }
            }
        }
        eprintln!(
            "PERF gomme de trait sur 5000 traits : {:?} par échantillon ({} touchés)",
            start.elapsed() / samples,
            hits.len()
        );
    }
}
