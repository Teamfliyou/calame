//! Palette de recherche et de commandes, ouverte par Ctrl K (maquette `Recherche`).
//!
//! La recherche porte sur les noms des pages, cahiers, matières et dossiers (sans tenir compte des
//! accents ni de la casse) ; les commandes reprennent les actions des menus.

use std::collections::HashMap;

use calame_core::background::PATTERN_BACKGROUNDS;
use calame_core::{LibraryTree, Node, NodeId, NodeKind};
use egui::text::{LayoutJob, TextFormat};
use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::files::ExportFormat;
use crate::i18n::t;
use crate::icons::{self, Icon};
use crate::library_ui::{node_color, page_order, Action};
use crate::theme::{self, Weight};
use crate::thumbs::ThumbSource;
use crate::toolbar::{background_key, Command};
use crate::widgets;

const FILTERS: [&str; 4] = [
    "filter-all",
    "filter-pages",
    "filter-notebooks",
    "filter-commands",
];

#[derive(Default)]
pub struct PaletteState {
    query: String,
    filter: usize,
    selected: usize,
    /// Vrai à l'ouverture, pour donner le focus au champ.
    fresh: bool,
}

impl PaletteState {
    pub fn new() -> Self {
        Self {
            fresh: true,
            ..Self::default()
        }
    }
}

/// Ce que la palette lit de l'application.
pub struct PaletteData<'a> {
    pub tree: &'a LibraryTree,
    pub stamps: &'a HashMap<NodeId, i64>,
    /// Cahier ouvert, et dossier où irait une nouvelle page.
    pub notebook: Option<NodeId>,
    pub target: Option<NodeId>,
    pub has_page: bool,
    pub can_paste: bool,
    pub dark: bool,
}

/// Minuscules sans accents, caractère par caractère (les positions restent alignées).
fn fold_char(c: char) -> char {
    match c {
        'à' | 'â' | 'ä' | 'á' | 'À' | 'Â' | 'Ä' | 'Á' => 'a',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
        'î' | 'ï' | 'í' | 'Î' | 'Ï' | 'Í' => 'i',
        'ô' | 'ö' | 'ó' | 'Ô' | 'Ö' | 'Ó' => 'o',
        'ù' | 'û' | 'ü' | 'ú' | 'Ù' | 'Û' | 'Ü' | 'Ú' => 'u',
        'ç' | 'Ç' => 'c',
        'ÿ' | 'Ÿ' => 'y',
        'œ' | 'Œ' => 'o',
        'æ' | 'Æ' => 'a',
        c => c.to_lowercase().next().unwrap_or(c),
    }
}

/// Plage d'octets de `text` qui correspond à `query`, sans tenir compte des accents ni de la casse.
pub fn find(text: &str, query: &str) -> Option<std::ops::Range<usize>> {
    let needle: Vec<char> = query.trim().chars().map(fold_char).collect();
    if needle.is_empty() {
        return None;
    }
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let folded: Vec<char> = chars.iter().map(|(_, c)| fold_char(*c)).collect();
    let start = folded
        .windows(needle.len())
        .position(|w| w == needle.as_slice())?;
    let begin = chars[start].0;
    let end = chars
        .get(start + needle.len())
        .map_or(text.len(), |(i, _)| *i);
    Some(begin..end)
}

/// Texte avec la partie trouvée surlignée en safran.
fn highlighted(text: &str, query: &str, size: f32, color: Color32) -> LayoutJob {
    let format = TextFormat {
        font_id: theme::font(size, Weight::Medium),
        color,
        ..Default::default()
    };
    let mut job = LayoutJob::default();
    match find(text, query) {
        Some(range) => {
            job.append(&text[..range.start], 0.0, format.clone());
            job.append(
                &text[range.clone()],
                0.0,
                TextFormat {
                    background: Color32::from_rgba_unmultiplied(244, 183, 64, 128),
                    ..format.clone()
                },
            );
            job.append(&text[range.end..], 0.0, format);
        }
        None => job.append(text, 0.0, format),
    }
    job.wrap = egui::text::TextWrapping::truncate_at_width(440.0);
    job
}

enum Entry {
    Page(NodeId),
    Node(NodeId),
    Command {
        command: Command,
        label: String,
        icon: Icon,
        shortcut: Option<&'static str>,
    },
}

fn commands(data: &PaletteData<'_>) -> Vec<(Command, String, Icon, Option<&'static str>)> {
    let in_notebook = data.notebook.is_some();
    let mut list: Vec<(bool, Command, String, Icon, Option<&'static str>)> = Vec::new();
    let target = data
        .target
        .and_then(|id| data.tree.get(id))
        .map_or(String::new(), |n| n.name.clone());
    list.push((
        in_notebook,
        Command::NewPage,
        format!("{} « {target} »", t("cmd-new-page-in")),
        Icon::Plus,
        Some("Ctrl N"),
    ));
    list.push((
        in_notebook,
        Command::NewFolder,
        format!("{} « {target} »", t("cmd-new-folder-in")),
        Icon::FolderPlus,
        None,
    ));
    list.push((
        true,
        Command::QuickNote,
        t("quick-note").into(),
        Icon::Pen,
        (!in_notebook).then_some("Ctrl N"),
    ));
    list.push((
        true,
        Command::NewNotebook,
        t("new-notebook").into(),
        Icon::Notebook,
        None,
    ));
    list.push((
        true,
        Command::NewSubject,
        t("new-subject").into(),
        Icon::Book,
        None,
    ));
    list.push((
        true,
        Command::ImportPdf,
        t("import-pdf").into(),
        Icon::ImportPdf,
        None,
    ));
    for (format, key, shortcut) in [
        (ExportFormat::Pdf, "cmd-export-page-pdf", Some("Ctrl E")),
        (ExportFormat::Png, "cmd-export-page-png", None),
        (ExportFormat::Svg, "cmd-export-page-svg", None),
    ] {
        list.push((
            data.has_page,
            Command::ExportPage(format),
            t(key).into(),
            Icon::Export,
            shortcut,
        ));
    }
    for (format, key) in [
        (ExportFormat::Pdf, "cmd-export-notebook-pdf"),
        (ExportFormat::Png, "cmd-export-notebook-png"),
        (ExportFormat::Svg, "cmd-export-notebook-svg"),
    ] {
        list.push((
            in_notebook,
            Command::ExportNotebook(format),
            t(key).into(),
            Icon::Export,
            None,
        ));
    }
    list.push((
        data.has_page,
        Command::ToggleFocus,
        t("focus-mode").into(),
        Icon::Focus,
        Some("F11"),
    ));
    list.push((
        in_notebook,
        Command::ToggleSidebar,
        t("toggle-sidebar").into(),
        Icon::Sidebar,
        None,
    ));
    list.push((
        data.has_page,
        Command::ResetView,
        t("reset-view").into(),
        Icon::Recenter,
        None,
    ));
    list.push((
        data.has_page && data.can_paste,
        Command::Paste,
        t("paste").into(),
        Icon::Paste,
        Some("Ctrl V"),
    ));
    list.push((
        data.has_page,
        Command::SelectAll,
        t("select-all").into(),
        Icon::Lasso,
        Some("Ctrl A"),
    ));
    for background in PATTERN_BACKGROUNDS {
        list.push((
            data.has_page,
            Command::SetBackground(background),
            format!(
                "{} : {}",
                t("menu-background"),
                t(background_key(background))
            ),
            Icon::Page,
            None,
        ));
    }
    list.push((
        data.has_page,
        Command::PickBackgroundImage,
        format!("{} : {}", t("menu-background"), t("bg-pick-image")),
        Icon::Image,
        None,
    ));
    let (theme_key, theme_icon) = if data.dark {
        ("light-theme", Icon::Sun)
    } else {
        ("dark-theme", Icon::Moon)
    };
    list.push((
        true,
        Command::SetDark(!data.dark),
        t(theme_key).into(),
        theme_icon,
        None,
    ));
    list.push((
        true,
        Command::Backup,
        t("backup").into(),
        Icon::Export,
        None,
    ));
    list.push((
        true,
        Command::RestoreBackup,
        t("restore-backup").into(),
        Icon::ImportPdf,
        None,
    ));
    list.push((
        in_notebook,
        Command::BackToLibrary,
        t("back-to-library").into(),
        Icon::Home,
        None,
    ));
    list.into_iter()
        .filter(|(enabled, ..)| *enabled)
        .map(|(_, c, label, icon, shortcut)| (c, label, icon, shortcut))
        .collect()
}

/// Résultats groupés (titre de groupe, entrées), selon la recherche et le filtre.
fn results(state: &PaletteState, data: &PaletteData<'_>) -> Vec<(&'static str, Vec<Entry>)> {
    let query = state.query.trim();
    let tree = data.tree;
    let matches = |node: &Node| query.is_empty() || find(&node.name, query).is_some();
    let mut groups = Vec::new();
    let all = state.filter == 0;

    if all || state.filter == 1 {
        let mut pages: Vec<&Node> = if query.is_empty() {
            let mut recent: Vec<(&NodeId, &i64)> = data.stamps.iter().collect();
            recent.sort_by(|a, b| b.1.cmp(a.1));
            recent
                .into_iter()
                .filter_map(|(id, _)| tree.get(*id))
                .collect()
        } else {
            all_nodes(tree)
                .into_iter()
                .filter(|n| n.kind == NodeKind::Page && matches(n))
                .collect()
        };
        if !query.is_empty() {
            pages.sort_by_key(|n| {
                (
                    find(&n.name, query).map_or(usize::MAX, |r| r.start),
                    n.name.to_lowercase(),
                )
            });
        } else {
            pages.sort_by_key(|n| std::cmp::Reverse(data.stamps.get(&n.id).copied().unwrap_or(0)));
        }
        let limit = if state.filter == 1 { 40 } else { 6 };
        let entries: Vec<Entry> = pages
            .into_iter()
            .take(limit)
            .map(|n| Entry::Page(n.id))
            .collect();
        if !entries.is_empty() {
            groups.push(("group-pages", entries));
        }
    }
    if all || state.filter == 2 {
        let nodes = all_nodes(tree);
        let limit = if state.filter == 2 { 40 } else { 5 };
        let notebooks: Vec<Entry> = nodes
            .iter()
            .filter(|n| matches!(n.kind, NodeKind::Notebook | NodeKind::Subject) && matches(n))
            .take(limit)
            .map(|n| Entry::Node(n.id))
            .collect();
        if !notebooks.is_empty() {
            groups.push(("group-notebooks", notebooks));
        }
        if !query.is_empty() {
            let folders: Vec<Entry> = nodes
                .iter()
                .filter(|n| n.kind == NodeKind::Folder && matches(n))
                .take(limit)
                .map(|n| Entry::Node(n.id))
                .collect();
            if !folders.is_empty() {
                groups.push(("group-folders", folders));
            }
        }
    }
    if all || state.filter == 3 {
        let limit = if state.filter == 3 || query.is_empty() {
            60
        } else {
            6
        };
        let entries: Vec<Entry> = commands(data)
            .into_iter()
            .filter(|(_, label, ..)| query.is_empty() || find(label, query).is_some())
            .take(if all && query.is_empty() { 8 } else { limit })
            .map(|(command, label, icon, shortcut)| Entry::Command {
                command,
                label,
                icon,
                shortcut,
            })
            .collect();
        if !entries.is_empty() {
            groups.push(("group-commands", entries));
        }
    }
    groups
}

/// Tous les éléments de la bibliothèque, dans l'ordre de l'arborescence.
fn all_nodes(tree: &LibraryTree) -> Vec<&Node> {
    fn walk<'a>(tree: &'a LibraryTree, parent: Option<NodeId>, out: &mut Vec<&'a Node>) {
        for child in tree.children(parent) {
            out.push(child);
            walk(tree, Some(child.id), out);
        }
    }
    let mut out = Vec::new();
    walk(tree, None, &mut out);
    out
}

fn activate(entry: &Entry, tree: &LibraryTree) -> Option<Action> {
    match entry {
        Entry::Page(id) => Some(Action::OpenPage(*id)),
        Entry::Node(id) => match tree.get(*id)?.kind {
            NodeKind::Notebook => Some(Action::OpenNotebook(*id)),
            NodeKind::Folder => tree
                .first_page_in(*id)
                .map(Action::OpenPage)
                .or_else(|| tree.notebook_of(*id).map(Action::OpenNotebook)),
            NodeKind::Subject => tree
                .children(Some(*id))
                .first()
                .map(|n| Action::OpenNotebook(n.id)),
            NodeKind::Page => Some(Action::OpenPage(*id)),
        },
        Entry::Command { command, .. } => Some(Action::Command(*command)),
    }
}

/// Affiche la palette ; `state` devient `None` quand elle se ferme.
pub fn show(
    ctx: &egui::Context,
    slot: &mut Option<PaletteState>,
    data: &PaletteData<'_>,
    thumbs: &mut ThumbSource<'_>,
) -> Vec<Action> {
    let mut actions = Vec::new();
    let Some(state) = slot.as_mut() else {
        return actions;
    };
    let dark = data.dark;
    let groups = results(state, data);
    let count: usize = groups.iter().map(|(_, e)| e.len()).sum();
    // Les flèches et Entrée sont lues avant le champ de saisie.
    let (down, up, enter) = ctx.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
        )
    });
    let mut moved = false;
    if count > 0 {
        if down {
            state.selected = (state.selected + 1) % count;
            moved = true;
        }
        if up {
            state.selected = (state.selected + count - 1) % count;
            moved = true;
        }
        state.selected = state.selected.min(count - 1);
    }
    let mut chosen: Option<usize> = enter.then_some(state.selected).filter(|_| count > 0);

    let screen = ctx.content_rect();
    let width = (screen.width() - 32.0).min(660.0);
    let area = egui::Modal::default_area(egui::Id::unique("calame-palette"))
        .anchor(Align2::CENTER_TOP, [0.0, 96.0]);
    let modal = egui::Modal::new(egui::Id::unique("calame-palette"))
        .area(area)
        .backdrop_color(theme::veil())
        .frame(
            egui::Frame::new()
                .fill(theme::surface(dark))
                .corner_radius(20)
                .shadow(theme::window_shadow()),
        )
        .show(ctx, |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            // Champ de recherche.
            let (row, _) = ui.allocate_exact_size(vec2(width, 62.0), Sense::hover());
            icons::paint(
                ui.painter(),
                Rect::from_center_size(pos2(row.left() + 28.0, row.center().y), Vec2::splat(20.0)),
                Icon::Search,
                theme::label_secondary(dark),
            );
            let kbd = widgets::kbd(
                ui.painter(),
                pos2(row.right() - 18.0, row.center().y),
                t("key-escape"),
                dark,
                false,
            );
            let field = Rect::from_min_max(
                pos2(row.left() + 50.0, row.top()),
                pos2(kbd.left() - 12.0, row.bottom()),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(field)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            let edit = child.add(
                egui::TextEdit::singleline(&mut state.query)
                    .frame(egui::Frame::NONE)
                    .font(theme::font(18.0, Weight::Regular))
                    .hint_text(t("palette-hint"))
                    .desired_width(field.width()),
            );
            if state.fresh {
                edit.request_focus();
                state.fresh = false;
            }
            if edit.changed() {
                state.selected = 0;
            }
            ui.painter().line_segment(
                [row.left_bottom(), row.right_bottom()],
                Stroke::new(1.0, theme::border(dark)),
            );

            // Filtres.
            egui::Frame::new()
                .inner_margin(egui::Margin {
                    left: 18,
                    right: 18,
                    top: 12,
                    bottom: 4,
                })
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                        for (i, key) in FILTERS.iter().enumerate() {
                            if widgets::chip(ui, t(key), None, state.filter == i, 30.0).clicked() {
                                state.filter = i;
                                state.selected = 0;
                            }
                        }
                    });
                });

            // Résultats.
            let max_height = (screen.height() - 96.0 - 62.0 - 60.0 - 50.0 - 32.0).max(160.0);
            egui::ScrollArea::vertical()
                .max_height(max_height)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    egui::Frame::new()
                        .inner_margin(egui::Margin {
                            left: 10,
                            right: 10,
                            top: 8,
                            bottom: 10,
                        })
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            if groups.is_empty() {
                                ui.add_space(14.0);
                                ui.horizontal(|ui| {
                                    ui.add_space(10.0);
                                    ui.label(
                                        egui::RichText::new(t("no-results"))
                                            .color(theme::label_secondary(dark)),
                                    );
                                });
                                ui.add_space(14.0);
                            }
                            let mut index = 0;
                            for (title, entries) in &groups {
                                ui.add_space(if index == 0 { 4.0 } else { 10.0 });
                                ui.horizontal(|ui| {
                                    ui.add_space(10.0);
                                    widgets::section_caps(ui, t(title));
                                });
                                ui.add_space(6.0);
                                for entry in entries {
                                    let active = index == state.selected;
                                    let response =
                                        result_row(ui, entry, active, &state.query, data, thumbs);
                                    if active && moved {
                                        response.scroll_to_me(None);
                                    }
                                    if response.hovered()
                                        && ui.input(|i| i.pointer.delta() != Vec2::ZERO)
                                    {
                                        state.selected = index;
                                    }
                                    if response.clicked() {
                                        chosen = Some(index);
                                    }
                                    index += 1;
                                }
                            }
                        });
                });

            // Aide au clavier.
            let (footer, _) = ui.allocate_exact_size(vec2(width, 44.0), Sense::hover());
            ui.painter().rect_filled(
                footer,
                egui::CornerRadius {
                    nw: 0,
                    ne: 0,
                    sw: 20,
                    se: 20,
                },
                theme::sidebar(dark),
            );
            ui.painter().line_segment(
                [footer.left_top(), footer.right_top()],
                Stroke::new(1.0, theme::border(dark)),
            );
            let mut x = footer.left() + 20.0;
            for (keys, text) in [
                (&["↑", "↓"][..], t("navigate")),
                (&["Entrée"][..], t("open")),
                (&["Échap"][..], t("dismiss")),
            ] {
                for key in keys {
                    let r = widgets::kbd_left(ui.painter(), pos2(x, footer.center().y), key, dark);
                    x = r.right() + 4.0;
                }
                let end = ui.painter().text(
                    pos2(x + 2.0, footer.center().y),
                    Align2::LEFT_CENTER,
                    text,
                    theme::font(12.0, Weight::Regular),
                    theme::label_secondary(dark),
                );
                x = end.right() + 18.0;
            }
        });

    if let Some(i) = chosen {
        if let Some(entry) = groups.iter().flat_map(|(_, e)| e.iter()).nth(i) {
            if let Some(action) = activate(entry, data.tree) {
                actions.push(action);
            }
        }
        *slot = None;
    } else if modal.should_close() {
        *slot = None;
    }
    actions
}

fn result_row(
    ui: &mut Ui,
    entry: &Entry,
    active: bool,
    query: &str,
    data: &PaletteData<'_>,
    thumbs: &mut ThumbSource<'_>,
) -> egui::Response {
    let dark = data.dark;
    let tree = data.tree;
    let height = if matches!(entry, Entry::Page(_)) {
        64.0
    } else {
        54.0
    };
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let painter = ui.painter();
    let fill = if active {
        theme::accent_soft(dark)
    } else if response.hovered() {
        theme::hover(dark)
    } else {
        Color32::TRANSPARENT
    };
    painter.rect_filled(rect, 12.0, fill);
    let left = rect.left() + 10.0;
    let text_x = left + 38.0 + 14.0;
    let label_color = theme::label(dark);
    let secondary = theme::label_secondary(dark);
    let mut right = rect.right() - 10.0;
    if active {
        let r = widgets::kbd(painter, pos2(right, rect.center().y), "Entrée", dark, false);
        right = r.left() - 10.0;
    }
    match entry {
        Entry::Page(id) => {
            let thumb = Rect::from_min_size(pos2(left, rect.center().y - 24.0), vec2(38.0, 48.0));
            thumbs.paint(ui.ctx(), painter, thumb, *id, 5.0, dark);
            painter.rect_stroke(
                thumb,
                5.0,
                Stroke::new(
                    1.0,
                    if active {
                        theme::field_border(dark)
                    } else {
                        theme::border(dark)
                    },
                ),
                StrokeKind::Outside,
            );
            let name = tree.get(*id).map_or("", |n| n.name.as_str());
            let galley = painter.layout_job(highlighted(name, query, 15.0, label_color));
            painter.galley(
                pos2(text_x, rect.center().y - 2.0 - galley.size().y),
                galley,
                label_color,
            );
            let mut parts: Vec<String> = tree
                .path(*id)
                .iter()
                .filter(|n| n.kind != NodeKind::Page)
                .map(|n| n.name.clone())
                .collect();
            if let Some(notebook) = tree.notebook_of(*id) {
                if let Some(n) = page_order(tree, notebook).iter().position(|p| p == id) {
                    parts.push(format!("{} {}", t("page").to_lowercase(), n + 1));
                }
            }
            widgets::truncated(
                painter,
                pos2(text_x, rect.center().y + 3.0),
                Align2::LEFT_TOP,
                &parts.join(" · "),
                theme::font(12.0, Weight::Regular),
                secondary,
                right - text_x,
            );
        }
        Entry::Node(id) => {
            let Some(node) = tree.get(*id) else {
                return response;
            };
            let badge = Rect::from_min_size(pos2(left, rect.center().y - 19.0), Vec2::splat(38.0));
            let tint = theme::to_color32(node_color(tree, node));
            match node.kind {
                NodeKind::Subject => icons::tile(painter, badge, tint, &node.icon),
                _ => {
                    painter.rect_filled(badge, 10.0, theme::accent_soft(dark));
                    let icon = if node.kind == NodeKind::Folder {
                        Icon::Folder
                    } else {
                        Icon::Notebook
                    };
                    icons::paint(painter, badge.shrink(10.0), icon, theme::accent(dark));
                }
            }
            let galley = painter.layout_job(highlighted(&node.name, query, 15.0, label_color));
            painter.galley(
                pos2(text_x, rect.center().y - 1.0 - galley.size().y),
                galley,
                label_color,
            );
            let parent = node
                .parent_id
                .and_then(|p| tree.get(p))
                .map(|p| p.name.as_str());
            let pages = tree.page_count(node.id);
            let detail = match (node.kind, parent) {
                (NodeKind::Subject, _) => format!(
                    "{} {}",
                    tree.children(Some(node.id)).len(),
                    t("notebook-many")
                ),
                (_, Some(parent)) => format!(
                    "{} {parent} · {pages} {}",
                    t("inside"),
                    if pages == 1 {
                        t("page-one")
                    } else {
                        t("page-many")
                    }
                ),
                _ => format!("{pages} {}", t("page-many")),
            };
            widgets::truncated(
                painter,
                pos2(text_x, rect.center().y + 3.0),
                Align2::LEFT_TOP,
                &detail,
                theme::font(12.0, Weight::Regular),
                secondary,
                right - text_x,
            );
        }
        Entry::Command {
            label,
            icon,
            shortcut,
            ..
        } => {
            let badge = Rect::from_min_size(pos2(left, rect.center().y - 19.0), Vec2::splat(38.0));
            painter.rect_filled(
                badge,
                10.0,
                if active {
                    theme::surface(dark)
                } else {
                    theme::chip(dark)
                },
            );
            icons::paint(painter, badge.shrink(10.0), *icon, theme::icon(dark));
            if let Some(shortcut) = shortcut {
                let r = widgets::kbd(painter, pos2(right, rect.center().y), shortcut, dark, false);
                right = r.left() - 10.0;
            }
            let mut job = highlighted(label, query, 15.0, label_color);
            for section in &mut job.sections {
                section.format.font_id = theme::font(15.0, Weight::Regular);
            }
            job.wrap = egui::text::TextWrapping::truncate_at_width(right - text_x);
            let galley = painter.layout_job(job);
            painter.galley(
                pos2(text_x, rect.center().y - galley.size().y / 2.0),
                galley,
                label_color,
            );
        }
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_ignores_accents_and_case() {
        assert_eq!(find("Dérivées et tangentes", "dériv"), Some(0..6));
        assert_eq!(find("Dérivées et tangentes", "DERIV"), Some(0..6));
        assert_eq!(find("Les dérivées", "derivees"), Some(4..14));
        assert_eq!(find("Tangente", "x"), None);
        assert_eq!(find("Tangente", "  "), None);
        let text = "Œuvre complète";
        let range = find(text, "complete").unwrap_or(0..0);
        assert_eq!(&text[range], "complète");
    }

    #[test]
    fn commands_depend_on_the_screen() {
        let tree = LibraryTree::default();
        let stamps = HashMap::new();
        let library = PaletteData {
            tree: &tree,
            stamps: &stamps,
            notebook: None,
            target: None,
            has_page: false,
            can_paste: false,
            dark: false,
        };
        let list = commands(&library);
        assert!(list.iter().any(|(c, ..)| *c == Command::QuickNote));
        assert!(!list.iter().any(|(c, ..)| *c == Command::NewPage));
        assert!(!list
            .iter()
            .any(|(c, ..)| matches!(c, Command::ExportPage(_))));
    }
}
