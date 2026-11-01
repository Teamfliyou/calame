//! Icônes vectorielles sur une grille de 24 × 24, trait de 1,8 et bouts ronds.
//!
//! Les icônes de l'interface reprennent les tracés SVG des maquettes (`docs/refonte-ux`), lus par
//! `svg_path` ; les instruments et les pictogrammes de matière sont dessinés à la main. Aucune
//! police d'icônes n'est nécessaire : chaque icône est peinte directement, nette à toutes les
//! tailles et teintée par la couleur demandée.

use egui::{vec2, Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};

use crate::svg_path::{self, Polyline};
use crate::theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    // Outils
    Pen,
    Highlighter,
    EraserStroke,
    EraserArea,
    Lasso,
    Text,
    Pipette,
    Ruler,
    SetSquare45,
    SetSquare60,
    Protractor,
    Compass,
    Shapes,
    // Actions
    Undo,
    Redo,
    Recenter,
    ChevronLeft,
    Plus,
    Sidebar,
    Trash,
    More,
    Copy,
    Cut,
    Paste,
    Recolor,
    Xmark,
    // Interface « l'encre d'abord »
    Search,
    Home,
    Clock,
    Favorite,
    Settings,
    Moon,
    Sun,
    Focus,
    Export,
    ImportPdf,
    Image,
    Check,
    ArrowRight,
    ChevronRight,
    ChevronDown,
    Minus,
    Close,
    // Éléments de la bibliothèque
    Folder,
    FolderPlus,
    Page,
    PagePlus,
    Notebook,
    // Pictogrammes de matière
    Book,
    Math,
    Flask,
    Globe,
    Brush,
    Music,
    Code,
    Sport,
    Chat,
    Bulb,
    Heart,
    Star,
}

/// Pictogrammes proposés pour une matière, dans l'ordre d'affichage.
pub const SUBJECT_GLYPHS: [Icon; 12] = [
    Icon::Book,
    Icon::Math,
    Icon::Flask,
    Icon::Globe,
    Icon::Brush,
    Icon::Music,
    Icon::Code,
    Icon::Sport,
    Icon::Chat,
    Icon::Bulb,
    Icon::Heart,
    Icon::Star,
];

impl Icon {
    /// Identifiant enregistré dans la bibliothèque pour un pictogramme de matière.
    pub fn key(self) -> &'static str {
        match self {
            Self::Math => "math",
            Self::Flask => "flask",
            Self::Globe => "globe",
            Self::Brush => "brush",
            Self::Music => "music",
            Self::Code => "code",
            Self::Sport => "sport",
            Self::Chat => "chat",
            Self::Bulb => "bulb",
            Self::Heart => "heart",
            Self::Star => "star",
            _ => "book",
        }
    }

    /// Pictogramme correspondant à ce qui est enregistré. Les anciennes bibliothèques gardent des
    /// émojis : les plus courants sont traduits, les autres restent affichés tels quels.
    pub fn from_key(key: &str) -> Option<Self> {
        let icon = match key.trim() {
            "book" | "📚" | "📖" => Self::Book,
            "math" | "📐" => Self::Math,
            "flask" | "🔬" | "🧪" => Self::Flask,
            "globe" | "🌍" => Self::Globe,
            "brush" | "🎨" => Self::Brush,
            "music" | "🎵" => Self::Music,
            "code" | "💻" => Self::Code,
            "sport" | "⚽" => Self::Sport,
            "chat" | "🗣" => Self::Chat,
            "bulb" | "💡" => Self::Bulb,
            "heart" => Self::Heart,
            "star" => Self::Star,
            _ => return None,
        };
        Some(icon)
    }
}

/// Pinceau à coordonnées 24 × 24.
struct Brush<'a> {
    painter: &'a Painter,
    origin: Pos2,
    k: f32,
    stroke: Stroke,
    color: Color32,
}

type Pts<'a> = &'a [(f32, f32)];

impl Brush<'_> {
    fn p(&self, x: f32, y: f32) -> Pos2 {
        self.origin + vec2(x, y) * self.k
    }

    fn points(&self, pts: Pts<'_>) -> Vec<Pos2> {
        pts.iter().map(|&(x, y)| self.p(x, y)).collect()
    }

    fn line(&self, pts: Pts<'_>) {
        self.painter.add(Shape::line(self.points(pts), self.stroke));
    }

    fn closed(&self, pts: Pts<'_>) {
        self.painter
            .add(Shape::closed_line(self.points(pts), self.stroke));
    }

    fn circle(&self, cx: f32, cy: f32, r: f32) {
        self.painter
            .circle_stroke(self.p(cx, cy), r * self.k, self.stroke);
    }

    fn dot(&self, cx: f32, cy: f32, r: f32) {
        self.painter
            .circle_filled(self.p(cx, cy), r * self.k, self.color);
    }

    fn rrect(&self, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) {
        let rect = Rect::from_two_pos(self.p(x0, y0), self.p(x1, y1));
        self.painter
            .rect_stroke(rect, r * self.k, self.stroke, StrokeKind::Middle);
    }
}

/// Points d'un arc de cercle, angles en degrés (0° à droite, sens horaire à l'écran).
fn arc(cx: f32, cy: f32, r: f32, from: f32, to: f32) -> Vec<(f32, f32)> {
    let steps = 16;
    (0..=steps)
        .map(|i| {
            let a = (from + (to - from) * i as f32 / steps as f32).to_radians();
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

fn star_points() -> Vec<(f32, f32)> {
    (0..10)
        .map(|i| {
            let a = (-90.0 + 36.0 * i as f32).to_radians();
            let r = if i % 2 == 0 { 9.5 } else { 4.0 };
            (12.0 + r * a.cos(), 12.8 + r * a.sin())
        })
        .collect()
}

fn heart_points() -> Vec<(f32, f32)> {
    (0..32)
        .map(|i| {
            let t = (i as f32 / 32.0) * std::f32::consts::TAU;
            let x = 16.0 * t.sin().powi(3);
            let y =
                13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos();
            (12.0 + x * 0.55, 11.2 - y * 0.55)
        })
        .collect()
}

/// Tracé SVG (viewBox 24) et épaisseur de trait des icônes reprises des maquettes.
fn svg_of(icon: Icon) -> Option<(&'static str, f32)> {
    let path = match icon {
        Icon::Pen => ("M4 20l1.2-4.4L16 4.8l3.2 3.2L8.4 18.8z M14 7l3 3", 1.8),
        Icon::Highlighter => (
            "M8 13.5l7.8-8.8 4.2 3.6-8 8.6z M8 13.5L6 19l2.5 1 3.5-3.7 M3.5 21h6",
            1.8,
        ),
        Icon::EraserStroke => (
            "M4.5 15.5l9-9.4a1.5 1.5 0 0 1 2.1 0l3.3 3.3a1.5 1.5 0 0 1 0 2.1l-7 7H7.4z M9.5 10.5l5 5 M12 18.5h8",
            1.8,
        ),
        Icon::Lasso => (
            "M12 4.5c4.6 0 8 2.3 8 5.2s-3.4 5.3-8 5.3-8-2.4-8-5.3 3.4-5.2 8-5.2z M7.6 14.3c-1.7 1.3-1.5 3.6.4 4.3 1.3.5 1.7 1.5 1.1 2.6",
            1.8,
        ),
        Icon::Text => ("M5 7V5h14v2 M12 5v14 M9 19h6", 1.8),
        Icon::Image => (
            "M4 5.5h16v13H4z M4 15.5l4.5-4.5 4 4 2.5-2.5 5 5 M15.5 9.5h.01",
            1.8,
        ),
        Icon::Ruler => (
            "M3.5 16.5L16.5 3.5l4 4-13 13z M7.5 12.5l2 2 M10.5 9.5l2 2 M13.5 6.5l2 2",
            1.8,
        ),
        Icon::Shapes => ("M12.5 8a4.5 4.5 0 1 1-9 0 4.5 4.5 0 0 1 9 0z M13 13h7.5v7.5H13z", 1.8),
        Icon::Undo => ("M9 14L4 9l5-5 M4 9h10.5a5.5 5.5 0 0 1 0 11H11", 1.8),
        Icon::Redo => ("M15 14l5-5-5-5 M20 9H9.5a5.5 5.5 0 0 0 0 11H13", 1.8),
        Icon::ChevronLeft => ("M15 18l-6-6 6-6", 2.0),
        Icon::ChevronRight => ("M9 6l6 6-6 6", 2.2),
        Icon::ChevronDown => ("M6 9l6 6 6-6", 2.2),
        Icon::Plus => ("M12 5v14M5 12h14", 2.0),
        Icon::Minus => ("M5 12h14", 2.0),
        Icon::Sidebar => ("M6 4.5h12a2.5 2.5 0 0 1 2.5 2.5v10a2.5 2.5 0 0 1-2.5 2.5H6A2.5 2.5 0 0 1 3.5 17V7A2.5 2.5 0 0 1 6 4.5z M9.5 4.5v15", 1.8),
        Icon::Trash => ("M5 7h14M10 7V5h4v2M7 7l1 12.5h8L17 7", 1.8),
        Icon::Copy => (
            "M10 8h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-8a2 2 0 0 1-2-2v-8a2 2 0 0 1 2-2z M16 8V5.5A1.5 1.5 0 0 0 14.5 4h-9A1.5 1.5 0 0 0 4 5.5v9A1.5 1.5 0 0 0 5.5 16H8",
            1.8,
        ),
        Icon::Folder => (
            "M3.5 7a1 1 0 0 1 1-1h5l2 2h8a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1h-15a1 1 0 0 1-1-1z",
            1.8,
        ),
        Icon::Search => ("M17.5 11a6.5 6.5 0 1 1-13 0 6.5 6.5 0 0 1 13 0z M20 20l-4.3-4.3", 1.8),
        Icon::Home => (
            "M4 11l8-7 8 7v8.5a.5.5 0 0 1-.5.5H15v-6H9v6H4.5a.5.5 0 0 1-.5-.5z",
            1.8,
        ),
        Icon::Clock => ("M20 12a8 8 0 1 1-16 0 8 8 0 0 1 16 0z M12 8v4l2.8 1.8", 1.8),
        Icon::Favorite => ("M12 4l2.4 5 5.4.6-4 3.7 1.1 5.3L12 16l-4.9 2.6 1.1-5.3-4-3.7 5.4-.6z", 1.8),
        Icon::Settings => (
            "M4 7h9M17 7h3M4 17h3M11 17h9 M17 7a2 2 0 1 1-4 0 2 2 0 0 1 4 0z M11 17a2 2 0 1 1-4 0 2 2 0 0 1 4 0z",
            1.8,
        ),
        Icon::Moon => ("M19 14.5A7.5 7.5 0 0 1 9.5 5a7.5 7.5 0 1 0 9.5 9.5z", 1.8),
        Icon::Sun => (
            "M16 12a4 4 0 1 1-8 0 4 4 0 0 1 8 0z M12 3v2 M12 19v2 M3 12h2 M19 12h2 M5.6 5.6l1.4 1.4 M17 17l1.4 1.4 M5.6 18.4L7 17 M17 7l1.4-1.4",
            1.8,
        ),
        Icon::Focus => ("M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5", 1.8),
        Icon::Export => (
            "M12 15V4M7.5 8.5L12 4l4.5 4.5M5 14v5.5a.5.5 0 0 0 .5.5h13a.5.5 0 0 0 .5-.5V14",
            1.8,
        ),
        Icon::ImportPdf => (
            "M14 3H6.5a.5.5 0 0 0-.5.5v17a.5.5 0 0 0 .5.5h11a.5.5 0 0 0 .5-.5V7z M14 3v4h4M12 11v6M9.5 14.5L12 17l2.5-2.5",
            1.8,
        ),
        Icon::Check => ("M5 12.5l4.5 4.5L19 7.5", 2.4),
        Icon::ArrowRight => ("M5 12h14M13 6l6 6-6 6", 2.0),
        Icon::Close => ("M6 6l12 12M18 6L6 18", 2.2),
        _ => return None,
    };
    Some(path)
}

thread_local! {
    /// Tracés déjà découpés, pour ne pas relire les chaînes à chaque image.
    static PARSED: std::cell::RefCell<std::collections::HashMap<&'static str, std::rc::Rc<Vec<Polyline>>>> =
        std::cell::RefCell::default();
}

fn parsed(d: &'static str) -> std::rc::Rc<Vec<Polyline>> {
    PARSED.with(|cache| {
        cache
            .borrow_mut()
            .entry(d)
            .or_insert_with(|| std::rc::Rc::new(svg_path::parse(d)))
            .clone()
    })
}

/// Peint un tracé SVG (viewBox de `view` unités) dans `rect`, avec des bouts ronds.
pub fn paint_svg(
    painter: &Painter,
    rect: Rect,
    d: &'static str,
    view: f32,
    width: f32,
    color: Color32,
) {
    let k = rect.width().min(rect.height()) / view;
    let origin = rect.center() - vec2(view, view) * 0.5 * k;
    let stroke = Stroke::new((width * k).max(1.0), color);
    for line in parsed(d).iter() {
        let pts: Vec<Pos2> = line
            .points
            .iter()
            .map(|&(x, y)| origin + vec2(x, y) * k)
            .collect();
        if line.closed {
            painter.add(Shape::closed_line(pts, stroke));
        } else {
            if let (Some(first), Some(last)) = (pts.first(), pts.last()) {
                painter.circle_filled(*first, stroke.width / 2.0, color);
                painter.circle_filled(*last, stroke.width / 2.0, color);
            }
            painter.add(Shape::line(pts, stroke));
        }
    }
}

/// Peint `icon` au centre de `rect`, dans la couleur `color`.
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    if let Some((d, width)) = svg_of(icon) {
        paint_svg(painter, rect, d, 24.0, width, color);
        return;
    }
    let k = rect.width().min(rect.height()) / 24.0;
    let stroke = Stroke::new((1.8 * k).max(1.3), color);
    let b = Brush {
        painter,
        origin: rect.center() - vec2(12.0, 12.0) * k,
        k,
        stroke,
        color,
    };
    match icon {
        Icon::EraserArea => {
            for i in 0..8 {
                let from = i as f32 * 45.0 + 6.0;
                b.line(&arc(12.0, 12.0, 8.5, from, from + 33.0));
            }
            b.dot(12.0, 12.0, 1.3);
        }
        Icon::Pipette => {
            b.closed(&[(14.0, 4.0), (20.0, 10.0), (17.0, 13.0), (11.0, 7.0)]);
            b.closed(&[
                (12.0, 8.0),
                (16.0, 12.0),
                (9.0, 19.0),
                (4.0, 20.0),
                (5.0, 15.0),
            ]);
        }
        Icon::SetSquare45 => {
            b.closed(&[(4.0, 20.0), (4.0, 4.0), (20.0, 20.0)]);
            b.closed(&[(8.0, 16.5), (8.0, 11.5), (13.0, 16.5)]);
        }
        Icon::SetSquare60 => {
            b.closed(&[(6.0, 21.0), (6.0, 3.0), (20.0, 21.0)]);
            b.line(&[(6.0, 13.0), (9.0, 13.0)]);
            b.line(&[(6.0, 17.0), (9.5, 17.0)]);
        }
        Icon::Protractor => {
            b.line(&arc(12.0, 18.0, 9.5, 180.0, 360.0));
            b.line(&[(2.5, 18.0), (21.5, 18.0)]);
            for deg in [210.0_f32, 240.0, 270.0, 300.0, 330.0] {
                let a = deg.to_radians();
                let inner = if (deg - 270.0).abs() < 1.0 { 6.0 } else { 7.5 };
                b.line(&[
                    (12.0 + 9.5 * a.cos(), 18.0 + 9.5 * a.sin()),
                    (12.0 + inner * a.cos(), 18.0 + inner * a.sin()),
                ]);
            }
            b.dot(12.0, 18.0, 1.1);
        }
        Icon::Compass => {
            b.circle(12.0, 5.0, 1.8);
            b.line(&[(11.0, 6.6), (5.0, 21.0)]);
            b.line(&[(13.0, 6.6), (19.0, 21.0)]);
            b.line(&[(8.0, 14.0), (16.0, 14.0)]);
        }
        Icon::Recenter => {
            b.circle(12.0, 12.0, 6.0);
            b.line(&[(12.0, 2.5), (12.0, 6.0)]);
            b.line(&[(12.0, 18.0), (12.0, 21.5)]);
            b.line(&[(2.5, 12.0), (6.0, 12.0)]);
            b.line(&[(18.0, 12.0), (21.5, 12.0)]);
            b.dot(12.0, 12.0, 1.5);
        }
        Icon::More => {
            b.dot(5.0, 12.0, 1.7);
            b.dot(12.0, 12.0, 1.7);
            b.dot(19.0, 12.0, 1.7);
        }
        Icon::Cut => {
            b.circle(7.0, 18.0, 2.6);
            b.circle(17.0, 18.0, 2.6);
            b.line(&[(8.5, 15.8), (17.0, 3.5)]);
            b.line(&[(15.5, 15.8), (7.0, 3.5)]);
        }
        Icon::Paste => {
            b.rrect(5.0, 5.0, 19.0, 21.0, 2.5);
            b.rrect(9.0, 3.0, 15.0, 7.0, 1.5);
            b.line(&[(9.0, 12.0), (15.0, 12.0)]);
            b.line(&[(9.0, 16.0), (15.0, 16.0)]);
        }
        Icon::Recolor => {
            let mut pts = vec![(12.0, 3.0)];
            pts.extend(arc(12.0, 15.0, 6.5, -35.0, 215.0));
            b.closed(&pts);
        }
        Icon::Xmark => {
            b.line(&[(6.0, 6.0), (18.0, 18.0)]);
            b.line(&[(18.0, 6.0), (6.0, 18.0)]);
        }
        Icon::FolderPlus => {
            if let Some((d, width)) = svg_of(Icon::Folder) {
                paint_svg(painter, rect, d, 24.0, width, color);
            }
            b.line(&[(12.0, 11.0), (12.0, 16.0)]);
            b.line(&[(9.5, 13.5), (14.5, 13.5)]);
        }
        Icon::Page | Icon::PagePlus => {
            b.closed(&[
                (6.0, 3.0),
                (14.0, 3.0),
                (19.0, 8.0),
                (19.0, 21.0),
                (6.0, 21.0),
            ]);
            b.line(&[(14.0, 3.0), (14.0, 8.0), (19.0, 8.0)]);
            if icon == Icon::PagePlus {
                b.line(&[(12.5, 12.0), (12.5, 18.0)]);
                b.line(&[(9.5, 15.0), (15.5, 15.0)]);
            } else {
                b.line(&[(9.0, 13.0), (16.0, 13.0)]);
                b.line(&[(9.0, 17.0), (16.0, 17.0)]);
            }
        }
        Icon::Notebook => {
            b.rrect(5.0, 3.0, 19.0, 21.0, 2.0);
            b.line(&[(9.0, 3.0), (9.0, 21.0)]);
            b.line(&[(12.0, 8.0), (16.0, 8.0)]);
        }
        Icon::Book => {
            b.line(&[(12.0, 7.0), (5.0, 5.0), (5.0, 18.0), (12.0, 20.0)]);
            b.line(&[(12.0, 7.0), (19.0, 5.0), (19.0, 18.0), (12.0, 20.0)]);
            b.line(&[(12.0, 7.0), (12.0, 20.0)]);
        }
        Icon::Math => {
            b.line(&[(5.0, 7.0), (19.0, 7.0)]);
            b.line(&[(9.0, 7.0), (8.0, 19.0)]);
            b.line(&[(15.0, 7.0), (15.0, 17.0), (16.5, 19.0)]);
        }
        Icon::Flask => {
            b.line(&[(10.0, 3.0), (14.0, 3.0)]);
            b.line(&[
                (10.5, 3.0),
                (10.5, 10.0),
                (5.0, 19.5),
                (19.0, 19.5),
                (13.5, 10.0),
                (13.5, 3.0),
            ]);
            b.line(&[(7.5, 15.0), (16.5, 15.0)]);
        }
        Icon::Globe => {
            b.circle(12.0, 12.0, 9.0);
            let ellipse: Vec<(f32, f32)> = (0..24)
                .map(|i| {
                    let a = (i as f32 / 24.0) * std::f32::consts::TAU;
                    (12.0 + 4.0 * a.cos(), 12.0 + 9.0 * a.sin())
                })
                .collect();
            b.closed(&ellipse);
            b.line(&[(3.0, 12.0), (21.0, 12.0)]);
        }
        Icon::Brush => {
            b.circle(12.0, 12.0, 9.0);
            b.dot(8.0, 10.0, 1.3);
            b.dot(12.0, 7.5, 1.3);
            b.dot(16.0, 10.0, 1.3);
            b.circle(14.0, 15.5, 2.2);
        }
        Icon::Music => {
            b.line(&[(9.0, 18.0), (9.0, 5.0), (18.0, 3.5), (18.0, 16.0)]);
            b.line(&[(9.0, 9.0), (18.0, 7.5)]);
            b.circle(7.0, 18.0, 2.5);
            b.circle(16.0, 16.0, 2.5);
        }
        Icon::Code => {
            b.line(&[(9.0, 6.0), (3.0, 12.0), (9.0, 18.0)]);
            b.line(&[(15.0, 6.0), (21.0, 12.0), (15.0, 18.0)]);
            b.line(&[(13.5, 5.0), (10.5, 19.0)]);
        }
        Icon::Sport => {
            b.circle(12.0, 12.0, 9.0);
            b.line(&[(12.0, 3.0), (12.0, 21.0)]);
            b.line(&arc(2.0, 12.0, 10.5, -52.0, 52.0));
            b.line(&arc(22.0, 12.0, 10.5, 128.0, 232.0));
        }
        Icon::Chat => {
            b.rrect(3.0, 4.0, 21.0, 17.0, 4.0);
            b.line(&[(8.0, 17.0), (7.0, 21.0), (12.0, 17.0)]);
        }
        Icon::Bulb => {
            b.line(&arc(12.0, 10.0, 6.5, 125.0, 415.0));
            b.line(&[(8.3, 15.3), (9.5, 18.0), (14.5, 18.0), (15.7, 15.3)]);
            b.line(&[(10.0, 20.5), (14.0, 20.5)]);
        }
        Icon::Heart => b.closed(&heart_points()),
        Icon::Star => b.closed(&star_points()),
        // Les autres icônes sont des tracés SVG, peints plus haut.
        _ => {}
    }
}

/// Pastille arrondie colorée portant le pictogramme d'une matière.
pub fn tile(painter: &Painter, rect: Rect, color: Color32, glyph: &str) {
    painter.rect_filled(rect, rect.width() * 0.24, color);
    let ink = theme::on_color(color);
    let inner = Rect::from_center_size(rect.center(), Vec2::splat(rect.width() * 0.56));
    match Icon::from_key(glyph) {
        Some(icon) => paint(painter, inner, icon, ink),
        None if !glyph.trim().is_empty() => {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                glyph.trim(),
                FontId::proportional(rect.width() * 0.5),
                ink,
            );
        }
        None => paint(painter, inner, Icon::Book, ink),
    }
}

/// Logo de Calame : carré d'encre, plume blanche et vague safran (d'après `assets/icon.png`).
pub fn logo(painter: &Painter, rect: Rect, accent: Color32) {
    let k = rect.width() / 32.0;
    painter.rect_filled(rect, 9.0 * k, accent);
    let p = |x: f32, y: f32| rect.min + vec2(x, y) * k;
    painter.add(Shape::convex_polygon(
        vec![
            p(9.0, 22.0),
            p(11.2, 16.4),
            p(20.5, 7.5),
            p(23.5, 10.5),
            p(14.5, 19.5),
        ],
        Color32::WHITE,
        Stroke::NONE,
    ));
    paint_svg(
        painter,
        rect,
        "M7 26c1.5-1.6 3-1.6 4.5 0s3 1.6 4.5 0 3-1.6 4.5 0 3 1.6 4.5 0",
        32.0,
        2.0,
        theme::SAFFRON,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_keys_roundtrip_and_legacy_emojis_are_translated() {
        for icon in SUBJECT_GLYPHS {
            assert_eq!(Icon::from_key(icon.key()), Some(icon));
        }
        assert_eq!(Icon::from_key("📚"), Some(Icon::Book));
        assert_eq!(Icon::from_key("🧪"), Some(Icon::Flask));
        assert_eq!(Icon::from_key("🦄"), None);
        assert_eq!(Icon::from_key(""), None);
    }

    #[test]
    fn every_interface_icon_has_a_readable_path() {
        for icon in [
            Icon::Pen,
            Icon::Highlighter,
            Icon::EraserStroke,
            Icon::Lasso,
            Icon::Text,
            Icon::Image,
            Icon::Ruler,
            Icon::Shapes,
            Icon::Undo,
            Icon::Redo,
            Icon::ChevronLeft,
            Icon::ChevronRight,
            Icon::ChevronDown,
            Icon::Plus,
            Icon::Minus,
            Icon::Sidebar,
            Icon::Trash,
            Icon::Copy,
            Icon::Folder,
            Icon::Search,
            Icon::Home,
            Icon::Clock,
            Icon::Favorite,
            Icon::Settings,
            Icon::Moon,
            Icon::Sun,
            Icon::Focus,
            Icon::Export,
            Icon::ImportPdf,
            Icon::Check,
            Icon::ArrowRight,
            Icon::Close,
        ] {
            let (d, _) = svg_of(icon).unwrap_or(("", 0.0));
            let lines = svg_path::parse(d);
            assert!(!lines.is_empty(), "{icon:?}");
            for line in lines {
                for (x, y) in line.points {
                    assert!(
                        (-0.5..=24.5).contains(&x) && (-0.5..=24.5).contains(&y),
                        "{icon:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn arcs_start_and_end_on_the_circle() {
        let pts = arc(10.0, 10.0, 5.0, 0.0, 90.0);
        let first = pts[0];
        let last = pts[pts.len() - 1];
        assert!((first.0 - 15.0).abs() < 1e-4 && (first.1 - 10.0).abs() < 1e-4);
        assert!((last.0 - 10.0).abs() < 1e-4 && (last.1 - 15.0).abs() < 1e-4);
    }
}
