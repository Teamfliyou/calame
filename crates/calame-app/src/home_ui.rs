//! Écran d'accueil : la bibliothèque. Chaque matière est une étagère de cahiers ; on choisit un
//! cahier pour l'ouvrir, ou l'on en crée un.

use calame_core::{Color, LibraryTree, Node, NodeId, NodeKind};
use egui::{
    pos2, vec2, Align, Align2, Color32, CornerRadius, FontId, Layout, Painter, Rect, RichText,
    Sense, Stroke, Ui, Vec2,
};

use crate::files::ExportFormat;
use crate::i18n::t;
use crate::icons::{self, ButtonStyle, Icon};
use crate::library_ui::{node_color, Action};
use crate::theme;

/// Dimensions d'une couverture de cahier.
const COVER: Vec2 = vec2(132.0, 176.0);
const CAPTION_HEIGHT: f32 = 24.0;

enum SheetTarget {
    Create(Option<NodeId>),
    Edit(NodeId),
}

/// Fenêtre de création ou de modification d'une matière ou d'un cahier.
struct Sheet {
    target: SheetTarget,
    kind: NodeKind,
    name: String,
    color: Color,
    icon: String,
    /// Vrai avant la première image, pour donner le focus au champ du nom.
    fresh: bool,
}

#[derive(Default)]
pub struct HomeState {
    search: String,
    sheet: Option<Sheet>,
    show_trash: bool,
}

impl HomeState {
    fn open_create(&mut self, parent: Option<NodeId>, kind: NodeKind, color: Color) {
        let icon = if kind == NodeKind::Subject {
            Icon::Book.key().to_owned()
        } else {
            kind.default_icon().to_owned()
        };
        self.sheet = Some(Sheet {
            target: SheetTarget::Create(parent),
            kind,
            name: String::new(),
            color,
            icon,
            fresh: true,
        });
    }

    fn open_edit(&mut self, node: &Node, color: Color) {
        self.sheet = Some(Sheet {
            target: SheetTarget::Edit(node.id),
            kind: node.kind,
            name: node.name.clone(),
            color,
            icon: node.icon.clone(),
            fresh: true,
        });
    }
}

/// Encre lisible sur une couleur de couverture : sombre sur les fonds clairs.
fn on_color(color: Color32) -> Color32 {
    let luminance =
        (0.299 * f32::from(color.r()) + 0.587 * f32::from(color.g()) + 0.114 * f32::from(color.b()))
            / 255.0;
    if luminance > 0.68 {
        Color32::from_rgb(28, 28, 30)
    } else {
        Color32::WHITE
    }
}

/// Couverture d'un cahier : couleur, tranche, titre, et pictogramme de la matière.
fn draw_cover(painter: &Painter, cover: Rect, color: Color32, title: &str, glyph: &str, dark: bool) {
    let k = cover.width() / COVER.x;
    let radius = 8.0 * k;
    painter.rect_filled(
        cover.translate(vec2(0.0, 3.0 * k)).expand(0.5),
        radius,
        Color32::from_black_alpha(if dark { 120 } else { 46 }),
    );
    painter.rect_filled(cover, radius, color);

    let rounded = radius.round() as u8;
    let spine = Rect::from_min_max(cover.min, pos2(cover.min.x + 12.0 * k, cover.max.y));
    painter.rect_filled(
        spine,
        CornerRadius {
            nw: rounded,
            sw: rounded,
            ne: 0,
            se: 0,
        },
        Color32::from_black_alpha(34),
    );
    painter.line_segment(
        [
            pos2(spine.max.x, cover.min.y + 5.0 * k),
            pos2(spine.max.x, cover.max.y - 5.0 * k),
        ],
        Stroke::new(1.0, Color32::from_white_alpha(46)),
    );

    let ink = on_color(color);
    let galley = painter.layout(
        title.to_owned(),
        FontId::proportional((16.0 * k).max(8.0)),
        ink,
        cover.width() - 38.0 * k,
    );
    painter
        .with_clip_rect(cover.shrink(2.0))
        .galley(pos2(cover.min.x + 24.0 * k, cover.min.y + 18.0 * k), galley, ink);
    if let Some(icon) = Icon::from_key(glyph) {
        let spot = Rect::from_center_size(
            pos2(cover.max.x - 24.0 * k, cover.max.y - 24.0 * k),
            Vec2::splat(24.0 * k),
        );
        icons::paint(painter, spot, icon, ink.gamma_multiply(0.75));
    }
}

fn count_label(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

pub fn show(
    ui: &mut Ui,
    state: &mut HomeState,
    tree: &LibraryTree,
    trash: &[Node],
) -> Vec<Action> {
    let mut actions = Vec::new();
    let dark = ui.visuals().dark_mode;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(40, 24))
                .show(ui, |ui| {
                    header(ui, state, tree, trash.len());
                    ui.add_space(14.0);
                    if state.search.trim().is_empty() {
                        shelves(ui, state, tree, &mut actions, dark);
                    } else {
                        results(ui, state, tree, &mut actions, dark);
                    }
                });
        });
    sheet_window(ui.ctx(), state, &mut actions);
    trash_window(ui.ctx(), state, trash, &mut actions);
    actions
}

fn header(ui: &mut Ui, state: &mut HomeState, tree: &LibraryTree, trash_count: usize) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(t("library")).size(32.0).strong());
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let new_subject = icons::text_button(
                ui,
                Some(Icon::Plus),
                t("new-subject"),
                ButtonStyle::Prominent,
                true,
            );
            if new_subject.clicked() {
                let index = tree.children(None).len();
                let color = theme::COVER_COLORS[index % theme::COVER_COLORS.len()];
                state.open_create(None, NodeKind::Subject, color);
            }
            let trash_label = if trash_count == 0 {
                t("trash").to_owned()
            } else {
                format!("{} ({trash_count})", t("trash"))
            };
            if icons::icon_button(ui, Icon::Trash, &trash_label, state.show_trash, true).clicked()
            {
                state.show_trash = !state.show_trash;
            }
            ui.add(
                egui::TextEdit::singleline(&mut state.search)
                    .hint_text(t("search-hint"))
                    .desired_width(220.0),
            );
        });
    });
}

fn shelves(
    ui: &mut Ui,
    state: &mut HomeState,
    tree: &LibraryTree,
    actions: &mut Vec<Action>,
    dark: bool,
) {
    let subjects = tree.children(None);
    if subjects.is_empty() {
        empty_state(ui, state, dark);
        return;
    }
    for subject in subjects {
        subject_section(ui, state, tree, subject, actions, dark);
        ui.add_space(22.0);
    }
}

fn empty_state(ui: &mut Ui, state: &mut HomeState, dark: bool) {
    ui.add_space(70.0);
    ui.vertical_centered(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(96.0), Sense::hover());
        icons::tile(ui.painter(), rect, theme::accent(dark), Icon::Book.key());
        ui.add_space(14.0);
        ui.label(RichText::new(t("welcome-title")).size(24.0).strong());
        ui.label(RichText::new(t("welcome-text")).color(theme::label_secondary(dark)));
        ui.add_space(10.0);
        if icons::text_button(
            ui,
            Some(Icon::Plus),
            t("create-first-subject"),
            ButtonStyle::Prominent,
            true,
        )
        .clicked()
        {
            state.open_create(None, NodeKind::Subject, theme::COVER_COLORS[6]);
        }
    });
}

fn subject_section(
    ui: &mut Ui,
    state: &mut HomeState,
    tree: &LibraryTree,
    subject: &Node,
    actions: &mut Vec<Action>,
    dark: bool,
) {
    let color = theme::to_color32(subject.color);
    let notebooks = tree.children(Some(subject.id));
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
        icons::tile(ui.painter(), rect, color, &subject.icon);
        ui.label(RichText::new(&subject.name).size(22.0).strong());
        ui.label(
            RichText::new(count_label(
                notebooks.len(),
                t("notebook-one"),
                t("notebook-many"),
            ))
            .color(theme::label_secondary(dark)),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let more = icons::icon_button(ui, Icon::More, t("more"), false, true);
            egui::Popup::menu(&more).show(|ui| node_menu(ui, tree, state, subject, actions));
            if icons::text_button(ui, Some(Icon::Plus), t("notebook"), ButtonStyle::Link, true)
                .clicked()
            {
                state.open_create(Some(subject.id), NodeKind::Notebook, subject.color);
            }
        });
    });
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(24.0, 18.0);
        for notebook in notebooks {
            notebook_card(ui, state, tree, notebook, subject, actions, dark);
        }
        new_notebook_card(ui, state, subject, dark);
    });
}

fn notebook_card(
    ui: &mut Ui,
    state: &mut HomeState,
    tree: &LibraryTree,
    notebook: &Node,
    subject: &Node,
    actions: &mut Vec<Action>,
    dark: bool,
) {
    let (rect, response) = ui.allocate_exact_size(
        vec2(COVER.x, COVER.y + CAPTION_HEIGHT),
        Sense::click(),
    );
    if ui.is_rect_visible(rect) {
        let lift = if response.hovered() { -3.0 } else { 0.0 };
        let cover = Rect::from_min_size(rect.min, COVER).translate(vec2(0.0, lift));
        let color = theme::to_color32(node_color(tree, notebook));
        draw_cover(ui.painter(), cover, color, &notebook.name, &subject.icon, dark);
        let pages = count_label(
            tree.page_count(notebook.id),
            t("page-one"),
            t("page-many"),
        );
        ui.painter().text(
            pos2(rect.center().x, rect.min.y + COVER.y + 13.0),
            Align2::CENTER_CENTER,
            pages,
            FontId::proportional(12.0),
            theme::label_secondary(dark),
        );
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &notebook.name));
    if response.clicked() {
        actions.push(Action::OpenNotebook(notebook.id));
    }
    response.context_menu(|ui| node_menu(ui, tree, state, notebook, actions));
    response.on_hover_cursor(egui::CursorIcon::PointingHand);
}

fn new_notebook_card(ui: &mut Ui, state: &mut HomeState, subject: &Node, dark: bool) {
    let (rect, response) = ui.allocate_exact_size(
        vec2(COVER.x, COVER.y + CAPTION_HEIGHT),
        Sense::click(),
    );
    if ui.is_rect_visible(rect) {
        let cover = Rect::from_min_size(rect.min, COVER);
        let fill = if response.hovered() {
            theme::fill_hover(dark)
        } else {
            theme::fill_secondary(dark)
        };
        ui.painter().rect_filled(cover, 8.0, fill);
        icons::paint(
            ui.painter(),
            Rect::from_center_size(cover.center(), Vec2::splat(34.0)),
            Icon::Plus,
            theme::accent(dark),
        );
        ui.painter().text(
            pos2(rect.center().x, rect.min.y + COVER.y + 13.0),
            Align2::CENTER_CENTER,
            t("new-notebook"),
            FontId::proportional(12.0),
            theme::label_secondary(dark),
        );
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, true, t("new-notebook"))
    });
    if response.clicked() {
        state.open_create(Some(subject.id), NodeKind::Notebook, subject.color);
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand);
}

/// Menu d'une matière ou d'un cahier (clic droit, ou bouton « plus » d'une matière).
fn node_menu(
    ui: &mut Ui,
    tree: &LibraryTree,
    state: &mut HomeState,
    node: &Node,
    actions: &mut Vec<Action>,
) {
    if ui.button(t("edit")).clicked() {
        state.open_edit(node, node_color(tree, node));
        ui.close();
    }
    if node.kind == NodeKind::Subject && ui.button(t("new-notebook-dots")).clicked() {
        state.open_create(Some(node.id), NodeKind::Notebook, node.color);
        ui.close();
    }
    if node.kind == NodeKind::Notebook {
        let others: Vec<&Node> = tree
            .children(None)
            .into_iter()
            .filter(|s| Some(s.id) != node.parent_id)
            .collect();
        if !others.is_empty() {
            ui.menu_button(t("move-to"), |ui| {
                for subject in others {
                    if ui.button(subject.name.as_str()).clicked() {
                        actions.push(Action::Move(node.id, Some(subject.id), None));
                        ui.close();
                    }
                }
            });
        }
    }
    if ui.button(t("duplicate")).clicked() {
        actions.push(Action::Duplicate(node.id));
        ui.close();
    }
    if ui.button(t("move-up")).clicked() {
        actions.push(Action::MoveBy(node.id, -1));
        ui.close();
    }
    if ui.button(t("move-down")).clicked() {
        actions.push(Action::MoveBy(node.id, 1));
        ui.close();
    }
    ui.separator();
    if ui.button(t("import-pdf-here")).clicked() {
        actions.push(Action::ImportPdf(node.id));
        ui.close();
    }
    ui.menu_button(t("export"), |ui| {
        for (format, key) in [
            (ExportFormat::Pdf, "export-pdf"),
            (ExportFormat::Png, "export-png"),
            (ExportFormat::Svg, "export-svg"),
        ] {
            if ui.button(t(key)).clicked() {
                actions.push(Action::Export(node.id, format));
                ui.close();
            }
        }
    });
    ui.separator();
    if ui.button(t("to-trash")).clicked() {
        actions.push(Action::Trash(node.id));
        ui.close();
    }
}

fn kind_rank(kind: NodeKind) -> u8 {
    match kind {
        NodeKind::Subject => 0,
        NodeKind::Notebook => 1,
        NodeKind::Folder => 2,
        NodeKind::Page => 3,
    }
}

/// Résultats de recherche : matières, cahiers, sous-matières et pages dont le nom correspond.
fn results(
    ui: &mut Ui,
    state: &mut HomeState,
    tree: &LibraryTree,
    actions: &mut Vec<Action>,
    dark: bool,
) {
    let query = state.search.trim().to_lowercase();
    let keep = tree.matching(&query);
    let mut hits: Vec<&Node> = keep
        .iter()
        .filter_map(|id| tree.get(*id))
        .filter(|n| n.name.to_lowercase().contains(&query))
        .collect();
    hits.sort_by_key(|n| (kind_rank(n.kind), n.name.to_lowercase()));
    if hits.is_empty() {
        ui.label(RichText::new(t("no-results")).color(theme::label_secondary(dark)));
        return;
    }
    for node in hits {
        let path = tree
            .path(node.id)
            .iter()
            .take_while(|n| n.id != node.id)
            .map(|n| n.name.as_str())
            .collect::<Vec<_>>()
            .join(" › ");
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), 48.0), Sense::click());
        if ui.is_rect_visible(rect) {
            if response.hovered() {
                ui.painter().rect_filled(rect, 10.0, theme::fill_hover(dark));
            }
            let badge = Rect::from_center_size(
                pos2(rect.left() + 28.0, rect.center().y),
                Vec2::splat(30.0),
            );
            let tint = theme::to_color32(node_color(tree, node));
            match node.kind {
                NodeKind::Subject => icons::tile(ui.painter(), badge, tint, &node.icon),
                NodeKind::Notebook => icons::paint(ui.painter(), badge, Icon::Notebook, tint),
                NodeKind::Folder => icons::paint(ui.painter(), badge, Icon::Folder, tint),
                NodeKind::Page => {
                    icons::paint(ui.painter(), badge, Icon::Page, theme::label_secondary(dark));
                }
            }
            let text_x = rect.left() + 56.0;
            ui.painter().text(
                pos2(text_x, rect.center().y - 8.0),
                Align2::LEFT_CENTER,
                &node.name,
                FontId::proportional(15.0),
                theme::label(dark),
            );
            ui.painter().text(
                pos2(text_x, rect.center().y + 10.0),
                Align2::LEFT_CENTER,
                if path.is_empty() { t("library") } else { path.as_str() },
                FontId::proportional(12.0),
                theme::label_secondary(dark),
            );
        }
        if response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            match node.kind {
                NodeKind::Subject => {}
                NodeKind::Notebook => actions.push(Action::OpenNotebook(node.id)),
                NodeKind::Folder | NodeKind::Page => {
                    if let Some(notebook) = tree.notebook_of(node.id) {
                        actions.push(Action::OpenNotebook(notebook));
                        if node.kind == NodeKind::Page {
                            actions.push(Action::Select(node.id));
                        }
                    }
                }
            }
            state.search.clear();
        }
    }
}

fn sheet_title(sheet: &Sheet) -> &'static str {
    match (&sheet.target, sheet.kind) {
        (SheetTarget::Create(_), NodeKind::Subject) => t("sheet-new-subject"),
        (SheetTarget::Create(_), _) => t("sheet-new-notebook"),
        (SheetTarget::Edit(_), NodeKind::Subject) => t("sheet-edit-subject"),
        (SheetTarget::Edit(_), _) => t("sheet-edit-notebook"),
    }
}

fn glyph_label(icon: Icon) -> &'static str {
    match icon {
        Icon::Math => t("glyph-math"),
        Icon::Flask => t("glyph-flask"),
        Icon::Globe => t("glyph-globe"),
        Icon::Brush => t("glyph-brush"),
        Icon::Music => t("glyph-music"),
        Icon::Code => t("glyph-code"),
        Icon::Sport => t("glyph-sport"),
        Icon::Chat => t("glyph-chat"),
        Icon::Bulb => t("glyph-bulb"),
        Icon::Heart => t("glyph-heart"),
        Icon::Star => t("glyph-star"),
        _ => t("glyph-book"),
    }
}

/// Feuille de création ou de modification : aperçu, nom, couleur et pictogramme.
fn sheet_window(ctx: &egui::Context, state: &mut HomeState, actions: &mut Vec<Action>) {
    let Some(mut sheet) = state.sheet.take() else {
        return;
    };
    let dark = ctx.global_style().visuals.dark_mode;
    let (mut done, mut cancel) = (false, false);
    egui::Window::new(sheet_title(&sheet))
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.set_width(340.0);
            ui.add_space(4.0);
            ui.vertical_centered(|ui| {
                if sheet.kind == NodeKind::Subject {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(76.0), Sense::hover());
                    icons::tile(ui.painter(), rect, theme::to_color32(sheet.color), &sheet.icon);
                } else {
                    let (rect, _) = ui.allocate_exact_size(vec2(88.0, 118.0), Sense::hover());
                    draw_cover(
                        ui.painter(),
                        rect,
                        theme::to_color32(sheet.color),
                        &sheet.name,
                        "",
                        dark,
                    );
                }
                ui.add_space(6.0);
                ui.label(RichText::new(sheet_title(&sheet)).size(17.0).strong());
            });
            ui.add_space(6.0);

            let hint = if sheet.kind == NodeKind::Subject {
                t("subject-name")
            } else {
                t("notebook-name")
            };
            let name = ui.add(
                egui::TextEdit::singleline(&mut sheet.name)
                    .hint_text(hint)
                    .desired_width(f32::INFINITY),
            );
            if sheet.fresh {
                name.request_focus();
                sheet.fresh = false;
            }
            let enter = name.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

            ui.label(RichText::new(t("color")).small().color(theme::label_secondary(dark)));
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                for color in theme::COVER_COLORS {
                    let selected = sheet.color == color;
                    if icons::color_dot(
                        ui,
                        theme::to_color32(color),
                        24.0,
                        selected,
                        &color.to_hex(),
                    )
                    .clicked()
                    {
                        sheet.color = color;
                    }
                }
            });

            if sheet.kind == NodeKind::Subject {
                ui.label(RichText::new(t("icon")).small().color(theme::label_secondary(dark)));
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                    for glyph in icons::SUBJECT_GLYPHS {
                        let selected = Icon::from_key(&sheet.icon) == Some(glyph);
                        if icons::icon_button(ui, glyph, glyph_label(glyph), selected, true)
                            .clicked()
                        {
                            sheet.icon = glyph.key().to_owned();
                        }
                    }
                });
            }

            ui.add_space(8.0);
            let valid = !sheet.name.trim().is_empty();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if icons::text_button(ui, None, t("done"), ButtonStyle::Prominent, valid)
                    .clicked()
                    || (enter && valid)
                {
                    done = true;
                }
                if icons::text_button(ui, None, t("cancel"), ButtonStyle::Plain, true).clicked() {
                    cancel = true;
                }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                cancel = true;
            }
        });

    if done {
        let name = sheet.name.trim().to_owned();
        match sheet.target {
            SheetTarget::Create(parent) => actions.push(Action::CreateWith {
                parent,
                kind: sheet.kind,
                name,
                color: sheet.color,
                icon: sheet.icon,
            }),
            SheetTarget::Edit(id) => {
                actions.push(Action::Rename(id, name));
                actions.push(Action::SetStyle(id, sheet.color, sheet.icon));
            }
        }
    } else if !cancel {
        state.sheet = Some(sheet);
    }
}

fn trash_window(
    ctx: &egui::Context,
    state: &mut HomeState,
    trash: &[Node],
    actions: &mut Vec<Action>,
) {
    if !state.show_trash {
        return;
    }
    let dark = ctx.global_style().visuals.dark_mode;
    let mut open = true;
    egui::Window::new(t("trash"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.set_width(380.0);
            if trash.is_empty() {
                ui.label(RichText::new(t("trash-empty")).color(theme::label_secondary(dark)));
                return;
            }
            for node in trash {
                ui.horizontal(|ui| {
                    let icon = match node.kind {
                        NodeKind::Subject => Icon::Book,
                        NodeKind::Notebook => Icon::Notebook,
                        NodeKind::Folder => Icon::Folder,
                        NodeKind::Page => Icon::Page,
                    };
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
                    icons::paint(ui.painter(), rect, icon, theme::label_secondary(dark));
                    ui.add(egui::Label::new(node.name.as_str()).truncate());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if icons::icon_button(ui, Icon::Xmark, t("delete-forever"), false, true)
                            .clicked()
                        {
                            actions.push(Action::DeleteForever(node.id));
                        }
                        if icons::text_button(ui, None, t("restore"), ButtonStyle::Link, true)
                            .clicked()
                        {
                            actions.push(Action::Restore(node.id));
                        }
                    });
                });
            }
            ui.add_space(6.0);
            if icons::text_button(ui, Some(Icon::Trash), t("empty-trash"), ButtonStyle::Plain, true)
                .clicked()
            {
                actions.push(Action::EmptyTrash);
            }
        });
    state.show_trash = open;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ink_is_dark_on_light_covers_and_white_otherwise() {
        assert_eq!(on_color(Color32::from_rgb(255, 204, 0)), Color32::from_rgb(28, 28, 30));
        assert_eq!(on_color(Color32::from_rgb(0, 122, 255)), Color32::WHITE);
        assert_eq!(on_color(Color32::from_rgb(88, 86, 214)), Color32::WHITE);
    }

    #[test]
    fn counts_are_pluralised() {
        assert_eq!(count_label(1, "cahier", "cahiers"), "1 cahier");
        assert_eq!(count_label(0, "cahier", "cahiers"), "0 cahiers");
        assert_eq!(count_label(3, "cahier", "cahiers"), "3 cahiers");
    }
}
