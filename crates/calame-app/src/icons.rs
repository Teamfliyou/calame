//! Icônes vectorielles dessinées à la main, dans l'esprit des SF Symbols : même épaisseur de
//! trait, extrémités sobres, sur une grille de 24 × 24.
//!
//! Aucune police d'icônes n'est nécessaire : chaque icône est un tracé peint directement, donc
//! nette à toutes les tailles et teintée par la couleur du texte.

use egui::{
    pos2, vec2, Align2, Color32, FontId, Painter, Pos2, Rect, Response, Role, Sense, Shape,
    Stroke, StrokeKind, Ui, Vec2, WidgetInfo,
};

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

/// Pinceau à coordonnées 24 × 24, avec miroir horizontal optionnel.
struct Brush<'a> {
    painter: &'a Painter,
    origin: Pos2,
    k: f32,
    stroke: Stroke,
    color: Color32,
    flip: bool,
}

type Pts<'a> = &'a [(f32, f32)];

impl Brush<'_> {
    fn p(&self, x: f32, y: f32) -> Pos2 {
        let x = if self.flip { 24.0 - x } else { x };
        self.origin + vec2(x, y) * self.k
    }

    fn points(&self, pts: Pts<'_>) -> Vec<Pos2> {
        pts.iter().map(|&(x, y)| self.p(x, y)).collect()
    }

    fn line(&self, pts: Pts<'_>) {
        self.painter
            .add(Shape::line(self.points(pts), self.stroke));
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
            let y = 13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos();
            (12.0 + x * 0.55, 11.2 - y * 0.55)
        })
        .collect()
}

/// Peint `icon` au centre de `rect`, dans la couleur `color`.
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let k = rect.width().min(rect.height()) / 24.0;
    let stroke = Stroke::new((1.8 * k).max(1.3), color);
    let mut b = Brush {
        painter,
        origin: rect.center() - vec2(12.0, 12.0) * k,
        k,
        stroke,
        color,
        flip: false,
    };
    match icon {
        Icon::Pen => {
            b.closed(&[(15.0, 5.0), (19.0, 9.0), (9.0, 19.0), (4.0, 20.0), (5.0, 15.0)]);
            b.line(&[(13.0, 7.0), (17.0, 11.0)]);
        }
        Icon::Highlighter => {
            b.closed(&[(15.0, 3.0), (21.0, 9.0), (14.0, 16.0), (8.0, 10.0)]);
            b.closed(&[(8.0, 10.0), (14.0, 16.0), (10.0, 19.0), (5.0, 19.0), (5.5, 14.0)]);
            b.line(&[(13.0, 21.5), (21.0, 21.5)]);
        }
        Icon::EraserStroke => {
            b.closed(&[(14.0, 4.0), (20.0, 10.0), (11.0, 19.0), (5.0, 13.0)]);
            b.line(&[(9.0, 9.0), (15.0, 15.0)]);
            b.line(&[(10.0, 21.0), (20.0, 21.0)]);
        }
        Icon::EraserArea => {
            for i in 0..8 {
                let from = i as f32 * 45.0 + 6.0;
                b.line(&arc(12.0, 12.0, 8.5, from, from + 33.0));
            }
            b.dot(12.0, 12.0, 1.3);
        }
        Icon::Lasso => {
            let ellipse: Vec<(f32, f32)> = (0..24)
                .map(|i| {
                    let a = (i as f32 / 24.0) * std::f32::consts::TAU;
                    (12.0 + 9.0 * a.cos(), 10.0 + 6.0 * a.sin())
                })
                .collect();
            b.closed(&ellipse);
            b.line(&[(9.5, 15.5), (8.0, 19.0), (10.5, 21.0)]);
        }
        Icon::Text => {
            b.line(&[(6.0, 8.5), (6.0, 6.0), (18.0, 6.0), (18.0, 8.5)]);
            b.line(&[(12.0, 6.0), (12.0, 19.0)]);
            b.line(&[(9.0, 19.0), (15.0, 19.0)]);
        }
        Icon::Pipette => {
            b.closed(&[(14.0, 4.0), (20.0, 10.0), (17.0, 13.0), (11.0, 7.0)]);
            b.closed(&[(12.0, 8.0), (16.0, 12.0), (9.0, 19.0), (4.0, 20.0), (5.0, 15.0)]);
        }
        Icon::Ruler => {
            b.closed(&[(3.0, 16.0), (16.0, 3.0), (21.0, 8.0), (8.0, 21.0)]);
            for i in 1..=4 {
                let t = i as f32 / 5.0;
                let (x, y) = (3.0 + 13.0 * t, 16.0 - 13.0 * t);
                let len = if i % 2 == 1 { 4.5 } else { 3.0 };
                b.line(&[(x, y), (x + len * 0.707, y + len * 0.707)]);
            }
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
        Icon::Shapes => {
            b.circle(9.0, 9.0, 5.5);
            b.rrect(11.0, 11.0, 20.0, 20.0, 1.5);
        }
        Icon::Undo | Icon::Redo => {
            b.flip = icon == Icon::Redo;
            b.line(&[(9.0, 4.5), (4.0, 9.5), (9.0, 14.5)]);
            let mut path = vec![(4.0, 9.5)];
            path.extend(arc(14.0, 14.5, 5.0, -90.0, 90.0));
            path.push((8.0, 19.5));
            b.line(&path);
        }
        Icon::Recenter => {
            b.circle(12.0, 12.0, 6.0);
            b.line(&[(12.0, 2.5), (12.0, 6.0)]);
            b.line(&[(12.0, 18.0), (12.0, 21.5)]);
            b.line(&[(2.5, 12.0), (6.0, 12.0)]);
            b.line(&[(18.0, 12.0), (21.5, 12.0)]);
            b.dot(12.0, 12.0, 1.5);
        }
        Icon::ChevronLeft => b.line(&[(15.0, 5.0), (8.0, 12.0), (15.0, 19.0)]),
        Icon::Plus => {
            b.line(&[(12.0, 5.0), (12.0, 19.0)]);
            b.line(&[(5.0, 12.0), (19.0, 12.0)]);
        }
        Icon::Sidebar => {
            b.rrect(3.0, 5.0, 21.0, 19.0, 3.0);
            b.line(&[(9.5, 5.0), (9.5, 19.0)]);
        }
        Icon::Trash => {
            b.line(&[(4.0, 7.0), (20.0, 7.0)]);
            b.line(&[(9.0, 7.0), (9.0, 4.0), (15.0, 4.0), (15.0, 7.0)]);
            b.closed(&[(6.0, 7.0), (7.0, 20.0), (17.0, 20.0), (18.0, 7.0)]);
            b.line(&[(10.0, 11.0), (10.0, 16.0)]);
            b.line(&[(14.0, 11.0), (14.0, 16.0)]);
        }
        Icon::More => {
            b.dot(5.0, 12.0, 1.7);
            b.dot(12.0, 12.0, 1.7);
            b.dot(19.0, 12.0, 1.7);
        }
        Icon::Copy => {
            b.rrect(9.0, 9.0, 20.0, 20.0, 2.0);
            b.line(&[(9.0, 15.0), (4.0, 15.0), (4.0, 4.0), (15.0, 4.0), (15.0, 9.0)]);
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
        Icon::Folder | Icon::FolderPlus => {
            b.closed(&[
                (3.0, 6.0),
                (9.5, 6.0),
                (11.5, 8.5),
                (21.0, 8.5),
                (21.0, 19.5),
                (3.0, 19.5),
            ]);
            if icon == Icon::FolderPlus {
                b.line(&[(12.0, 11.5), (12.0, 16.5)]);
                b.line(&[(9.5, 14.0), (14.5, 14.0)]);
            }
        }
        Icon::Page | Icon::PagePlus => {
            b.closed(&[(6.0, 3.0), (14.0, 3.0), (19.0, 8.0), (19.0, 21.0), (6.0, 21.0)]);
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
            b.line(&[(10.5, 3.0), (10.5, 10.0), (5.0, 19.5), (19.0, 19.5), (13.5, 10.0), (13.5, 3.0)]);
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
    }
}

/// Bouton d'icône sans cadre, comme dans une barre d'outils Apple : fond discret au survol,
/// teinte d'accent quand il est actif. `label` sert d'infobulle et de nom pour les lecteurs
/// d'écran.
pub fn icon_button(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    selected: bool,
    enabled: bool,
) -> Response {
    let dark = ui.visuals().dark_mode;
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(36.0, 32.0), sense);
    if ui.is_rect_visible(rect) {
        let accent = theme::accent(dark);
        let background = if selected {
            Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 40)
        } else if enabled && response.hovered() {
            theme::fill_hover(dark)
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, 8.0, background);
        let color = if !enabled {
            theme::label_secondary(dark).gamma_multiply(0.45)
        } else if selected {
            accent
        } else {
            theme::label(dark)
        };
        let glyph = Rect::from_center_size(rect.center(), Vec2::splat(20.0));
        paint(ui.painter(), glyph, icon, color);
    }
    response.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, label));
    let response = response.on_hover_text(label);
    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonStyle {
    /// Action principale : fond d'accent, texte blanc.
    Prominent,
    /// Action secondaire : fond gris discret.
    Plain,
    /// Texte seul teinté d'accent (retour, liens).
    Link,
}

/// Bouton avec une icône facultative et un texte.
pub fn text_button(
    ui: &mut Ui,
    icon: Option<Icon>,
    text: &str,
    style: ButtonStyle,
    enabled: bool,
) -> Response {
    let dark = ui.visuals().dark_mode;
    let accent = theme::accent(dark);
    let color = match style {
        ButtonStyle::Prominent => Color32::WHITE,
        ButtonStyle::Plain => theme::label(dark),
        ButtonStyle::Link => accent,
    };
    let color = if enabled {
        color
    } else {
        color.gamma_multiply(0.45)
    };
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), FontId::proportional(14.0), color);
    let icon_width = if icon.is_some() { 24.0 } else { 0.0 };
    let pad = if style == ButtonStyle::Link { 4.0 } else { 14.0 };
    let size = vec2(galley.size().x + icon_width + pad * 2.0, 32.0);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    if ui.is_rect_visible(rect) {
        let hovered = enabled && response.hovered();
        let background = match style {
            ButtonStyle::Prominent => {
                if hovered {
                    accent.gamma_multiply(0.88)
                } else {
                    accent
                }
            }
            ButtonStyle::Plain => {
                if hovered {
                    theme::fill_hover(dark)
                } else {
                    theme::fill_secondary(dark)
                }
            }
            ButtonStyle::Link => {
                if hovered {
                    theme::fill_hover(dark)
                } else {
                    Color32::TRANSPARENT
                }
            }
        };
        ui.painter().rect_filled(rect, 8.0, background);
        let mut x = rect.left() + pad;
        if let Some(icon) = icon {
            let glyph = Rect::from_center_size(pos2(x + 9.0, rect.center().y), Vec2::splat(18.0));
            paint(ui.painter(), glyph, icon, color);
            x += icon_width;
        }
        let y = rect.center().y - galley.size().y / 2.0;
        ui.painter().galley(pos2(x, y), galley, color);
    }
    response.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, text));
    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

/// Pastille de couleur ronde ; un anneau d'accent entoure la couleur choisie. `label` est
/// l'infobulle et le nom lu par les lecteurs d'écran.
pub fn color_dot(
    ui: &mut Ui,
    color: Color32,
    diameter: f32,
    selected: bool,
    label: &str,
) -> Response {
    let dark = ui.visuals().dark_mode;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(diameter + 8.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let radius = diameter / 2.0 * if response.hovered() { 1.08 } else { 1.0 };
        let center = rect.center();
        ui.painter().circle_filled(center, radius, color);
        ui.painter()
            .circle_stroke(center, radius, Stroke::new(1.0, theme::separator(dark)));
        if selected {
            ui.painter()
                .circle_stroke(center, radius + 3.0, Stroke::new(2.0, theme::accent(dark)));
        }
    }
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, label));
    response
        .on_hover_text(label)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Pastille arrondie colorée portant le pictogramme d'une matière.
pub fn tile(painter: &Painter, rect: Rect, color: Color32, glyph: &str) {
    painter.rect_filled(rect, rect.width() * 0.24, color);
    let inner = Rect::from_center_size(rect.center(), Vec2::splat(rect.width() * 0.56));
    match Icon::from_key(glyph) {
        Some(icon) => paint(painter, inner, icon, Color32::WHITE),
        None if !glyph.trim().is_empty() => {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                glyph.trim(),
                FontId::proportional(rect.width() * 0.5),
                Color32::WHITE,
            );
        }
        None => paint(painter, inner, Icon::Book, Color32::WHITE),
    }
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
    fn arcs_start_and_end_on_the_circle() {
        let pts = arc(10.0, 10.0, 5.0, 0.0, 90.0);
        let first = pts[0];
        let last = pts[pts.len() - 1];
        assert!((first.0 - 15.0).abs() < 1e-4 && (first.1 - 10.0).abs() < 1e-4);
        assert!((last.0 - 10.0).abs() < 1e-4 && (last.1 - 15.0).abs() < 1e-4);
    }
}
