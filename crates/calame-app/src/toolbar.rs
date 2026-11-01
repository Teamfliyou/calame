//! Barre d'outils : outils, couleurs, épaisseurs et actions sur la sélection.

use calame_core::Color;
use egui::{Color32, Sense, Stroke, Vec2};

use crate::i18n::t;
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
}

pub struct ToolbarState<'a> {
    pub can_undo: bool,
    pub can_redo: bool,
    pub has_selection: bool,
    pub can_paste: bool,
    pub dark: bool,
    pub breadcrumb: Option<&'a str>,
}

const TOOLS: [(Tool, &str); 7] = [
    (Tool::Pen, "tool-pen"),
    (Tool::Highlighter, "tool-highlighter"),
    (Tool::EraserStroke, "tool-eraser-stroke"),
    (Tool::EraserArea, "tool-eraser-area"),
    (Tool::Lasso, "tool-lasso"),
    (Tool::Text, "tool-text"),
    (Tool::Pipette, "tool-pipette"),
];

pub fn show(ui: &mut egui::Ui, tools: &mut ToolSettings, state: &ToolbarState<'_>) -> Vec<Command> {
    let mut commands = Vec::new();
    tools_row(ui, tools, state, &mut commands);
    color_row(ui, tools, &mut commands);
    options_row(ui, tools, state, &mut commands);
    commands
}

fn tools_row(
    ui: &mut egui::Ui,
    tools: &mut ToolSettings,
    state: &ToolbarState<'_>,
    commands: &mut Vec<Command>,
) {
    ui.horizontal_wrapped(|ui| {
        for (tool, key) in TOOLS {
            if ui.selectable_label(tools.tool == tool, t(key)).clicked() && tools.tool != tool {
                if tool == Tool::Pipette {
                    tools.previous = tools.tool;
                }
                tools.tool = tool;
            }
        }
        ui.separator();
        if ui
            .add_enabled(state.can_undo, egui::Button::new(t("undo")))
            .clicked()
        {
            commands.push(Command::Undo);
        }
        if ui
            .add_enabled(state.can_redo, egui::Button::new(t("redo")))
            .clicked()
        {
            commands.push(Command::Redo);
        }
        ui.separator();
        if ui.button(t("reset-view")).clicked() {
            commands.push(Command::ResetView);
        }
        let mut dark = state.dark;
        if ui.checkbox(&mut dark, t("dark-theme")).changed() {
            commands.push(Command::SetDark(dark));
        }
        if let Some(path) = state.breadcrumb {
            ui.separator();
            ui.weak(path);
        }
    });
}

fn swatch(ui: &mut egui::Ui, color: Color, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
    let fill = Color32::from_rgb(color.r, color.g, color.b);
    ui.painter().rect_filled(rect, 4.0, fill);
    let outline = if selected {
        ui.visuals().selection.stroke
    } else {
        Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color)
    };
    ui.painter()
        .rect_stroke(rect, 4.0, outline, egui::StrokeKind::Outside);
    response
}

fn color_row(ui: &mut egui::Ui, tools: &mut ToolSettings, commands: &mut Vec<Command>) {
    ui.horizontal_wrapped(|ui| {
        for color in PALETTE {
            if swatch(ui, color, tools.color == color).clicked() {
                tools.color = color;
            }
        }
        ui.separator();

        // Sélecteur complet : teinte, saturation, valeur (HSV) et transparence désactivée.
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

        if ui.button("★+").on_hover_text(t("add-favorite")).clicked() {
            tools.add_favorite();
            commands.push(Command::FavoritesChanged);
        }
        let mut removed = None;
        for (i, color) in tools.favorites.clone().into_iter().enumerate() {
            let response =
                swatch(ui, color, tools.color == color).on_hover_text(t("favorite-hint"));
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
    });
}

fn width_presets(ui: &mut egui::Ui, presets: &[f32], value: &mut f32) {
    for &w in presets {
        let selected = (*value - w).abs() < 0.01;
        if ui
            .selectable_label(selected, format!("{w:.0}"))
            .on_hover_text(t("width"))
            .clicked()
        {
            *value = w;
        }
    }
}

fn options_row(
    ui: &mut egui::Ui,
    tools: &mut ToolSettings,
    state: &ToolbarState<'_>,
    commands: &mut Vec<Command>,
) {
    ui.horizontal_wrapped(|ui| match tools.tool {
        Tool::Pen => {
            width_presets(ui, &PEN_WIDTHS, &mut tools.pen_width);
            ui.add(egui::Slider::new(&mut tools.pen_width, 0.5..=30.0).text(t("width")));
        }
        Tool::Highlighter => {
            width_presets(ui, &HIGHLIGHTER_WIDTHS, &mut tools.highlighter_width);
            ui.add(egui::Slider::new(&mut tools.highlighter_width, 5.0..=80.0).text(t("width")));
        }
        Tool::EraserStroke | Tool::EraserArea => {
            ui.add(egui::Slider::new(&mut tools.eraser_size, 4.0..=80.0).text(t("eraser-size")));
        }
        Tool::Text => {
            ui.add(egui::Slider::new(&mut tools.text_size, 8.0..=96.0).text(t("text-size")));
        }
        Tool::Pipette => {
            ui.weak(t("pipette-hint"));
        }
        Tool::Lasso => lasso_actions(ui, state, commands),
    });
}

fn lasso_actions(ui: &mut egui::Ui, state: &ToolbarState<'_>, commands: &mut Vec<Command>) {
    let sel = state.has_selection;
    let actions = [
        (sel, "copy", Command::Copy),
        (sel, "cut", Command::Cut),
        (state.can_paste, "paste", Command::Paste),
        (sel, "recolor", Command::Recolor),
        (sel, "delete", Command::Delete),
    ];
    for (enabled, key, command) in actions {
        if ui.add_enabled(enabled, egui::Button::new(t(key))).clicked() {
            commands.push(command);
        }
    }
    if !sel {
        ui.weak(t("lasso-hint"));
    }
}
