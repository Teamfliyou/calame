//! Habillage de l'éditeur : barre du haut, dock d'outils flottant, bulle de réglages, barre de la
//! sélection au lasso et pastille de zoom (maquettes `Editeur` et `Concentration`).

use calame_core::background::PATTERN_BACKGROUNDS;
use calame_core::{Background, BrushKind, Color, InstrumentKind};
use egui::{pos2, vec2, Align, Align2, Color32, Layout, Rect, RichText, Sense, Stroke, Ui, Vec2};

use crate::files::ExportFormat;
use crate::i18n::t;
use crate::icons::{self, Icon};
use crate::theme::{self, Weight};
use crate::tools::{PenPreset, Tool, ToolSettings, HIGHLIGHTER_WIDTHS, PALETTE, PEN_WIDTHS};
use crate::widgets::{self, Button, ButtonStyle};

/// Ce que l'interface de l'éditeur demande à l'application (le reste est modifié directement).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    Undo,
    Redo,
    /// Recentre la page dans la zone visible.
    ResetView,
    ZoomIn,
    ZoomOut,
    SetDark(bool),
    Copy,
    Cut,
    Paste,
    Delete,
    SelectAll,
    /// Applique la couleur courante à la sélection.
    Recolor,
    Duplicate,
    /// Redresse les traits sélectionnés en formes parfaites.
    Straighten,
    /// Les favoris ont changé : à enregistrer.
    FavoritesChanged,
    /// La trousse a changé : à enregistrer.
    PensChanged,
    /// La reconnaissance automatique de formes a été activée ou désactivée : à enregistrer.
    ShapesChanged,
    AddInstrument(InstrumentKind),
    RemoveInstrument,
    ClearInstruments,
    /// Quitter le cahier pour revenir à la bibliothèque.
    BackToLibrary,
    ToggleSidebar,
    ToggleFocus,
    OpenPalette,
    ExportPage(ExportFormat),
    ExportNotebook(ExportFormat),
    ImportPdf,
    Backup,
    RestoreBackup,
    SetBackground(Background),
    PickBackgroundImage,
    NewPage,
    NewFolder,
    QuickNote,
    NewNotebook,
    NewSubject,
}

/// État de l'enregistrement de la page ouverte.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SaveState {
    Saved,
    Saving,
    Nothing,
}

pub struct EditorState {
    pub can_undo: bool,
    pub can_redo: bool,
    pub has_selection: bool,
    pub can_paste: bool,
    pub has_page: bool,
    pub has_active_instrument: bool,
    pub instrument_count: usize,
    pub dark: bool,
    /// Matière du cahier : nom et couleur.
    pub subject: Option<(String, Color32)>,
    pub notebook: String,
    pub page: Option<String>,
    pub save: SaveState,
    pub zoom: f32,
    pub background: Background,
}

/// Ce que l'éditeur garde d'une image à l'autre.
#[derive(Default)]
pub struct EditorUi {
    /// Bulle des réglages fins ouverte au-dessus du dock.
    pub bubble: bool,
}

pub fn background_key(background: Background) -> &'static str {
    match background {
        Background::Blank => "bg-blank",
        Background::GridSmall => "bg-grid-small",
        Background::GridLarge => "bg-grid-large",
        Background::Lines => "bg-lines",
        Background::Dots => "bg-dots",
        Background::Isometric => "bg-isometric",
        Background::Image(_) => "bg-image",
    }
}

const EXPORTS: [(ExportFormat, &str); 3] = [
    (ExportFormat::Pdf, "export-pdf"),
    (ExportFormat::Png, "export-png"),
    (ExportFormat::Svg, "export-svg"),
];

// ---- Barre du haut ----------------------------------------------------------------------

/// Barre du haut (56 px) : retour, sommaire, chemin, état d'enregistrement, actions.
pub fn top_bar(ui: &mut Ui, state: &EditorState) -> Vec<Command> {
    let mut commands = Vec::new();
    let dark = state.dark;
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if Button::new(t("library"), ButtonStyle::Ghost)
            .icon(Icon::ChevronLeft)
            .show(ui)
            .on_hover_text(t("back-to-library"))
            .clicked()
        {
            commands.push(Command::BackToLibrary);
        }
        if widgets::icon_button(ui, Icon::Sidebar, t("toggle-sidebar"), false, true).clicked() {
            commands.push(Command::ToggleSidebar);
        }
        widgets::vertical_separator(ui, 22.0, 4.0);

        // Le chemin prend la place restante, moins le groupe d'actions de droite.
        let export_width = 110.0;
        let right = 6.0 * 42.0 + 14.0 + export_width;
        let width = (ui.available_width() - right).max(60.0);
        let (rect, _) = ui.allocate_exact_size(vec2(width, 40.0), Sense::hover());
        breadcrumb(ui, rect, state);

        if widgets::icon_button(
            ui,
            Icon::Undo,
            &format!("{} (Ctrl Z)", t("undo")),
            false,
            state.can_undo,
        )
        .clicked()
        {
            commands.push(Command::Undo);
        }
        if widgets::icon_button(
            ui,
            Icon::Redo,
            &format!("{} (Ctrl Maj Z)", t("redo")),
            false,
            state.can_redo,
        )
        .clicked()
        {
            commands.push(Command::Redo);
        }
        widgets::vertical_separator(ui, 22.0, 6.0);
        if widgets::icon_button(
            ui,
            Icon::Search,
            &format!("{} (Ctrl K)", t("search")),
            false,
            true,
        )
        .clicked()
        {
            commands.push(Command::OpenPalette);
        }
        if widgets::icon_button(
            ui,
            Icon::Focus,
            &format!("{} (F11)", t("focus-mode")),
            false,
            state.has_page,
        )
        .clicked()
        {
            commands.push(Command::ToggleFocus);
        }
        let more = widgets::icon_button(ui, Icon::More, t("more"), false, true);
        egui::Popup::menu(&more).show(|ui| more_menu(ui, state, &mut commands));
        let export = Button::new(t("export"), ButtonStyle::Ink)
            .icon(Icon::Export)
            .height(38.0)
            .show(ui);
        egui::Popup::menu(&export).show(|ui| {
            ui.set_min_width(220.0);
            ui.add_enabled_ui(state.has_page, |ui| {
                ui.label(
                    RichText::new(t("export-page"))
                        .small()
                        .color(theme::label_secondary(dark)),
                );
                for (format, key) in EXPORTS {
                    if ui.button(t(key)).clicked() {
                        commands.push(Command::ExportPage(format));
                    }
                }
            });
            ui.separator();
            ui.label(
                RichText::new(t("export-notebook"))
                    .small()
                    .color(theme::label_secondary(dark)),
            );
            for (format, key) in EXPORTS {
                if ui.button(t(key)).clicked() {
                    commands.push(Command::ExportNotebook(format));
                }
            }
        });
    });
    commands
}

/// Matière / Cahier / **Page**, puis « ✓ Enregistré ».
fn breadcrumb(ui: &Ui, rect: Rect, state: &EditorState) {
    let dark = state.dark;
    let painter = ui.painter_at(rect);
    let secondary = theme::label_secondary(dark);
    let font = theme::font(14.0, Weight::Regular);
    let mut x = rect.left() + 4.0;
    let y = rect.center().y;
    let room = |x: f32| (rect.right() - x - 110.0).max(20.0);
    if let Some((name, color)) = &state.subject {
        let color = *color;
        painter.rect_filled(
            Rect::from_center_size(pos2(x + 4.0, y), Vec2::splat(8.0)),
            2.0,
            color,
        );
        x += 14.0;
        let r = widgets::truncated(
            &painter,
            pos2(x, y),
            Align2::LEFT_CENTER,
            name,
            font.clone(),
            secondary,
            room(x) * 0.3,
        );
        x = r.right() + 8.0;
        let r = widgets::truncated(
            &painter,
            pos2(x, y),
            Align2::LEFT_CENTER,
            "/",
            font.clone(),
            secondary,
            20.0,
        );
        x = r.right() + 8.0;
    }
    let r = widgets::truncated(
        &painter,
        pos2(x, y),
        Align2::LEFT_CENTER,
        &state.notebook,
        font.clone(),
        secondary,
        room(x) * 0.45,
    );
    x = r.right() + 8.0;
    if let Some(page) = &state.page {
        let r = widgets::truncated(
            &painter,
            pos2(x, y),
            Align2::LEFT_CENTER,
            "/",
            font.clone(),
            secondary,
            20.0,
        );
        x = r.right() + 8.0;
        let r = widgets::truncated(
            &painter,
            pos2(x, y),
            Align2::LEFT_CENTER,
            page,
            theme::font(14.0, Weight::Semibold),
            theme::label(dark),
            room(x),
        );
        x = r.right() + 14.0;
    }
    let (icon, text, color) = match state.save {
        SaveState::Saved => (Some(Icon::Check), t("saved"), theme::success(dark)),
        SaveState::Saving => (None, t("saving"), secondary),
        SaveState::Nothing => return,
    };
    if let Some(icon) = icon {
        icons::paint(
            &painter,
            Rect::from_center_size(pos2(x + 7.0, y), Vec2::splat(14.0)),
            icon,
            color,
        );
        x += 19.0;
    }
    painter.text(
        pos2(x, y),
        Align2::LEFT_CENTER,
        text,
        theme::font(12.0, Weight::Medium),
        color,
    );
}

/// Menu « … » : ce qui était dans les menus Fichier et Affichage.
fn more_menu(ui: &mut Ui, state: &EditorState, commands: &mut Vec<Command>) {
    ui.set_min_width(240.0);
    ui.add_enabled_ui(state.has_page, |ui| {
        ui.menu_button(t("menu-background"), |ui| {
            for background in PATTERN_BACKGROUNDS {
                if ui
                    .selectable_label(
                        state.background == background,
                        t(background_key(background)),
                    )
                    .clicked()
                {
                    commands.push(Command::SetBackground(background));
                    ui.close();
                }
            }
            ui.separator();
            let on_image = matches!(state.background, Background::Image(_));
            if ui.selectable_label(on_image, t("bg-pick-image")).clicked() {
                commands.push(Command::PickBackgroundImage);
                ui.close();
            }
        });
    });
    let mut item = |ui: &mut Ui, enabled: bool, key: &str, command: Command| {
        if ui.add_enabled(enabled, egui::Button::new(t(key))).clicked() {
            commands.push(command);
            ui.close();
        }
    };
    item(ui, true, "import-pdf", Command::ImportPdf);
    ui.separator();
    item(
        ui,
        state.can_paste && state.has_page,
        "paste",
        Command::Paste,
    );
    item(ui, state.has_page, "select-all", Command::SelectAll);
    item(ui, state.has_page, "reset-view", Command::ResetView);
    item(ui, true, "toggle-sidebar", Command::ToggleSidebar);
    let theme_key = if state.dark {
        "light-theme"
    } else {
        "dark-theme"
    };
    item(ui, true, theme_key, Command::SetDark(!state.dark));
    ui.separator();
    item(ui, true, "backup", Command::Backup);
    item(ui, true, "restore-backup", Command::RestoreBackup);
}

// ---- Dock d'outils ----------------------------------------------------------------------

const TOOLS: [(Tool, Icon, &str, &str); 7] = [
    (Tool::Pen, Icon::Pen, "tool-pen", "P"),
    (
        Tool::Highlighter,
        Icon::Highlighter,
        "tool-highlighter",
        "H",
    ),
    (Tool::EraserStroke, Icon::EraserStroke, "tool-eraser", "E"),
    (Tool::Lasso, Icon::Lasso, "tool-lasso", "L"),
    (Tool::Text, Icon::Text, "tool-text", "T"),
    (Tool::Pipette, Icon::Pipette, "tool-pipette", "I"),
    (Tool::Instruments, Icon::Ruler, "tool-instruments", "R"),
];

fn is_eraser(tool: Tool) -> bool {
    matches!(tool, Tool::EraserStroke | Tool::EraserArea)
}

/// Choisit un outil ; un second clic sur l'outil actif ouvre ou ferme la bulle de réglages.
pub fn pick_tool(tools: &mut ToolSettings, editor: &mut EditorUi, tool: Tool) {
    let same = tools.tool == tool || (is_eraser(tool) && is_eraser(tools.tool));
    if same {
        editor.bubble = !editor.bubble;
        return;
    }
    let tool = if is_eraser(tool) { tools.eraser } else { tool };
    if tool == Tool::Pipette {
        tools.previous = tools.tool;
    }
    tools.tool = tool;
}

fn dock_frame(dark: bool) -> egui::Frame {
    let fill = theme::surface(dark);
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(
            fill.r(),
            fill.g(),
            fill.b(),
            if dark { 235 } else { 245 },
        ))
        .stroke(Stroke::new(
            1.0,
            if dark {
                Color32::from_white_alpha(20)
            } else {
                theme::border(false)
            },
        ))
        .corner_radius(theme::RADIUS_DOCK)
        .shadow(theme::dock_shadow(dark))
        .inner_margin(6)
}

/// Diamètre de la pastille d'un stylo : grandit avec l'épaisseur.
fn pen_dot(width: f32) -> f32 {
    (width.sqrt() * 4.2).clamp(6.0, 15.0)
}

fn pen_label(pen: &PenPreset) -> String {
    let kind = match pen.kind {
        BrushKind::Pen => t("tool-pen"),
        BrushKind::Highlighter => t("tool-highlighter"),
    };
    format!("{kind} {} · {:.1}", pen.color.to_hex(), pen.width)
}

/// Pastille d'un stylo de la trousse (anneau de sa couleur quand il est actif).
fn pen_button(ui: &mut Ui, pen: &PenPreset, active: bool) -> egui::Response {
    let dark = ui.visuals().dark_mode;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let color = theme::display_ink(theme::to_color32(pen.color), dark);
        if response.hovered() {
            ui.painter()
                .rect_filled(rect, theme::RADIUS_DOCK_ITEM, theme::hover(dark));
        }
        let c = rect.center();
        if active {
            ui.painter().circle_stroke(c, 14.0, Stroke::new(2.0, color));
        } else {
            ui.painter()
                .circle_stroke(c, 13.0, Stroke::new(1.0, theme::border(dark)));
        }
        ui.painter()
            .circle_filled(c, pen_dot(pen.width) / 2.0, color);
    }
    let label = pen_label(pen);
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, active, &label));
    response
        .on_hover_text(label)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Dock complet : outils, trousse, formes automatiques.
pub fn dock(
    ctx: &egui::Context,
    tools: &mut ToolSettings,
    editor: &mut EditorUi,
    dark: bool,
) -> Vec<Command> {
    let mut commands = Vec::new();
    egui::Area::new(egui::Id::unique("calame-dock"))
        .anchor(Align2::CENTER_BOTTOM, [0.0, -20.0])
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            dock_frame(dark).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for (tool, icon, key, shortcut) in TOOLS {
                        let active =
                            tools.tool == tool || (is_eraser(tool) && is_eraser(tools.tool));
                        // La gomme montre le mode qu'elle reprendra : trait entier ou zone.
                        let icon = if is_eraser(tool) && tools.eraser == Tool::EraserArea {
                            Icon::EraserArea
                        } else {
                            icon
                        };
                        let label = format!("{} ({shortcut})", t(key));
                        if widgets::tool_button(ui, icon, &label, active).clicked() {
                            pick_tool(tools, editor, tool);
                        }
                    }
                    ui.add_space(4.0);
                    widgets::vertical_separator(ui, 28.0, 0.0);
                    ui.add_space(4.0);
                    let count = tools.pens.len();
                    pens(ui, tools, editor, &mut commands, count);
                    if widgets::icon_button_sized(
                        ui,
                        Icon::Plus,
                        t("add-pen"),
                        false,
                        tools.current_pen().is_some(),
                        44.0,
                    )
                    .clicked()
                        && tools.add_current_pen()
                    {
                        commands.push(Command::PensChanged);
                    }
                    ui.add_space(4.0);
                    widgets::vertical_separator(ui, 28.0, 0.0);
                    ui.add_space(4.0);
                    if shapes_toggle(ui, tools.shape_recognition)
                        .on_hover_text(t("shape-recognition-hint"))
                        .clicked()
                    {
                        tools.shape_recognition = !tools.shape_recognition;
                        commands.push(Command::ShapesChanged);
                    }
                });
            });
        });
    commands
}

/// Mini-dock du mode concentration : trois stylos, gomme, lasso, annuler.
pub fn mini_dock(
    ctx: &egui::Context,
    tools: &mut ToolSettings,
    editor: &mut EditorUi,
    can_undo: bool,
) -> Vec<Command> {
    let mut commands = Vec::new();
    egui::Area::new(egui::Id::unique("calame-mini-dock"))
        .anchor(Align2::CENTER_BOTTOM, [0.0, -22.0])
        .show(ctx, |ui| {
            dock_frame(true).corner_radius(18).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let count = tools.pens.len().min(3);
                    pens(ui, tools, editor, &mut commands, count);
                    widgets::vertical_separator(ui, 24.0, 2.0);
                    for (tool, icon, key) in [
                        (Tool::EraserStroke, Icon::EraserStroke, "tool-eraser"),
                        (Tool::Lasso, Icon::Lasso, "tool-lasso"),
                    ] {
                        let active =
                            tools.tool == tool || (is_eraser(tool) && is_eraser(tools.tool));
                        if widgets::tool_button(ui, icon, t(key), active).clicked() {
                            pick_tool(tools, editor, tool);
                        }
                    }
                    if widgets::icon_button_sized(ui, Icon::Undo, t("undo"), false, can_undo, 44.0)
                        .clicked()
                    {
                        commands.push(Command::Undo);
                    }
                });
            });
        });
    commands
}

fn pens(
    ui: &mut Ui,
    tools: &mut ToolSettings,
    editor: &mut EditorUi,
    commands: &mut Vec<Command>,
    count: usize,
) {
    let active = tools.active_pen();
    let mut removed = None;
    for (i, pen) in tools.pens.clone().iter().take(count).enumerate() {
        let response = pen_button(ui, pen, active == Some(i));
        if response.clicked() {
            if active == Some(i) {
                editor.bubble = !editor.bubble;
            } else {
                tools.apply_pen(*pen);
            }
        }
        response.context_menu(|ui| {
            if ui.button(t("remove-pen")).clicked() {
                removed = Some(i);
                ui.close();
            }
        });
    }
    if let Some(i) = removed {
        tools.pens.remove(i);
        commands.push(Command::PensChanged);
    }
}

fn shapes_toggle(ui: &mut Ui, on: bool) -> egui::Response {
    let dark = ui.visuals().dark_mode;
    let text = t("shapes-auto");
    let color = if on {
        theme::on_saffron(dark)
    } else {
        theme::icon(dark)
    };
    let galley =
        ui.painter()
            .layout_no_wrap(text.to_owned(), theme::font(13.0, Weight::Semibold), color);
    let size = vec2(galley.size().x + 18.0 + 8.0 + 28.0, 44.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let fill = if on {
            theme::saffron_soft(dark)
        } else if response.hovered() {
            theme::hover(dark)
        } else {
            Color32::TRANSPARENT
        };
        ui.painter()
            .rect_filled(rect, theme::RADIUS_DOCK_ITEM, fill);
        let glyph = Rect::from_center_size(
            pos2(rect.left() + 14.0 + 9.0, rect.center().y),
            Vec2::splat(18.0),
        );
        icons::paint(ui.painter(), glyph, Icon::Shapes, color);
        let y = rect.center().y - galley.size().y / 2.0;
        ui.painter()
            .galley(pos2(glyph.right() + 8.0, y), galley, color);
    }
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, on, text));
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

// ---- Bulle de réglages ------------------------------------------------------------------

/// Bulle au-dessus du dock : palette complète, sélecteur HSV, hexadécimal, épaisseur, taille de
/// gomme, instruments. `bottom` est la distance du bas de la fenêtre au-dessus de laquelle elle
/// s'affiche.
pub fn bubble(
    ctx: &egui::Context,
    tools: &mut ToolSettings,
    editor: &mut EditorUi,
    state: &EditorState,
    bottom: f32,
) -> Vec<Command> {
    let mut commands = Vec::new();
    if !editor.bubble {
        return commands;
    }
    let dark = state.dark;
    let response = egui::Area::new(egui::Id::unique("calame-bulle"))
        .anchor(Align2::CENTER_BOTTOM, [0.0, -bottom])
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(theme::surface(dark))
                .stroke(Stroke::new(1.0, theme::border(dark)))
                .corner_radius(18)
                .shadow(theme::dock_shadow(dark))
                .inner_margin(16)
                .show(ui, |ui| {
                    ui.set_max_width(560.0);
                    bubble_header(ui, tools, editor, &mut commands);
                    ui.add_space(4.0);
                    bubble_body(ui, tools, state, &mut commands);
                });
        })
        .response;
    // Un appui ailleurs que dans la bulle ou le dock la referme.
    let clicked_outside = ctx.input(|i| {
        i.pointer.any_pressed()
            && i.pointer
                .interact_pos()
                .is_some_and(|p| !response.rect.contains(p))
    });
    if clicked_outside {
        let on_dock = ctx
            .input(|i| i.pointer.interact_pos())
            .and_then(|p| ctx.layer_id_at(p))
            .is_some_and(|layer| {
                layer.id == egui::Id::unique("calame-dock")
                    || layer.id == egui::Id::unique("calame-mini-dock")
            });
        if !on_dock && !ctx.any_popup_open() {
            editor.bubble = false;
        }
    }
    commands
}

fn tool_title(tool: Tool) -> &'static str {
    match tool {
        Tool::Pen => t("tool-pen"),
        Tool::Highlighter => t("tool-highlighter"),
        Tool::EraserStroke | Tool::EraserArea => t("tool-eraser"),
        Tool::Lasso => t("tool-lasso"),
        Tool::Text => t("tool-text"),
        Tool::Pipette => t("tool-pipette"),
        Tool::Instruments => t("tool-instruments"),
    }
}

fn bubble_header(
    ui: &mut Ui,
    tools: &mut ToolSettings,
    editor: &mut EditorUi,
    commands: &mut Vec<Command>,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(tool_title(tools.tool)).font(theme::font(15.0, Weight::Semibold)));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::icon_button_sized(ui, Icon::Close, t("dismiss"), false, true, 32.0)
                .clicked()
            {
                editor.bubble = false;
            }
            if let Some(i) = tools.active_pen() {
                if Button::new(t("remove-pen"), ButtonStyle::Ghost)
                    .height(32.0)
                    .show(ui)
                    .clicked()
                {
                    tools.pens.remove(i);
                    commands.push(Command::PensChanged);
                }
            } else if tools.current_pen().is_some()
                && Button::new(t("add-pen"), ButtonStyle::Ghost)
                    .icon(Icon::Plus)
                    .height(32.0)
                    .show(ui)
                    .clicked()
                && tools.add_current_pen()
            {
                commands.push(Command::PensChanged);
            }
        });
    });
}

fn caption(ui: &mut Ui, text: &str) {
    let dark = ui.visuals().dark_mode;
    ui.label(
        RichText::new(text)
            .size(13.0)
            .color(theme::label_secondary(dark)),
    );
}

fn slider(ui: &mut Ui, value: &mut f32, range: std::ops::RangeInclusive<f32>) {
    ui.scope(|ui| {
        ui.spacing_mut().slider_width = 200.0;
        ui.add(
            egui::Slider::new(value, range)
                .show_value(true)
                .max_decimals(1),
        );
    });
}

/// Diamètre de l'aperçu d'une épaisseur de trait.
fn dot_diameter(width: f32) -> f32 {
    (width.sqrt() * 3.2).clamp(3.0, 20.0)
}

/// Rangée de pastilles montrant l'épaisseur de chaque réglage prédéfini.
fn width_dots(ui: &mut Ui, presets: &[f32], value: &mut f32, color: Color32) {
    let dark = ui.visuals().dark_mode;
    for &w in presets {
        let selected = (*value - w).abs() < 0.01;
        let (rect, response) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::click());
        if ui.is_rect_visible(rect) {
            if selected {
                ui.painter()
                    .rect_filled(rect, theme::RADIUS_BUTTON, theme::accent_soft(dark));
            } else if response.hovered() {
                ui.painter()
                    .rect_filled(rect, theme::RADIUS_BUTTON, theme::hover(dark));
            }
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

fn colors(ui: &mut Ui, tools: &mut ToolSettings, commands: &mut Vec<Command>) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        for color in PALETTE {
            if widgets::color_dot(
                ui,
                theme::to_color32(color),
                20.0,
                tools.color == color,
                &color.to_hex(),
            )
            .clicked()
            {
                tools.color = color;
            }
        }
    });
    ui.horizontal(|ui| {
        // Sélecteur complet : teinte, saturation, valeur (HSV).
        let mut rgb = [tools.color.r, tools.color.g, tools.color.b];
        if ui
            .color_edit_button_srgb(&mut rgb)
            .on_hover_text(t("hsv-picker"))
            .changed()
        {
            tools.color = Color::rgb(rgb[0], rgb[1], rgb[2]);
        }
        let hex_edit = ui.add(
            egui::TextEdit::singleline(&mut tools.hex)
                .desired_width(76.0)
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
        if widgets::icon_button(ui, Icon::Star, t("add-favorite"), false, true).clicked() {
            tools.add_favorite();
            commands.push(Command::FavoritesChanged);
        }
        let mut removed = None;
        ui.spacing_mut().item_spacing.x = 0.0;
        for (i, color) in tools.favorites.clone().into_iter().enumerate() {
            let response = widgets::color_dot(
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
    });
}

fn bubble_body(
    ui: &mut Ui,
    tools: &mut ToolSettings,
    state: &EditorState,
    commands: &mut Vec<Command>,
) {
    let dark = state.dark;
    let ink = theme::display_ink(theme::to_color32(tools.color), dark);
    match tools.tool {
        Tool::Pen => {
            colors(ui, tools, commands);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                width_dots(ui, &PEN_WIDTHS, &mut tools.pen_width, ink);
                ui.add_space(8.0);
                slider(ui, &mut tools.pen_width, 0.5..=30.0);
            });
        }
        Tool::Highlighter => {
            colors(ui, tools, commands);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                width_dots(
                    ui,
                    &HIGHLIGHTER_WIDTHS,
                    &mut tools.highlighter_width,
                    ink.gamma_multiply(0.6),
                );
                ui.add_space(8.0);
                slider(ui, &mut tools.highlighter_width, 5.0..=80.0);
            });
        }
        Tool::Text => {
            colors(ui, tools, commands);
            ui.horizontal(|ui| {
                caption(ui, t("text-size"));
                slider(ui, &mut tools.text_size, 8.0..=96.0);
            });
        }
        Tool::EraserStroke | Tool::EraserArea => {
            let mut mode = usize::from(tools.tool == Tool::EraserArea);
            if widgets::segmented(ui, &[t("eraser-stroke"), t("eraser-area")], &mut mode) {
                tools.tool = if mode == 1 {
                    Tool::EraserArea
                } else {
                    Tool::EraserStroke
                };
                tools.eraser = tools.tool;
            }
            ui.horizontal(|ui| {
                caption(ui, t("eraser-size"));
                slider(ui, &mut tools.eraser_size, 4.0..=80.0);
            });
        }
        Tool::Pipette => caption(ui, t("pipette-hint")),
        Tool::Lasso => {
            ui.horizontal(|ui| {
                let sel = state.has_selection;
                for (icon, enabled, key, command) in [
                    (Icon::Copy, sel, "copy", Command::Copy),
                    (Icon::Cut, sel, "cut", Command::Cut),
                    (Icon::Paste, state.can_paste, "paste", Command::Paste),
                    (Icon::Recolor, sel, "recolor", Command::Recolor),
                    (Icon::Trash, sel, "delete", Command::Delete),
                ] {
                    if widgets::icon_button(ui, icon, t(key), false, enabled).clicked() {
                        commands.push(command);
                    }
                }
            });
            colors(ui, tools, commands);
            if !state.has_selection {
                caption(ui, t("lasso-hint"));
            }
        }
        Tool::Instruments => {
            ui.horizontal(|ui| {
                for (kind, icon, key) in INSTRUMENT_BUTTONS {
                    let label = format!("{} {}", t("add-instrument"), t(key));
                    if widgets::icon_button(ui, icon, &label, false, true).clicked() {
                        commands.push(Command::AddInstrument(kind));
                    }
                }
                widgets::vertical_separator(ui, 24.0, 4.0);
                if widgets::icon_button(
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
                if widgets::icon_button(
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
            });
            if state.instrument_count == 0 {
                caption(ui, t("instruments-hint"));
            }
        }
    }
}

const INSTRUMENT_BUTTONS: [(InstrumentKind, Icon, &str); 5] = [
    (InstrumentKind::Ruler, Icon::Ruler, "ruler"),
    (
        InstrumentKind::SetSquare45,
        Icon::SetSquare45,
        "set-square-45",
    ),
    (
        InstrumentKind::SetSquare60,
        Icon::SetSquare60,
        "set-square-60",
    ),
    (InstrumentKind::Protractor, Icon::Protractor, "protractor"),
    (InstrumentKind::Compass, Icon::Compass, "compass"),
];

// ---- Barre de la sélection, zoom, repères du mode concentration -------------------------

/// Barre sombre flottante au-dessus de la sélection : couleur, dupliquer, copier, couper,
/// redresser les formes, supprimer.
pub fn selection_bar(
    ctx: &egui::Context,
    selection: Rect,
    canvas: Rect,
    color: Color,
) -> Vec<Command> {
    let mut commands = Vec::new();
    let above = selection.top() - 64.0 > canvas.top();
    let (pivot, anchor) = if above {
        (
            Align2::CENTER_BOTTOM,
            pos2(selection.center().x, selection.top() - 14.0),
        )
    } else {
        (
            Align2::CENTER_TOP,
            pos2(
                selection.center().x,
                (selection.bottom() + 14.0).min(canvas.bottom() - 60.0),
            ),
        )
    };
    egui::Area::new(egui::Id::unique("calame-selection-bar"))
        .pivot(pivot)
        .fixed_pos(anchor)
        .constrain_to(canvas.shrink(8.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(theme::INK_BUTTON)
                .corner_radius(12)
                .shadow(egui::epaint::Shadow {
                    offset: [0, 10],
                    blur: 24,
                    spread: 0,
                    color: Color32::from_black_alpha(90),
                })
                .inner_margin(4)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        ui.visuals_mut().dark_mode = true;
                        let (rect, response) =
                            ui.allocate_exact_size(Vec2::splat(36.0), Sense::click());
                        let c = rect.center();
                        if response.hovered() {
                            ui.painter()
                                .rect_filled(rect, 8.0, Color32::from_white_alpha(20));
                        }
                        ui.painter().circle_filled(c, 9.5, Color32::WHITE);
                        ui.painter().circle_filled(c, 8.0, theme::INK_BUTTON);
                        ui.painter().circle_filled(c, 6.5, theme::to_color32(color));
                        if response.on_hover_text(t("recolor")).clicked() {
                            commands.push(Command::Recolor);
                        }
                        for (icon, key, command) in [
                            (Icon::Copy, "duplicate", Command::Duplicate),
                            (Icon::Cut, "cut", Command::Cut),
                        ] {
                            if dark_icon(ui, icon, t(key), Color32::WHITE).clicked() {
                                commands.push(command);
                            }
                        }
                        let text = t("straighten");
                        let galley = ui.painter().layout_no_wrap(
                            text.to_owned(),
                            theme::font(13.0, Weight::Medium),
                            Color32::WHITE,
                        );
                        let (rect, response) = ui.allocate_exact_size(
                            vec2(galley.size().x + 20.0, 36.0),
                            Sense::click(),
                        );
                        if response.hovered() {
                            ui.painter()
                                .rect_filled(rect, 8.0, Color32::from_white_alpha(20));
                        }
                        ui.painter().galley(
                            pos2(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0),
                            galley,
                            Color32::WHITE,
                        );
                        if response
                            .on_hover_text(t("straighten-hint"))
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            commands.push(Command::Straighten);
                        }
                        let (sep, _) = ui.allocate_exact_size(vec2(9.0, 20.0), Sense::hover());
                        ui.painter().line_segment(
                            [sep.center_top(), sep.center_bottom()],
                            Stroke::new(1.0, Color32::from_white_alpha(50)),
                        );
                        if dark_icon(
                            ui,
                            Icon::Trash,
                            t("delete"),
                            Color32::from_rgb(0xFF, 0x8A, 0x75),
                        )
                        .clicked()
                        {
                            commands.push(Command::Delete);
                        }
                    });
                });
        });
    commands
}

fn dark_icon(ui: &mut Ui, icon: Icon, label: &str, color: Color32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, 8.0, Color32::from_white_alpha(20));
    }
    icons::paint(
        ui.painter(),
        Rect::from_center_size(rect.center(), Vec2::splat(17.0)),
        icon,
        color,
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
    response
        .on_hover_text(label)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Pastille « − 100 % + » en bas à droite ; un clic sur le pourcentage recentre la page.
pub fn zoom_pill(ctx: &egui::Context, canvas: Rect, zoom: f32, dark: bool) -> Vec<Command> {
    let mut commands = Vec::new();
    egui::Area::new(egui::Id::unique("calame-zoom"))
        .pivot(Align2::RIGHT_BOTTOM)
        .fixed_pos(canvas.right_bottom() - vec2(16.0, 26.0))
        .show(ctx, |ui| {
            let fill = theme::surface(dark);
            egui::Frame::new()
                .fill(Color32::from_rgba_unmultiplied(
                    fill.r(),
                    fill.g(),
                    fill.b(),
                    245,
                ))
                .stroke(Stroke::new(1.0, theme::border(dark)))
                .corner_radius(12)
                .inner_margin(4)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        if widgets::icon_button_sized(
                            ui,
                            Icon::Minus,
                            t("zoom-out"),
                            false,
                            true,
                            32.0,
                        )
                        .clicked()
                        {
                            commands.push(Command::ZoomOut);
                        }
                        let text = format!("{:.0} %", zoom * 100.0);
                        let (rect, response) =
                            ui.allocate_exact_size(vec2(50.0, 32.0), Sense::click());
                        if response.hovered() {
                            ui.painter().rect_filled(rect, 8.0, theme::hover(dark));
                        }
                        ui.painter().text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            text,
                            theme::font(13.0, Weight::Medium),
                            theme::label(dark),
                        );
                        if response.on_hover_text(t("reset-view")).clicked() {
                            commands.push(Command::ResetView);
                        }
                        if widgets::icon_button_sized(
                            ui,
                            Icon::Plus,
                            t("zoom-in"),
                            false,
                            true,
                            32.0,
                        )
                        .clicked()
                        {
                            commands.push(Command::ZoomIn);
                        }
                    });
                });
        });
    commands
}

/// Repères du mode concentration : « Cahier · page N » en haut au centre, et la sortie à droite.
pub fn focus_overlays(ctx: &egui::Context, canvas: Rect, label: &str) -> Vec<Command> {
    let mut commands = Vec::new();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::unique("calame-focus-label"),
    ));
    let galley = painter.layout_no_wrap(
        label.to_owned(),
        theme::font(13.0, Weight::Regular),
        theme::label_secondary(true),
    );
    let size = galley.size() + vec2(28.0 + 17.0, 16.0);
    let pill = Rect::from_center_size(
        pos2(canvas.center().x, canvas.top() + 18.0 + size.y / 2.0),
        size,
    );
    painter.rect_filled(pill, size.y / 2.0, Color32::from_white_alpha(13));
    painter.rect_filled(
        Rect::from_center_size(pos2(pill.left() + 17.5, pill.center().y), Vec2::splat(7.0)),
        2.0,
        Color32::from_rgb(0x7F, 0x96, 0xFF),
    );
    painter.galley(
        pos2(pill.left() + 31.0, pill.center().y - galley.size().y / 2.0),
        galley,
        theme::label_secondary(true),
    );

    egui::Area::new(egui::Id::unique("calame-focus-exit"))
        .pivot(Align2::RIGHT_TOP)
        .fixed_pos(canvas.right_top() + vec2(-16.0, 14.0))
        .show(ctx, |ui| {
            let text = t("exit-focus");
            let galley = ui.painter().layout_no_wrap(
                text.to_owned(),
                theme::font(13.0, Weight::Medium),
                theme::label(true),
            );
            let (rect, response) =
                ui.allocate_exact_size(vec2(galley.size().x + 28.0 + 50.0, 40.0), Sense::click());
            let fill = if response.hovered() {
                Color32::from_white_alpha(30)
            } else {
                Color32::from_white_alpha(18)
            };
            ui.painter().rect_filled(rect, 12.0, fill);
            ui.painter().galley(
                pos2(rect.left() + 14.0, rect.center().y - galley.size().y / 2.0),
                galley,
                theme::label(true),
            );
            widgets::kbd(
                ui.painter(),
                pos2(rect.right() - 12.0, rect.center().y),
                t("key-escape"),
                true,
                true,
            );
            if response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                commands.push(Command::ToggleFocus);
            }
        });
    commands
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_previews_grow_with_the_width_and_stay_bounded() {
        for presets in [&PEN_WIDTHS[..], &HIGHLIGHTER_WIDTHS[..]] {
            let mut previous = 0.0;
            for w in presets {
                let d = dot_diameter(*w);
                assert!((3.0..=20.0).contains(&d));
                assert!(d >= previous);
                previous = d;
            }
        }
    }

    #[test]
    fn a_second_click_on_the_active_tool_opens_the_bubble() {
        let mut tools = ToolSettings::default();
        let mut editor = EditorUi::default();
        pick_tool(&mut tools, &mut editor, Tool::Lasso);
        assert_eq!(tools.tool, Tool::Lasso);
        assert!(!editor.bubble);
        pick_tool(&mut tools, &mut editor, Tool::Lasso);
        assert!(editor.bubble);
        // La gomme reprend le mode utilisé en dernier.
        tools.eraser = Tool::EraserArea;
        pick_tool(&mut tools, &mut editor, Tool::EraserStroke);
        assert_eq!(tools.tool, Tool::EraserArea);
        pick_tool(&mut tools, &mut editor, Tool::Pipette);
        assert_eq!(tools.previous, Tool::EraserArea);
    }

    #[test]
    fn pen_dots_grow_with_the_width() {
        assert!(pen_dot(2.0) < pen_dot(3.5));
        assert!(pen_dot(18.0) <= 15.0);
        assert!(pen_dot(0.5) >= 6.0);
    }
}
