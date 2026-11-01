//! Panneau latéral : arborescence repliable, recherche, glisser-déposer, corbeille.

use calame_core::{Color, LibraryTree, Node, NodeId, NodeKind};
use egui::collapsing_header::CollapsingState;
use egui::{Color32, RichText};

use crate::i18n::t;

/// Ce que l'utilisateur a demandé dans le panneau ; appliqué ensuite par l'application.
pub enum Action {
    Select(NodeId),
    Create {
        parent: Option<NodeId>,
        kind: NodeKind,
    },
    Rename(NodeId, String),
    Duplicate(NodeId),
    Trash(NodeId),
    MoveBy(NodeId, i32),
    Move(NodeId, Option<NodeId>, Option<usize>),
    SetStyle(NodeId, Color, String),
    Restore(NodeId),
    DeleteForever(NodeId),
    EmptyTrash,
}

struct StyleEdit {
    id: NodeId,
    rgb: [u8; 3],
    icon: String,
}

#[derive(Default)]
pub struct SidebarState {
    search: String,
    renaming: Option<(NodeId, String)>,
    /// Vrai pendant la première image de l'édition, pour donner le focus au champ.
    rename_fresh: bool,
    style: Option<StyleEdit>,
}

const ICONS: [&str; 12] = [
    "📚", "📐", "🔬", "🌍", "🎨", "🎵", "💻", "📖", "🧪", "⚽", "🗣", "💡",
];

impl SidebarState {
    pub fn start_rename(&mut self, id: NodeId, current: &str) {
        self.renaming = Some((id, current.to_owned()));
        self.rename_fresh = true;
    }
}

/// Déplie `id` dans l'arborescence (par exemple après y avoir créé un élément).
pub fn expand(ctx: &egui::Context, id: NodeId) {
    let mut state = CollapsingState::load_with_default_open(ctx, header_id(id), false);
    state.set_open(true);
    state.store(ctx);
}

fn header_id(id: NodeId) -> egui::Id {
    egui::Id::unique(("calame-node", id))
}

pub fn show(
    ui: &mut egui::Ui,
    state: &mut SidebarState,
    tree: &LibraryTree,
    trash: &[Node],
    selected: Option<NodeId>,
) -> Vec<Action> {
    let mut actions = Vec::new();
    ui.heading(t("library"));
    ui.add(
        egui::TextEdit::singleline(&mut state.search)
            .hint_text(t("search-hint"))
            .desired_width(f32::INFINITY),
    );
    if ui.button(t("new-subject")).clicked() {
        actions.push(Action::Create {
            parent: None,
            kind: NodeKind::Subject,
        });
    }
    ui.separator();

    let filter = (!state.search.trim().is_empty()).then(|| tree.matching(&state.search));
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let roots = tree.children(None);
            let visible = roots
                .iter()
                .filter(|n| filter.as_ref().map_or(true, |f| f.contains(&n.id)))
                .count();
            if filter.is_some() && visible == 0 {
                ui.weak(t("no-results"));
            }
            for node in roots {
                draw_node(
                    ui,
                    state,
                    tree,
                    node,
                    selected,
                    filter.as_ref(),
                    &mut actions,
                );
            }
            ui.add_space(12.0);
            trash_section(ui, trash, &mut actions);
        });

    style_window(ui.ctx(), state, &mut actions);
    actions
}

fn draw_node(
    ui: &mut egui::Ui,
    state: &mut SidebarState,
    tree: &LibraryTree,
    node: &Node,
    selected: Option<NodeId>,
    filter: Option<&std::collections::HashSet<NodeId>>,
    actions: &mut Vec<Action>,
) {
    if filter.is_some_and(|f| !f.contains(&node.id)) {
        return;
    }
    if !node.kind.is_container() {
        row(ui, state, tree, node, selected, actions);
        return;
    }
    let children = tree.children(Some(node.id));
    if filter.is_some() {
        // Recherche : tout est déplié, sans état persistant.
        row(ui, state, tree, node, selected, actions);
        ui.indent(("calame-search", node.id), |ui| {
            for child in children {
                draw_node(ui, state, tree, child, selected, filter, actions);
            }
        });
    } else {
        CollapsingState::load_with_default_open(ui.ctx(), header_id(node.id), false)
            .show_header(ui, |ui| row(ui, state, tree, node, selected, actions))
            .body(|ui| {
                for child in children {
                    draw_node(ui, state, tree, child, selected, filter, actions);
                }
            });
    }
}

fn row(
    ui: &mut egui::Ui,
    state: &mut SidebarState,
    tree: &LibraryTree,
    node: &Node,
    selected: Option<NodeId>,
    actions: &mut Vec<Action>,
) {
    if let Some((id, text)) = state.renaming.as_mut().filter(|(id, _)| *id == node.id) {
        let response = ui.text_edit_singleline(text);
        if state.rename_fresh {
            response.request_focus();
            state.rename_fresh = false;
        }
        let cancelled = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if cancelled {
            state.renaming = None;
        } else if response.lost_focus() {
            actions.push(Action::Rename(*id, text.clone()));
            state.renaming = None;
        }
        return;
    }

    let mut text = RichText::new(format!("{} {}", node.icon, node.name));
    if node.kind == NodeKind::Subject {
        let c = node.color;
        text = text.color(Color32::from_rgb(c.r, c.g, c.b)).strong();
    }
    let is_selected = selected == Some(node.id);
    let drag = ui.dnd_drag_source(egui::Id::unique(("calame-drag", node.id)), node.id, |ui| {
        ui.selectable_label(is_selected, text)
    });
    let (label, response) = (drag.inner, drag.response);

    if label.clicked() && node.kind == NodeKind::Page {
        actions.push(Action::Select(node.id));
    }
    if let Some(dragged) = response.dnd_hover_payload::<NodeId>() {
        if tree.drop_target(*dragged, node.id).is_some() {
            ui.painter().rect_stroke(
                response.rect,
                3.0,
                ui.visuals().selection.stroke,
                egui::StrokeKind::Outside,
            );
        }
    }
    if let Some(dragged) = response.dnd_release_payload::<NodeId>() {
        if let Some((parent, index)) = tree.drop_target(*dragged, node.id) {
            actions.push(Action::Move(*dragged, parent, index));
        }
    }
    label.context_menu(|ui| context_menu(ui, state, node, actions));
}

fn context_menu(
    ui: &mut egui::Ui,
    state: &mut SidebarState,
    node: &Node,
    actions: &mut Vec<Action>,
) {
    let create = |ui: &mut egui::Ui, actions: &mut Vec<Action>, kind: NodeKind, label: &str| {
        if ui.button(label).clicked() {
            actions.push(Action::Create {
                parent: Some(node.id),
                kind,
            });
            ui.close();
        }
    };
    if node.kind.can_contain(NodeKind::Notebook) {
        create(ui, actions, NodeKind::Notebook, t("new-notebook"));
    }
    if node.kind.can_contain(NodeKind::Page) {
        create(ui, actions, NodeKind::Page, t("new-page"));
    }
    if node.kind.can_contain(NodeKind::Folder) {
        create(ui, actions, NodeKind::Folder, t("new-folder"));
    }
    if node.kind.is_container() {
        ui.separator();
    }
    if ui.button(t("rename")).clicked() {
        state.start_rename(node.id, &node.name);
        ui.close();
    }
    if node.kind == NodeKind::Subject && ui.button(t("style")).clicked() {
        let c = node.color;
        state.style = Some(StyleEdit {
            id: node.id,
            rgb: [c.r, c.g, c.b],
            icon: node.icon.clone(),
        });
        ui.close();
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
    if ui.button(t("to-trash")).clicked() {
        actions.push(Action::Trash(node.id));
        ui.close();
    }
}

fn style_window(ctx: &egui::Context, state: &mut SidebarState, actions: &mut Vec<Action>) {
    let Some(edit) = state.style.as_mut() else {
        return;
    };
    let (mut apply, mut cancel) = (false, false);
    egui::Window::new(t("style-title"))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(t("color"));
                ui.color_edit_button_srgb(&mut edit.rgb);
            });
            ui.label(t("icon"));
            ui.horizontal_wrapped(|ui| {
                for icon in ICONS {
                    if ui.selectable_label(edit.icon == icon, icon).clicked() {
                        edit.icon = icon.to_owned();
                    }
                }
            });
            ui.text_edit_singleline(&mut edit.icon);
            ui.horizontal(|ui| {
                apply = ui.button(t("apply")).clicked();
                cancel = ui.button(t("cancel")).clicked();
            });
        });
    if apply {
        let [r, g, b] = edit.rgb;
        actions.push(Action::SetStyle(
            edit.id,
            Color::rgb(r, g, b),
            edit.icon.clone(),
        ));
    }
    if apply || cancel {
        state.style = None;
    }
}

fn trash_section(ui: &mut egui::Ui, trash: &[Node], actions: &mut Vec<Action>) {
    egui::CollapsingHeader::new(format!("🗑 {} ({})", t("trash"), trash.len()))
        .id_salt("calame-trash")
        .show(ui, |ui| {
            if trash.is_empty() {
                ui.weak(t("trash-empty"));
                return;
            }
            for node in trash {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("{} {}", node.icon, node.name));
                    if ui.small_button(t("restore")).clicked() {
                        actions.push(Action::Restore(node.id));
                    }
                    if ui
                        .small_button("✖")
                        .on_hover_text(t("delete-forever"))
                        .clicked()
                    {
                        actions.push(Action::DeleteForever(node.id));
                    }
                });
            }
            if ui.button(t("empty-trash")).clicked() {
                actions.push(Action::EmptyTrash);
            }
        });
}
