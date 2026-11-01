//! Apparence générale « l'encre d'abord » : palette, polices, rayons et ombres.
//!
//! Une encre bleue pour les actions, un safran pour ce qui doit ressortir, des gris légèrement
//! bleutés pour le reste. Chaque couleur existe en clair et en sombre (mode concentration et
//! thème sombre).

use std::sync::Arc;

use calame_core::Color;
use egui::epaint::Shadow;
use egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle,
};

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// Encre : actions principales, élément actif, sélection.
pub const ACCENT: Color32 = rgb(0x23, 0x40, 0xC4);
pub const ACCENT_HOVER: Color32 = rgb(0x1A, 0x30, 0x99);
pub const ACCENT_DARK: Color32 = rgb(0x9D, 0xB0, 0xFF);
/// Safran : surligneur, mise en avant, correspondances de recherche.
pub const SAFFRON: Color32 = rgb(0xF4, 0xB7, 0x40);
/// Fond du bouton principal sombre (« Continuer à écrire », « Exporter », outil actif).
pub const INK_BUTTON: Color32 = rgb(0x15, 0x17, 0x1C);

/// Couleurs de couverture d'une matière ou d'un cahier, avec leur nom.
pub const COVER_COLORS: [Color; 8] = [
    Color::rgb(0x23, 0x40, 0xC4),
    Color::rgb(0x15, 0x17, 0x1C),
    Color::rgb(0xC2, 0x4E, 0x3A),
    Color::rgb(0xE9, 0xA9, 0x3A),
    Color::rgb(0x2F, 0x7A, 0x5C),
    Color::rgb(0x7A, 0x4F, 0xC4),
    Color::rgb(0x16, 0x76, 0x76),
    Color::rgb(0xD8, 0xCB, 0xB0),
];

/// Clés de traduction des noms de couverture, dans l'ordre de `COVER_COLORS`.
pub const COVER_NAMES: [&str; 8] = [
    "cover-ink",
    "cover-night",
    "cover-brick",
    "cover-saffron",
    "cover-sage",
    "cover-plum",
    "cover-lagoon",
    "cover-kraft",
];

/// Couleur neutre que l'enregistrement donne aux cahiers et aux dossiers : on la remplace à
/// l'affichage par la couleur de la matière.
pub const LEGACY_NEUTRAL: Color = Color::rgb(128, 128, 128);

fn pick(dark: bool, light: Color32, dark_value: Color32) -> Color32 {
    if dark {
        dark_value
    } else {
        light
    }
}

pub fn accent(dark: bool) -> Color32 {
    pick(dark, ACCENT, ACCENT_DARK)
}

pub fn accent_hover(dark: bool) -> Color32 {
    pick(dark, ACCENT_HOVER, rgb(0xB9, 0xC7, 0xFF))
}

/// Fond de l'élément actif (navigation, page ouverte, résultat choisi).
pub fn accent_soft(dark: bool) -> Color32 {
    pick(dark, rgb(0xE6, 0xEA, 0xFA), rgb(0x26, 0x2D, 0x4A))
}

pub fn saffron_soft(dark: bool) -> Color32 {
    pick(dark, rgb(0xFF, 0xF4, 0xDC), rgb(0x3A, 0x2F, 0x17))
}

/// Texte posé sur `saffron_soft`.
pub fn on_saffron(dark: bool) -> Color32 {
    pick(dark, rgb(0x7A, 0x4D, 0x00), SAFFRON)
}

/// Fond de l'application (bibliothèque).
pub fn app_background(dark: bool) -> Color32 {
    pick(dark, rgb(0xF4, 0xF5, 0xF7), rgb(0x0F, 0x10, 0x14))
}

pub fn sidebar(dark: bool) -> Color32 {
    pick(dark, rgb(0xFA, 0xFB, 0xFC), rgb(0x14, 0x16, 0x1B))
}

/// Surface des cartes, barres et du dock.
pub fn surface(dark: bool) -> Color32 {
    pick(dark, Color32::WHITE, rgb(0x1C, 0x1E, 0x25))
}

/// Bureau autour de la page, et ses points.
pub fn desk(dark: bool) -> Color32 {
    pick(dark, rgb(0xE9, 0xEB, 0xEF), rgb(0x0F, 0x10, 0x14))
}

pub fn desk_dots(dark: bool) -> Color32 {
    pick(dark, rgb(0xD9, 0xDC, 0xE3), rgb(0x1C, 0x1E, 0x25))
}

pub fn paper(dark: bool) -> Color32 {
    pick(dark, rgb(0xFF, 0xFE, 0xFB), rgb(0x17, 0x19, 0x1F))
}

/// Quadrillage, lignes et points du papier.
pub fn paper_grid(dark: bool) -> Color32 {
    pick(dark, rgb(0xDC, 0xE1, 0xEC), rgb(0x2A, 0x2C, 0x32))
}

pub fn border(dark: bool) -> Color32 {
    pick(dark, rgb(0xE2, 0xE5, 0xEB), rgb(0x2A, 0x2C, 0x33))
}

/// Bordure des champs et des boutons secondaires.
pub fn field_border(dark: bool) -> Color32 {
    pick(dark, rgb(0xD5, 0xD9, 0xE2), rgb(0x36, 0x39, 0x44))
}

/// Bordure en pointillés des zones de dépôt et des cases « nouveau ».
pub fn dashed(dark: bool) -> Color32 {
    pick(dark, rgb(0xC3, 0xC9, 0xD6), rgb(0x3D, 0x41, 0x4C))
}

pub fn hover(dark: bool) -> Color32 {
    pick(dark, rgb(0xEE, 0xF0, 0xF4), rgb(0x26, 0x28, 0x30))
}

/// Fond discret d'une touche (`kbd`), d'une pastille d'icône ou d'un champ secondaire.
pub fn chip(dark: bool) -> Color32 {
    pick(dark, rgb(0xF1, 0xF2, 0xF5), rgb(0x23, 0x25, 0x2D))
}

pub fn label(dark: bool) -> Color32 {
    pick(dark, rgb(0x15, 0x17, 0x1C), rgb(0xE7, 0xE9, 0xEF))
}

/// Couleur des icônes et du texte de navigation.
pub fn icon(dark: bool) -> Color32 {
    pick(dark, rgb(0x2B, 0x2F, 0x38), rgb(0xC9, 0xCD, 0xD6))
}

pub fn label_secondary(dark: bool) -> Color32 {
    pick(dark, rgb(0x5B, 0x61, 0x6E), rgb(0x9C, 0xA2, 0xB0))
}

/// Icône ou texte inactif (rétablir sans rien à rétablir).
pub fn disabled(dark: bool) -> Color32 {
    pick(dark, rgb(0x9A, 0xA0, 0xAC), rgb(0x5A, 0x5F, 0x6B))
}

/// « Enregistré ».
pub fn success(dark: bool) -> Color32 {
    pick(dark, rgb(0x2F, 0x7A, 0x5C), rgb(0x6C, 0xC5, 0x9F))
}

pub fn danger(dark: bool) -> Color32 {
    pick(dark, rgb(0xC2, 0x3B, 0x26), rgb(0xFF, 0x9A, 0x86))
}

/// Bouton principal sombre : fond et texte.
pub fn ink_button(dark: bool) -> (Color32, Color32) {
    if dark {
        (rgb(0xE7, 0xE9, 0xEF), INK_BUTTON)
    } else {
        (INK_BUTTON, Color32::WHITE)
    }
}

pub fn to_color32(color: Color) -> Color32 {
    Color32::from_rgb(color.r, color.g, color.b)
}

// ---- Contraste ---------------------------------------------------------------------------

fn channel(c: u8) -> f32 {
    let c = f32::from(c) / 255.0;
    if c <= 0.039_28 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Luminance relative (WCAG 2).
pub fn luminance(color: Color32) -> f32 {
    0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
}

/// Rapport de contraste entre deux couleurs (de 1 à 21).
pub fn contrast(a: Color32, b: Color32) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Texte lisible sur une couverture : blanc, ou encre de nuit si le blanc n'atteint pas 4,5:1.
pub fn on_color(color: Color32) -> Color32 {
    if contrast(Color32::WHITE, color) >= 4.5
        || contrast(Color32::WHITE, color) >= contrast(INK_BUTTON, color)
    {
        Color32::WHITE
    } else {
        INK_BUTTON
    }
}

/// Couleur d'affichage d'une encre sur le papier sombre : les encres foncées sont éclaircies (même
/// teinte), sans toucher aux données de la page. Sur papier clair, l'encre est inchangée.
pub fn display_ink(color: Color32, dark: bool) -> Color32 {
    if !dark || contrast(color, paper(true)) >= 4.5 {
        return color;
    }
    let (r, g, b) = (
        f32::from(color.r()) / 255.0,
        f32::from(color.g()) / 255.0,
        f32::from(color.b()) / 255.0,
    );
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d < 0.08 {
        // Noirs et gris : le texte clair du thème sombre.
        let base = label(true);
        return Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), color.a());
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } * 60.0;
    let (r, g, b) = hsl_to_rgb(h, s.min(1.0), 0.72 + 0.12 * l);
    Color32::from_rgba_unmultiplied(r, g, b, color.a())
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let to = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (to(r), to(g), to(b))
}

// ---- Formes ----------------------------------------------------------------------------

pub const RADIUS_BUTTON: u8 = 10;
pub const RADIUS_CARD: u8 = 22;
pub const RADIUS_SHEET: u8 = 26;
pub const RADIUS_DOCK: u8 = 20;
pub const RADIUS_DOCK_ITEM: u8 = 13;

/// Couverture : dos à gauche, peu arrondi ; tranche à droite, plus arrondie.
pub fn cover_radius(scale: f32) -> CornerRadius {
    let small = (6.0 * scale).round() as u8;
    let large = (14.0 * scale).round() as u8;
    CornerRadius {
        nw: small,
        sw: small,
        ne: large,
        se: large,
    }
}

pub fn card_shadow(dark: bool) -> Shadow {
    Shadow {
        offset: [0, 10],
        blur: 26,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 90 } else { 20 }),
    }
}

pub fn dock_shadow(dark: bool) -> Shadow {
    Shadow {
        offset: [0, 14],
        blur: 32,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 150 } else { 46 }),
    }
}

pub fn window_shadow() -> Shadow {
    Shadow {
        offset: [0, 28],
        blur: 70,
        spread: 0,
        color: Color32::from_black_alpha(110),
    }
}

/// Voile derrière une feuille ou la palette (`#15171C` à 55 %).
pub fn veil() -> Color32 {
    Color32::from_rgba_unmultiplied(0x15, 0x17, 0x1C, 140)
}

// ---- Polices ---------------------------------------------------------------------------

/// Graisse de l'interface (Instrument Sans).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    Regular,
    Medium,
    Semibold,
}

const MEDIUM: &str = "calame-medium";
const SEMIBOLD: &str = "calame-semibold";
const DISPLAY: &str = "calame-display";

pub fn font(size: f32, weight: Weight) -> FontId {
    let family = match weight {
        Weight::Regular => FontFamily::Proportional,
        Weight::Medium => FontFamily::Name(MEDIUM.into()),
        Weight::Semibold => FontFamily::Name(SEMIBOLD.into()),
    };
    FontId::new(size, family)
}

/// Titres (Bricolage Grotesque, graisse 700).
pub fn display(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(DISPLAY.into()))
}

/// Instrument Sans (400, 500, 600) pour l'interface et Bricolage Grotesque (700) pour les titres,
/// toutes deux sous licence OFL (voir `assets/fonts`). Les polices d'egui restent en secours pour
/// les caractères absents (flèches, symboles).
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let files: [(&str, &'static [u8]); 4] = [
        (
            "InstrumentSans-400",
            include_bytes!("../../../assets/fonts/InstrumentSans-400.ttf"),
        ),
        (
            "InstrumentSans-500",
            include_bytes!("../../../assets/fonts/InstrumentSans-500.ttf"),
        ),
        (
            "InstrumentSans-600",
            include_bytes!("../../../assets/fonts/InstrumentSans-600.ttf"),
        ),
        (
            "BricolageGrotesque-700",
            include_bytes!("../../../assets/fonts/BricolageGrotesque-700.ttf"),
        ),
    ];
    for (name, bytes) in files {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    }
    let fallbacks = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let family = |first: &str| {
        let mut list = vec![first.to_owned()];
        list.extend(fallbacks.iter().cloned());
        list
    };
    fonts
        .families
        .insert(FontFamily::Proportional, family("InstrumentSans-400"));
    fonts.families.insert(
        FontFamily::Name(MEDIUM.into()),
        family("InstrumentSans-500"),
    );
    fonts.families.insert(
        FontFamily::Name(SEMIBOLD.into()),
        family("InstrumentSans-600"),
    );
    fonts.families.insert(
        FontFamily::Name(DISPLAY.into()),
        family("BricolageGrotesque-700"),
    );
    ctx.set_fonts(fonts);
}

// ---- Style egui ------------------------------------------------------------------------

fn visuals(dark: bool) -> egui::Visuals {
    let mut v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    let accent = accent(dark);
    v.panel_fill = surface(dark);
    v.window_fill = surface(dark);
    v.faint_bg_color = chip(dark);
    v.extreme_bg_color = surface(dark);
    v.code_bg_color = chip(dark);
    v.window_corner_radius = CornerRadius::same(16);
    v.menu_corner_radius = CornerRadius::same(12);
    v.window_stroke = Stroke::new(1.0, border(dark));
    v.window_shadow = dock_shadow(dark);
    v.popup_shadow = dock_shadow(dark);
    v.hyperlink_color = accent;
    v.selection.bg_fill = Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 56);
    v.selection.stroke = Stroke::new(1.5, accent);
    v.text_cursor.stroke = Stroke::new(2.0, accent);

    let radius = CornerRadius::same(RADIUS_BUTTON);
    let widgets = &mut v.widgets;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, border(dark));
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, label(dark));
    for w in [
        &mut widgets.noninteractive,
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
        &mut widgets.open,
    ] {
        w.corner_radius = radius;
        w.expansion = 0.0;
    }
    widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    widgets.inactive.bg_fill = chip(dark);
    widgets.inactive.bg_stroke = Stroke::new(1.0, field_border(dark));
    widgets.inactive.fg_stroke = Stroke::new(1.0, label(dark));
    widgets.hovered.weak_bg_fill = hover(dark);
    widgets.hovered.bg_fill = hover(dark);
    widgets.hovered.bg_stroke = Stroke::new(1.0, field_border(dark));
    widgets.hovered.fg_stroke = Stroke::new(1.0, label(dark));
    widgets.active.weak_bg_fill = accent_soft(dark);
    widgets.active.bg_fill = accent_soft(dark);
    widgets.active.bg_stroke = Stroke::new(1.0, accent);
    widgets.active.fg_stroke = Stroke::new(1.0, label(dark));
    widgets.open.weak_bg_fill = hover(dark);
    widgets.open.bg_fill = hover(dark);
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
        style.spacing.button_padding = egui::vec2(12.0, 7.0);
        style.spacing.interact_size.y = 32.0;
        style.spacing.menu_margin = egui::Margin::same(8);
        style.spacing.indent = 16.0;
        style
            .text_styles
            .insert(TextStyle::Body, font(14.0, Weight::Regular));
        style
            .text_styles
            .insert(TextStyle::Button, font(14.0, Weight::Medium));
        style
            .text_styles
            .insert(TextStyle::Small, font(12.0, Weight::Regular));
        style.text_styles.insert(TextStyle::Heading, display(22.0));
    });
    ctx.set_visuals_of(egui::Theme::Dark, visuals(true));
    ctx.set_visuals_of(egui::Theme::Light, visuals(false));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_colors_are_distinct_and_opaque() {
        assert_eq!(COVER_COLORS.len(), COVER_NAMES.len());
        for (i, a) in COVER_COLORS.iter().enumerate() {
            assert_eq!(a.a, 255);
            for b in &COVER_COLORS[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn cover_text_is_white_except_on_saffron_and_kraft() {
        for (color, name) in COVER_COLORS.iter().zip(COVER_NAMES) {
            let ink = on_color(to_color32(*color));
            let expected = if matches!(name, "cover-saffron" | "cover-kraft") {
                INK_BUTTON
            } else {
                Color32::WHITE
            };
            assert_eq!(ink, expected, "{name}");
        }
    }

    #[test]
    fn cover_text_reaches_the_minimal_contrast_on_light_covers() {
        for color in [rgb(255, 204, 0), rgb(0xD8, 0xCB, 0xB0), rgb(250, 250, 250)] {
            assert!(contrast(on_color(color), color) >= 4.5);
        }
    }

    #[test]
    fn dark_inks_are_lightened_on_dark_paper_and_kept_on_light_paper() {
        let blue = rgb(0x23, 0x40, 0xC4);
        assert_eq!(display_ink(blue, false), blue);
        let shown = display_ink(blue, true);
        assert!(contrast(shown, paper(true)) >= 4.5);
        assert!(shown.b() > shown.r(), "la teinte bleue est conservée");
        assert_eq!(display_ink(Color32::BLACK, true), label(true));
        let yellow = SAFFRON;
        assert_eq!(display_ink(yellow, true), yellow);
    }
}
