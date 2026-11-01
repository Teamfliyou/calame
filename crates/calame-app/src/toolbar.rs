//! Barre d'outils d'un cahier, à la manière d'une barre Apple : retour et titre à gauche, outils
//! au centre en une capsule d'icônes, annuler et rétablir à droite. Une seconde ligne, discrète,
//! ne montre que les réglages de l'outil actif (couleurs, épaisseur, actions sur la sélection).

use calame_core::{Color, InstrumentKind};
use egui::{vec2, Align, Layout, RichText, Sense, Ui};

use crate::i18n::t;
use crate::icons::{self, ButtonStyle, Icon};
use crate::theme;
use crate::tools::{Tool, ToolSettings, HIGHLIGHTER_WIDTHS, PALETTE, PEN_WIDTHS};

/// Ce que la barre d'outils demande à l'application (le reste est modifié directement).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Undo,
    Redo,
    ResetView,
    SetDark(bool),
    Copy,
    Cut,
    Paste,
    Delete,
    /// Applique la couleur courante à la sélection.
    Recolor,
    /// Les favoris ont changé : à enregistrer.
    FavoritesChanged,
    /// La reconnaissance automatique de formes a été activée ou désactivée : à enregistrer.
    ShapesChanged,
    AddInstrument(InstrumentKind),
    RemoveInstrument,
    ClearInstruments,
    /// Quitter le cahier pour revenir à la bibliothèque.
    BackToLibrary,
    ToggleSidebar,
}

pub struct ToolbarState<'a> {
    pub can_undo: bool,
    pub can_redo: bool,
    pub has_selection: bool,
    pub can_paste: bool,
    pub has_active_instrument: bool,
    pub instrument_count: usize,
    pub sidebar_visible: bool,
    /// Nom de la matière, affiché sur le bouton de retour.
    pub back_label: &'a str,
    pub notebook: &'a str,
}

const TOOLS: [(Tool, Icon, &str); 8] = [
    (Tool::Pen, Icon::Pen, "tool-pen"),
    (Tool::Highlighter, Icon::Highlighter, "tool-highlighter"),
    (Tool::EraserStroke, Icon::EraserStroke, "tool-eraser-stroke"),
    (Tool::EraserArea, Icon::EraserArea, "tool-eraser-area"),
    (Tool::Lasso, Icon::Lasso, "tool-lasso"),
    (Tool::Text, Icon::Text, "tool-text"),
    (Tool::Pipette, Icon::Pipette, "tool-pipette"),
    (Tool::Instruments, Icon::Ruler, "tool-instruments"),
];

pub fn show(ui: &mut Ui, tools: &mut ToolSettings, state: &ToolbarState<'_>) -> Vec<Command> {
    let mut commands = Vec::new();
    main_row(ui, tools, state, &mut commands);
    ui.add_space(2.0);
    context_row(ui, tools, state, &mut commands);
    commands
}

fn main_row(
    ui: &mut Ui,
    tools: &mut ToolSettings,
    state: &ToolbarState<'_>,
    commands: &mut Vec<Command>,
) {
    let dark = ui.visuals().dark_mode;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;

        if icons::text_button(
            ui,
            Some(Icon::ChevronLeft),
            state.back_label,
            ButtonStyle::Link,
            true,
        )
        .on_hover_text(t("back-to-library"))
        .clicked()
        {
            commands.push(Command::BackToLibrary);
        }
        if icons::icon_button(
            ui,
            Icon::Sidebar,
            t("toggle-sidebar"),
            state.sidebar_visible,
            true,
        )
        .clicked()
        {
            commands.push(Command::ToggleSidebar);
        }
        ui.scope(|ui| {
            ui.set_max_width(180.0);
            ui.add(egui::Label::new(RichText::new(state.notebook).strong()).truncate());
        });
        ui.add_space(14.0);

        egui::Frame::new()
            .fill(theme::fill_secondary(dark))
            .corner_radius(egui::CornerRadius::same(10))
            .inner_margin(egui::Margin::symmetric(4, 2))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for (tool, icon, key) in TOOLS {
                        let selected = tools.tool == tool;
                        if icons::icon_button(ui, icon, t(key), selected, true).clicked()
                            && !selected
                        {
                            if tool == Tool::Pipette {
                                tools.previous = tools.tool;
                            }
                            tools.tool = tool;
                        }
                    }
                    ui.add_space(2.0);
                    ui.separator();
                    ui.add_space(2.0);
                    if icons::icon_button(
                        ui,
                        Icon::Shapes,
                        t("shape-recognition"),
                        tools.shape_recognition,
                        true,
                    )
                    .on_hover_text(t("shape-recognition-hint"))
                    .clicked()
                    {
                        tools.shape_recognition = !tools.shape_recognition;
                        commands.push(Command::ShapesChanged);
                    }
                });
            });

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if icons::icon_button(ui, Icon::Recenter, t("reset-view"), false, true).clicked() {
                commands.push(Command::ResetView);
            }
            ui.add_space(4.0);
            if icons::icon_button(ui, Icon::Redo, t("redo"), false, state.can_redo).clicked() {
                commands.push(Command::Redo);
            }
            if icons::icon_button(ui, Icon::Undo, t("undo"), false, state.can_undo).clicked() {
                commands.push(Command::Undo);
            }
        });
    });
}

/// Diamètre de l'aperçu d'une épaisseur de trait.
fn dot_diameter(width: f32) -> f32 {
    (width.sqrt() * 3.2).clamp(3.0, 20.0)
}

/// Rangée de pastilles montrant l'épaisseur de chaque réglage prédéfini.
fn width_dots(ui: &mut Ui, presets: &[f32], value: &mut f32) {
    let dark = ui.visuals().dark_mode;
    for &w in presets {
        let selected = (*value - w).abs() < 0.01;
        let (rect, response) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::click());
        if ui.is_rect_visible(rect) {
            let accent = theme::accent(dark);
            if selected || response.hovered() {
                let background = if selected {
                    egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 40)
                } else {
                    theme::fill_hover(dark)
                };
                ui.painter().rect_filled(rect, 8.0, background);
            }
            let color = if selected {
                accent
            } else {
                theme::label(dark)
            };
            ui.painter()
                .circle_filled(rect.center(), dot_diameter(w) / 2.0, color);
        }
        let label = format!("{} {w:.0}", t("width"));
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
        if response
            .on_hover_text(label.as_str())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            *value = w;
        }
    }
}

fn slider(ui: &mut Ui, value: &mut f32, range: std::ops::RangeInclusive<f32>) {
    ui.scope(|ui| {
        ui.spacing_mut().slider_width = 120.0;
        ui.add(egui::Slider::new(value, range).show_value(false));
    });
}

fn caption(ui: &mut Ui, text: &str) {
    let dark = ui.visuals().dark_mode;
    ui.label(RichText::new(text).small().color(theme::label_secondary(dark)));
}

fn colors(ui: &mut Ui, tools: &mut ToolSettings, commands: &mut Vec<Command>) {
    for color in PALETTE {
        if icons::color_dot(
            ui,
            theme::to_color32(color),
            18.0,
            tools.color == color,
            &color.to_hex(),
        )
        .clicked()
        {
            tools.color = color;
        }
    }
    ui.separator();

    // Sélecteur complet : teinte, saturation, valeur (HSV).
    let mut rgb = [tools.color.r, tools.color.g, tools.color.b];
    if ui.color_edit_button_srgb(&mut rgb).changed() {
        tools.color = Color::rgb(rgb[0], rgb[1], rgb[2]);
    }
    let hex_edit = ui.add(
        egui::TextEdit::singleline(&mut tools.hex)
            .desired_width(64.0)
            .hint_text("#rrggbb"),
    );
    if hex_edit.changed() {
        if let Some(color) = Color::from_hex(&tools.hex) {
            tools.color = color;
        }
    }
    if !hex_edit.has_focus() {
        tools.hex = tools.color.to_hex();
    }

    if icons::icon_button(ui, Icon::Star, t("add-favorite"), false, true).clicked() {
        tools.add_favorite();
        commands.push(Command::FavoritesChanged);
    }
    let mut removed = None;
    for (i, color) in tools.favorites.clone().into_iter().enumerate() {
        let response = icons::color_dot(
            ui,
            theme::to_color32(color),
            18.0,
            tools.color == color,
            t("favorite-hint"),
        );
        if response.clicked() {
            tools.color = color;
        }
        if response.secondary_clicked() {
            removed = Some(i);
        }
    }
    if let Some(i) = removed {
        tools.favorites.remove(i);
        commands.push(Command::FavoritesChanged);
    }
}

fn context_row(
    ui: &mut Ui,
    tools: &mut ToolSettings,
    state: &ToolbarState<'_>,
    commands: &mut Vec<Command>,
) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 2.0);
        if matches!(
            tools.tool,
            Tool::Pen | Tool::Highlighter | Tool::Text | Tool::Lasso
        ) {
            colors(ui, tools, commands);
            ui.separator();
        }
        match tools.tool {
            Tool::Pen => {
                width_dots(ui, &PEN_WIDTHS, &mut tools.pen_width);
                slider(ui, &mut tools.pen_width, 0.5..=30.0);
            }
            Tool::Highlighter => {
                width_dots(ui, &HIGHLIGHTER_WIDTHS, &mut tools.highlighter_width);
                slider(ui, &mut tools.highlighter_width, 5.0..=80.0);
            }
            Tool::EraserStroke | Tool::EraserArea => {
                caption(ui, t("eraser-size"));
                slider(ui, &mut tools.eraser_size, 4.0..=80.0);
            }
            Tool::Text => {
                caption(ui, t("text-size"));
                slider(ui, &mut tools.text_size, 8.0..=96.0);
            }
            Tool::Pipette => caption(ui, t("pipette-hint")),
            Tool::Lasso => lasso_actions(ui, state, commands),
            Tool::Instruments => instrument_actions(ui, state, commands),
        }
    });
}

fn lasso_actions(ui: &mut Ui, state: &ToolbarState<'_>, commands: &mut Vec<Command>) {
    let sel = state.has_selection;
    let actions = [
        (Icon::Copy, sel, "copy", Command::Copy),
        (Icon::Cut, sel, "cut", Command::Cut),
        (Icon::Paste, state.can_paste, "paste", Command::Paste),
        (Icon::Recolor, sel, "recolor", Command::Recolor),
        (Icon::Trash, sel, "delete", Command::Delete),
    ];
    for (icon, enabled, key, command) in actions {
        if icons::icon_button(ui, icon, t(key), false, enabled).clicked() {
            commands.push(command);
        }
    }
    if !sel {
        caption(ui, t("lasso-hint"));
    }
}

const INSTRUMENT_BUTTONS: [(InstrumentKind, Icon, &str); 5] = [
    (InstrumentKind::Ruler, Icon::Ruler, "ruler"),
    (InstrumentKind::SetSquare45, Icon::SetSquare45, "set-square-45"),
    (InstrumentKind::SetSquare60, Icon::SetSquare60, "set-square-60"),
    (InstrumentKind::Protractor, Icon::Protractor, "protractor"),
    (InstrumentKind::Compass, Icon::Compass, "compass"),
];

fn instrument_actions(ui: &mut Ui, state: &ToolbarState<'_>, commands: &mut Vec<Command>) {
    for (kind, icon, key) in INSTRUMENT_BUTTONS {
        let label = format!("{} {}", t("add-instrument"), t(key));
        if icons::icon_button(ui, icon, &label, false, true).clicked() {
            commands.push(Command::AddInstrument(kind));
        }
    }
    ui.separator();
    if icons::icon_button(
        ui,
        Icon::Xmark,
        t("remove-instrument"),
        false,
        state.has_active_instrument,
    )
    .clicked()
    {
        commands.push(Command::RemoveInstrument);
    }
    if icons::icon_button(
        ui,
        Icon::Trash,
        t("clear-instruments"),
        false,
        state.instrument_count > 0,
    )
    .clicked()
    {
        commands.push(Command::ClearInstruments);
    }
    if state.instrument_count == 0 {
        caption(ui, t("instruments-hint"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_previews_grow_with_the_width_and_stay_bounded() {
        let mut previous = 0.0;
        for w in PEN_WIDTHS.iter().chain(HIGHLIGHTER_WIDTHS.iter()) {
            let d = dot_diameter(*w);
            assert!((3.0..=20.0).contains(&d));
            if *w <= 12.0 {
                assert!(d >= previous);
                previous = d;
            }
        }
    }
}
