//! Apparence générale : couleurs, arrondis et espacements inspirés des interfaces Apple.
//!
//! Les couleurs suivent les « couleurs système » d'Apple (bleu d'action, fonds groupés, séparateurs
//! fins) et existent en clair et en sombre.

use calame_core::Color;
use egui::{Color32, CornerRadius, Stroke, TextStyle};

/// Bleu système : actions principales et sélection.
pub const ACCENT: Color32 = Color32::from_rgb(0, 122, 255);
pub const ACCENT_DARK: Color32 = Color32::from_rgb(10, 132, 255);

/// Couleurs proposées pour une matière ou un cahier.
pub const COVER_COLORS: [Color; 12] = [
    Color::rgb(255, 59, 48),
    Color::rgb(255, 149, 0),
    Color::rgb(255, 204, 0),
    Color::rgb(52, 199, 89),
    Color::rgb(0, 199, 190),
    Color::rgb(48, 176, 199),
    Color::rgb(0, 122, 255),
    Color::rgb(88, 86, 214),
    Color::rgb(175, 82, 222),
    Color::rgb(255, 45, 85),
    Color::rgb(162, 132, 94),
    Color::rgb(142, 142, 147),
];

/// Couleur neutre que l'enregistrement donne aux cahiers et aux dossiers : on la remplace à
/// l'affichage par la couleur de la matière.
pub const LEGACY_NEUTRAL: Color = Color::rgb(128, 128, 128);

pub fn accent(dark: bool) -> Color32 {
    if dark {
        ACCENT_DARK
    } else {
        ACCENT
    }
}

pub fn to_color32(color: Color) -> Color32 {
    Color32::from_rgb(color.r, color.g, color.b)
}

/// Fond principal des écrans de lecture (bibliothèque, panneaux).
pub fn grouped_background(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(0, 0, 0)
    } else {
        Color32::from_rgb(242, 242, 247)
    }
}

/// Surface d'une carte ou d'une barre posée sur le fond groupé.
pub fn surface(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(28, 28, 30)
    } else {
        Color32::from_rgb(255, 255, 255)
    }
}

/// Surface secondaire : champs, éléments survolés.
pub fn fill_secondary(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgba_unmultiplied(120, 120, 128, 80)
    } else {
        Color32::from_rgba_unmultiplied(120, 120, 128, 36)
    }
}

pub fn fill_hover(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgba_unmultiplied(255, 255, 255, 22)
    } else {
        Color32::from_rgba_unmultiplied(0, 0, 0, 14)
    }
}

pub fn separator(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgba_unmultiplied(255, 255, 255, 32)
    } else {
        Color32::from_rgba_unmultiplied(60, 60, 67, 46)
    }
}

pub fn label(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(245, 245, 247)
    } else {
        Color32::from_rgb(28, 28, 30)
    }
}

pub fn label_secondary(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(152, 152, 159)
    } else {
        Color32::from_rgb(99, 99, 102)
    }
}

fn visuals(dark: bool) -> egui::Visuals {
    let mut v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    let accent = accent(dark);
    v.panel_fill = surface(dark);
    v.window_fill = surface(dark);
    v.faint_bg_color = fill_secondary(dark);
    v.extreme_bg_color = if dark {
        Color32::from_rgb(44, 44, 46)
    } else {
        Color32::from_rgb(235, 235, 240)
    };
    v.window_corner_radius = CornerRadius::same(14);
    v.menu_corner_radius = CornerRadius::same(10);
    v.window_stroke = Stroke::new(1.0, separator(dark));
    v.hyperlink_color = accent;
    v.selection.bg_fill = Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 64);
    v.selection.stroke = Stroke::new(1.5, accent);

    let radius = CornerRadius::same(8);
    let widgets = &mut v.widgets;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, separator(dark));
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, label(dark));
    for w in [
        &mut widgets.noninteractive,
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
        &mut widgets.open,
    ] {
        w.corner_radius = radius;
    }
    widgets.inactive.weak_bg_fill = fill_secondary(dark);
    widgets.inactive.bg_fill = fill_secondary(dark);
    widgets.inactive.bg_stroke = Stroke::NONE;
    widgets.inactive.fg_stroke = Stroke::new(1.0, label(dark));
    widgets.hovered.weak_bg_fill = fill_hover(dark);
    widgets.hovered.bg_fill = fill_hover(dark);
    widgets.hovered.bg_stroke = Stroke::NONE;
    widgets.hovered.fg_stroke = Stroke::new(1.0, label(dark));
    widgets.hovered.expansion = 0.0;
    widgets.active.weak_bg_fill = fill_secondary(dark);
    widgets.active.bg_fill = fill_secondary(dark);
    widgets.active.bg_stroke = Stroke::NONE;
    widgets.active.fg_stroke = Stroke::new(1.0, label(dark));
    widgets.active.expansion = 0.0;
    v
}

/// Applique l'apparence clair ou sombre, quelle que soit celle du système.
pub fn apply(ctx: &egui::Context, dark: bool) {
    ctx.set_theme(if dark {
        egui::ThemePreference::Dark
    } else {
        egui::ThemePreference::Light
    });
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        style.spacing.interact_size.y = 30.0;
        style.spacing.menu_margin = egui::Margin::same(6);
        style.spacing.indent = 16.0;
        style.text_styles.insert(
            TextStyle::Body,
            egui::FontId::proportional(14.0),
        );
        style.text_styles.insert(
            TextStyle::Button,
            egui::FontId::proportional(14.0),
        );
        style.text_styles.insert(
            TextStyle::Small,
            egui::FontId::proportional(12.0),
        );
        style.text_styles.insert(
            TextStyle::Heading,
            egui::FontId::proportional(22.0),
        );
    });
    ctx.set_visuals_of(egui::Theme::Dark, visuals(true));
    ctx.set_visuals_of(egui::Theme::Light, visuals(false));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_colors_are_distinct_and_opaque() {
        for (i, a) in COVER_COLORS.iter().enumerate() {
            assert_eq!(a.a, 255);
            for b in &COVER_COLORS[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }
}
