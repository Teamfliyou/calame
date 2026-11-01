//! Petits composants de l'interface : boutons, pastilles de couleur, touches, puces, cadres en
//! pointillés. Leurs mesures viennent des maquettes (`docs/refonte-ux/maquettes`).

use egui::text::{LayoutJob, TextFormat};
use egui::{
    pos2, vec2, Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Response, Role, Sense,
    Shape, Stroke, StrokeKind, Ui, Vec2, WidgetInfo,
};

use crate::icons::{self, Icon};
use crate::theme::{self, Weight};

fn clickable(response: Response, enabled: bool) -> Response {
    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

/// Bouton d'icône carré de 40 px : fond discret au survol, fond d'accent doux quand il est actif.
/// `label` sert d'infobulle et de nom pour les lecteurs d'écran.
pub fn icon_button(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    selected: bool,
    enabled: bool,
) -> Response {
    icon_button_sized(ui, icon, label, selected, enabled, 40.0)
}

pub fn icon_button_sized(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    selected: bool,
    enabled: bool,
    size: f32,
) -> Response {
    let dark = ui.visuals().dark_mode;
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), sense);
    if ui.is_rect_visible(rect) {
        let background = if selected {
            theme::accent_soft(dark)
        } else if enabled && response.hovered() {
            theme::hover(dark)
        } else {
            Color32::TRANSPARENT
        };
        ui.painter()
            .rect_filled(rect, theme::RADIUS_BUTTON, background);
        let color = if !enabled {
            theme::disabled(dark)
        } else if selected {
            theme::accent(dark)
        } else {
            theme::icon(dark)
        };
        let glyph = Rect::from_center_size(rect.center(), Vec2::splat(size * 0.48));
        icons::paint(ui.painter(), glyph, icon, color);
    }
    response.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, label));
    clickable(response.on_hover_text(label), enabled)
}

/// Outil du dock : 44 px, rayon 13 ; l'outil actif est une tuile d'encre à l'icône blanche.
pub fn tool_button(ui: &mut Ui, icon: Icon, label: &str, active: bool) -> Response {
    let dark = ui.visuals().dark_mode;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let (ink_bg, ink_fg) = theme::ink_button(dark);
        let (background, color) = if active {
            (ink_bg, ink_fg)
        } else if response.hovered() {
            (theme::hover(dark), theme::icon(dark))
        } else {
            (Color32::TRANSPARENT, theme::icon(dark))
        };
        ui.painter()
            .rect_filled(rect, theme::RADIUS_DOCK_ITEM, background);
        let glyph = Rect::from_center_size(rect.center(), Vec2::splat(21.0));
        icons::paint(ui.painter(), glyph, icon, color);
    }
    response.widget_info(|| WidgetInfo::selected(Role::Button, true, active, label));
    clickable(response.on_hover_text(label), true)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonStyle {
    /// Action principale : fond d'encre bleue, texte blanc.
    Primary,
    /// Bouton sombre (« Continuer à écrire », « Exporter »).
    Ink,
    /// Action secondaire : fond blanc, bordure de champ.
    Secondary,
    /// Texte seul, fond au survol (« ‹ Bibliothèque », « Annuler »).
    Ghost,
    /// Texte teinté d'accent (« Tout voir »).
    Link,
}

/// Bouton avec texte, icône de tête et icône de fin facultatives.
pub struct Button<'a> {
    text: &'a str,
    style: ButtonStyle,
    icon: Option<Icon>,
    trailing: Option<Icon>,
    height: f32,
    enabled: bool,
    full_width: bool,
}

impl<'a> Button<'a> {
    pub fn new(text: &'a str, style: ButtonStyle) -> Self {
        let height = match style {
            ButtonStyle::Primary | ButtonStyle::Secondary => 44.0,
            ButtonStyle::Ink | ButtonStyle::Ghost => 40.0,
            ButtonStyle::Link => 32.0,
        };
        Self {
            text,
            style,
            icon: None,
            trailing: None,
            height,
            enabled: true,
            full_width: false,
        }
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn trailing(mut self, icon: Icon) -> Self {
        self.trailing = Some(icon);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Occupe toute la largeur disponible, contenu centré.
    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let dark = ui.visuals().dark_mode;
        let accent = theme::accent(dark);
        let (ink_bg, ink_fg) = theme::ink_button(dark);
        let mut color = match self.style {
            ButtonStyle::Primary => {
                if dark {
                    theme::INK_BUTTON
                } else {
                    Color32::WHITE
                }
            }
            ButtonStyle::Ink => ink_fg,
            ButtonStyle::Secondary | ButtonStyle::Ghost => theme::label(dark),
            ButtonStyle::Link => accent,
        };
        if !self.enabled {
            color = color.gamma_multiply(0.45);
        }
        let weight = match self.style {
            ButtonStyle::Primary | ButtonStyle::Ink => Weight::Semibold,
            _ => Weight::Medium,
        };
        let galley =
            ui.painter()
                .layout_no_wrap(self.text.to_owned(), theme::font(14.0, weight), color);
        let pad = if self.style == ButtonStyle::Link {
            4.0
        } else {
            16.0
        };
        let icon_size = 17.0;
        let gap = 8.0;
        let content = galley.size().x
            + self.icon.map_or(0.0, |_| icon_size + gap)
            + self.trailing.map_or(0.0, |_| icon_size + gap);
        let width = if self.full_width {
            ui.available_width()
        } else {
            content + pad * 2.0
        };
        let sense = if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, response) = ui.allocate_exact_size(vec2(width, self.height), sense);
        if ui.is_rect_visible(rect) {
            let hovered = self.enabled && response.hovered();
            let radius = if self.height >= 44.0 {
                12
            } else {
                theme::RADIUS_BUTTON
            };
            let (background, stroke) = match self.style {
                ButtonStyle::Primary => (
                    if hovered {
                        theme::accent_hover(dark)
                    } else {
                        accent
                    },
                    Stroke::NONE,
                ),
                ButtonStyle::Ink => (
                    if hovered {
                        ink_bg.gamma_multiply(0.86)
                    } else {
                        ink_bg
                    },
                    Stroke::NONE,
                ),
                ButtonStyle::Secondary => (
                    if hovered {
                        theme::hover(dark)
                    } else {
                        theme::surface(dark)
                    },
                    Stroke::new(1.0, theme::field_border(dark)),
                ),
                ButtonStyle::Ghost | ButtonStyle::Link => (
                    if hovered {
                        theme::hover(dark)
                    } else {
                        Color32::TRANSPARENT
                    },
                    Stroke::NONE,
                ),
            };
            if self.style == ButtonStyle::Primary && self.enabled {
                // Halo bleuté sous le bouton principal.
                ui.painter().add(
                    egui::epaint::Shadow {
                        offset: [0, 6],
                        blur: 14,
                        spread: 0,
                        color: Color32::from_rgba_unmultiplied(
                            35,
                            64,
                            196,
                            if dark { 0 } else { 60 },
                        ),
                    }
                    .as_shape(rect, radius),
                );
            }
            ui.painter()
                .rect(rect, radius, background, stroke, StrokeKind::Inside);
            let mut x = if self.full_width {
                rect.center().x - content / 2.0
            } else {
                rect.left() + pad
            };
            if let Some(icon) = self.icon {
                let glyph = Rect::from_center_size(
                    pos2(x + icon_size / 2.0, rect.center().y),
                    Vec2::splat(icon_size),
                );
                icons::paint(ui.painter(), glyph, icon, color);
                x += icon_size + gap;
            }
            let text_width = galley.size().x;
            let y = rect.center().y - galley.size().y / 2.0;
            ui.painter().galley(pos2(x, y), galley, color);
            x += text_width + gap;
            if let Some(icon) = self.trailing {
                let glyph = Rect::from_center_size(
                    pos2(x + icon_size / 2.0, rect.center().y),
                    Vec2::splat(icon_size - 1.0),
                );
                icons::paint(ui.painter(), glyph, icon, color);
            }
        }
        response.widget_info(|| WidgetInfo::labeled(Role::Button, self.enabled, self.text));
        clickable(response, self.enabled)
    }
}

/// Pastille de couleur ronde dans une cible d'au moins 36 px ; la couleur choisie est entourée
/// d'un anneau de sa propre couleur, séparé par un liseré de la couleur du fond.
pub fn color_dot(
    ui: &mut Ui,
    color: Color32,
    diameter: f32,
    selected: bool,
    label: &str,
) -> Response {
    let dark = ui.visuals().dark_mode;
    let target = (diameter + 12.0).max(36.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(target), Sense::click());
    if ui.is_rect_visible(rect) {
        let center = rect.center();
        let radius = diameter / 2.0;
        if response.hovered() && !selected {
            ui.painter()
                .circle_filled(center, radius + 4.0, theme::hover(dark));
        }
        ui.painter().circle_filled(center, radius, color);
        if theme::contrast(color, theme::surface(dark)) < 1.3 {
            ui.painter()
                .circle_stroke(center, radius, Stroke::new(1.0, theme::border(dark)));
        }
        if selected {
            let ring = if theme::contrast(color, theme::surface(dark)) < 1.3 {
                theme::label_secondary(dark)
            } else {
                color
            };
            ui.painter()
                .circle_stroke(center, radius + 3.5, Stroke::new(2.0, ring));
        }
    }
    response.widget_info(|| WidgetInfo::selected(Role::Button, true, selected, label));
    clickable(response.on_hover_text(label), true)
}

/// Touche de clavier (« Ctrl K ») peinte à droite de `right` : renvoie son rectangle.
pub fn kbd(painter: &Painter, right: Pos2, text: &str, dark: bool, on_dark: bool) -> Rect {
    let (fill, border, color) = if on_dark {
        (
            Color32::from_white_alpha(28),
            Color32::TRANSPARENT,
            Color32::from_rgb(0xE3, 0xE5, 0xEA),
        )
    } else {
        (
            theme::chip(dark),
            theme::border(dark),
            theme::label_secondary(dark),
        )
    };
    let galley = painter.layout_no_wrap(text.to_owned(), theme::font(11.0, Weight::Medium), color);
    let size = galley.size() + vec2(12.0, 5.0);
    let rect = Rect::from_min_size(pos2(right.x - size.x, right.y - size.y / 2.0), size);
    painter.rect(
        rect,
        5.0,
        fill,
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );
    painter.galley(rect.min + vec2(6.0, 2.5), galley, color);
    rect
}

/// Touche de clavier peinte à droite de `left` : renvoie son rectangle.
pub fn kbd_left(painter: &Painter, left: Pos2, text: &str, dark: bool) -> Rect {
    let width = painter
        .layout_no_wrap(
            text.to_owned(),
            theme::font(11.0, Weight::Medium),
            Color32::WHITE,
        )
        .size()
        .x
        + 12.0;
    kbd(painter, pos2(left.x + width, left.y), text, dark, false)
}

/// Titre de section en petites capitales espacées (« MATIÈRES »).
pub fn caps_job(text: &str, color: Color32, size: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.append(
        &text.to_uppercase(),
        0.0,
        TextFormat {
            font_id: theme::font(size, Weight::Semibold),
            color,
            extra_letter_spacing: size * 0.06,
            ..Default::default()
        },
    );
    job
}

pub fn section_caps(ui: &mut Ui, text: &str) -> Response {
    let dark = ui.visuals().dark_mode;
    ui.label(caps_job(text, theme::label_secondary(dark), 12.0))
}

/// Points d'un rectangle arrondi (sens horaire), pour les tracés en pointillés et les rotations.
pub fn rounded_rect_points(rect: Rect, radius: CornerRadius) -> Vec<Pos2> {
    let mut pts = Vec::new();
    let corners = [
        (
            pos2(rect.max.x, rect.min.y),
            f32::from(radius.ne),
            -90.0_f32,
        ),
        (rect.max, f32::from(radius.se), 0.0),
        (pos2(rect.min.x, rect.max.y), f32::from(radius.sw), 90.0),
        (rect.min, f32::from(radius.nw), 180.0),
    ];
    for (corner, r, start) in corners {
        let r = r.min(rect.width() / 2.0).min(rect.height() / 2.0);
        let center = match start as i32 {
            -90 => corner + vec2(-r, r),
            0 => corner + vec2(-r, -r),
            90 => corner + vec2(r, -r),
            _ => corner + vec2(r, r),
        };
        for i in 0..=6 {
            let a = (start + 90.0 * i as f32 / 6.0).to_radians();
            pts.push(center + vec2(a.cos(), a.sin()) * r);
        }
    }
    pts
}

/// Cadre en pointillés (zones de dépôt, cases « Nouveau cahier »).
pub fn dashed_rect(painter: &Painter, rect: Rect, radius: impl Into<CornerRadius>, stroke: Stroke) {
    let mut pts = rounded_rect_points(rect, radius.into());
    if let Some(first) = pts.first().copied() {
        pts.push(first);
    }
    painter.extend(Shape::dashed_line(&pts, stroke, 6.0, 4.0));
}

/// Puce de filtre (« Tous », une matière) : sombre quand elle est choisie.
pub fn chip(
    ui: &mut Ui,
    text: &str,
    dot: Option<Color32>,
    selected: bool,
    height: f32,
) -> Response {
    let dark = ui.visuals().dark_mode;
    let (ink_bg, ink_fg) = theme::ink_button(dark);
    let color = if selected { ink_fg } else { theme::icon(dark) };
    let font = theme::font(
        13.0,
        if selected {
            Weight::Medium
        } else {
            Weight::Regular
        },
    );
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    let dot_width = if dot.is_some() { 15.0 } else { 0.0 };
    let size = vec2(galley.size().x + dot_width + 28.0, height);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let (fill, stroke) = if selected {
            (ink_bg, Stroke::NONE)
        } else if response.hovered() {
            (theme::hover(dark), Stroke::new(1.0, theme::border(dark)))
        } else {
            (theme::surface(dark), Stroke::new(1.0, theme::border(dark)))
        };
        ui.painter()
            .rect(rect, height / 2.0, fill, stroke, StrokeKind::Inside);
        let mut x = rect.left() + 14.0;
        if let Some(dot) = dot {
            ui.painter().rect_filled(
                Rect::from_center_size(pos2(x + 4.0, rect.center().y), Vec2::splat(8.0)),
                2.0,
                dot,
            );
            x += dot_width;
        }
        let y = rect.center().y - galley.size().y / 2.0;
        ui.painter().galley(pos2(x, y), galley, color);
    }
    response.widget_info(|| WidgetInfo::selected(Role::Button, true, selected, text));
    clickable(response, true)
}

/// Sélecteur à deux segments ou plus (« Pages / Aperçu », « Trait entier / Zone »).
pub fn segmented(ui: &mut Ui, options: &[&str], selected: &mut usize) -> bool {
    let dark = ui.visuals().dark_mode;
    let width = ui.available_width();
    let (outer, _) = ui.allocate_exact_size(vec2(width, 38.0), Sense::hover());
    let track = if dark {
        Color32::from_rgb(0x23, 0x25, 0x2D)
    } else {
        Color32::from_rgb(0xED, 0xEF, 0xF3)
    };
    ui.painter().rect_filled(outer, 10.0, track);
    let inner = outer.shrink(3.0);
    let n = options.len().max(1) as f32;
    let seg = inner.width() / n;
    let mut changed = false;
    for (i, text) in options.iter().enumerate() {
        let rect = Rect::from_min_size(
            pos2(inner.left() + seg * i as f32, inner.top()),
            vec2(seg, inner.height()),
        );
        let response = ui.interact(
            rect,
            ui.scope_id().with(("segment", i, *text)),
            Sense::click(),
        );
        let active = *selected == i;
        if active {
            ui.painter().add(
                egui::epaint::Shadow {
                    offset: [0, 1],
                    blur: 3,
                    spread: 0,
                    color: Color32::from_black_alpha(20),
                }
                .as_shape(rect, 8),
            );
            ui.painter().rect_filled(rect, 8.0, theme::surface(dark));
        }
        let (font, color) = if active {
            (theme::font(13.0, Weight::Semibold), theme::label(dark))
        } else {
            (
                theme::font(13.0, Weight::Regular),
                theme::label_secondary(dark),
            )
        };
        ui.painter()
            .text(rect.center(), Align2::CENTER_CENTER, *text, font, color);
        response.widget_info(|| WidgetInfo::selected(Role::Button, true, active, *text));
        if response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
            && !active
        {
            *selected = i;
            changed = true;
        }
    }
    changed
}

/// Champ de saisie haut de 46 px (feuilles) ou plus bas, à bordure de champ et anneau d'accent au
/// focus.
pub fn text_field(ui: &mut Ui, text: &mut String, hint: &str, height: f32, size: f32) -> Response {
    let dark = ui.visuals().dark_mode;
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let id = ui.scope_id().with(("champ", hint));
    let focused = ui.memory(|m| m.has_focus(id));
    let border = if focused {
        theme::accent(dark)
    } else {
        theme::field_border(dark)
    };
    if focused {
        let a = theme::accent(dark);
        ui.painter().rect_stroke(
            rect.expand(2.0),
            14.0,
            Stroke::new(
                3.0,
                Color32::from_rgba_unmultiplied(a.r(), a.g(), a.b(), 40),
            ),
            StrokeKind::Outside,
        );
    }
    ui.painter().rect(
        rect,
        12.0,
        theme::surface(dark),
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );
    let inner = rect.shrink2(vec2(14.0, 0.0));
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.add(
        egui::TextEdit::singleline(text)
            .id(id)
            .frame(egui::Frame::NONE)
            .font(theme::font(size, Weight::Regular))
            .hint_text(hint)
            .desired_width(inner.width()),
    )
}

/// Ligne de séparation verticale (entre groupes d'une barre).
pub fn vertical_separator(ui: &mut Ui, height: f32, margin: f32) {
    let dark = ui.visuals().dark_mode;
    let (rect, _) = ui.allocate_exact_size(vec2(1.0 + margin * 2.0, height), Sense::hover());
    ui.painter().line_segment(
        [rect.center_top(), rect.center_bottom()],
        Stroke::new(1.0, theme::border(dark)),
    );
}

/// Texte peint, tronqué avec « … » s'il dépasse `max_width`.
pub fn truncated(
    painter: &Painter,
    pos: Pos2,
    anchor: Align2,
    text: &str,
    font: FontId,
    color: Color32,
    max_width: f32,
) -> Rect {
    let mut job = LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_width.max(1.0));
    let galley = painter.layout_job(job);
    let rect = anchor.anchor_size(pos, galley.size());
    painter.galley(rect.min, galley, color);
    rect
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounded_rect_points_stay_inside_the_rectangle() {
        let rect = Rect::from_min_size(pos2(10.0, 20.0), vec2(100.0, 140.0));
        let pts = rounded_rect_points(rect, theme::cover_radius(1.0));
        assert_eq!(pts.len(), 28);
        for p in pts {
            assert!(rect.expand(0.01).contains(p), "{p:?}");
        }
    }
}
