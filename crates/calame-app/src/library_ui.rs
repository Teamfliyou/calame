//! Sommaire d'un cahier : sous-matières et pages, avec recherche et glisser-déposer.
//!
//! La bibliothèque elle-même (matières et cahiers) est dans `home_ui`. Ce panneau n'affiche que le
//! contenu du cahier ouvert : ses sous-matières, imbriquables sans limite, et ses pages.

use std::collections::HashSet;

use calame_core::{Color, LibraryTree, Node, NodeId, NodeKind};
use egui::collapsing_header::CollapsingState;
use egui::{vec2, Align2, Color32, FontId, Rect, Response, RichText, Sense, Ui};

use crate::files::ExportFormat;
use crate::i18n::t;
use crate::icons::{self, ButtonStyle, Icon};
use crate::theme;

/// Ce que l'utilisateur a demandé dans l'interface ; appliqué ensuite par l'application.
pub enum Action {
    /// Ouvrir une page du cahier courant.
    Select(NodeId),
    /// Entrer dans un cahier de la bibliothèque.
    OpenNotebook(NodeId),
    /// Exporter une page, un cahier, une sous-matière ou une matière.
    Export(NodeId, ExportFormat),
    /// Importer un PDF dans une matière, un cahier ou une sous-matière.
    ImportPdf(NodeId),
    /// Créer avec le nom par défaut, puis renommer sur place.
    Create {
        parent: Option<NodeId>,
        kind: NodeKind,
    },
    /// Créer avec le nom, la couleur et le pictogramme choisis dans la fenêtre de création.
    CreateWith {
        parent: Option<NodeId>,
        kind: NodeKind,
        name: String,
        color: Color,
        icon: String,
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

#[derive(Default)]
pub struct SidebarState {
    search: String,
    renaming: Option<(NodeId, String)>,
    /// Vrai pendant la première image de l'édition, pour donner le focus au champ.
    rename_fresh: bool,
    /// Sous-matière choisie comme destination des prochains ajouts.
    focus: Option<NodeId>,
}

impl SidebarState {
    pub fn start_rename(&mut self, id: NodeId, current: &str) {
        self.renaming = Some((id, current.to_owned()));
        self.rename_fresh = true;
    }

    /// Où iront une nouvelle page ou une nouvelle sous-matière : la sous-matière choisie, sinon
    /// le dossier de la page ouverte, sinon la racine du cahier.
    fn target(&self, tree: &LibraryTree, notebook: NodeId, selected: Option<NodeId>) -> NodeId {
        let usable = |id: NodeId| {
            tree.get(id).is_some_and(|n| n.kind.is_container()) && tree.is_within(id, notebook)
        };
        if let Some(focus) = self.focus.filter(|f| usable(*f)) {
            return focus;
        }
        if let Some(parent) = selected
            .and_then(|s| tree.get(s))
            .and_then(|n| n.parent_id)
            .filter(|p| usable(*p))
        {
            return parent;
        }
        notebook
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

/// Couleur d'affichage d'un élément : les cahiers et sous-matières enregistrés sans couleur
/// propre prennent celle de leur matière.
pub fn node_color(tree: &LibraryTree, node: &Node) -> Color {
    if node.kind == NodeKind::Subject || node.color != theme::LEGACY_NEUTRAL {
        return node.color;
    }
    tree.path(node.id)
        .iter()
        .find(|n| n.kind == NodeKind::Subject)
        .map_or(theme::COVER_COLORS[6], |s| s.color)
}

pub fn show(
    ui: &mut Ui,
    state: &mut SidebarState,
    tree: &LibraryTree,
    notebook: NodeId,
    selected: Option<NodeId>,
) -> Vec<Action> {
    let mut actions = Vec::new();
    let Some(nb) = tree.get(notebook) else {
        return actions;
    };
    let dark = ui.visuals().dark_mode;
    let tint = theme::to_color32(node_color(tree, nb));

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::hover());
        icons::tile(ui.painter(), rect, tint, "book");
        ui.add(egui::Label::new(RichText::new(&nb.name).size(16.0).strong()).truncate());
    });
    ui.add(
        egui::TextEdit::singleline(&mut state.search)
            .hint_text(t("search-in-notebook"))
            .desired_width(f32::INFINITY),
    );

    let target = state.target(tree, notebook, selected);
    let target_name = tree.get(target).map_or("", |n| n.name.as_str());
    egui::Panel::bottom("sommaire-actions")
        .resizable(false)
        .show(ui, |ui| {
            ui.add_space(2.0);
            ui.add(
                egui::Label::new(
                    RichText::new(format!("{} {target_name}", t("add-in")))
                        .small()
                        .color(theme::label_secondary(dark)),
                )
                .truncate(),
            );
            ui.horizontal(|ui| {
                if icons::text_button(ui, Some(Icon::PagePlus), t("page"), ButtonStyle::Plain, true)
                    .clicked()
                {
                    actions.push(Action::Create {
                        parent: Some(target),
                        kind: NodeKind::Page,
                    });
                }
                if icons::text_button(
                    ui,
                    Some(Icon::FolderPlus),
                    t("sub-subject"),
                    ButtonStyle::Plain,
                    true,
                )
                .on_hover_text(t("sub-subject-hint"))
                .clicked()
                {
                    actions.push(Action::Create {
                        parent: Some(target),
                        kind: NodeKind::Folder,
                    });
                }
            });
            ui.add_space(2.0);
        });

    let filter = (!state.search.trim().is_empty()).then(|| tree.matching(&state.search));
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let children = tree.children(Some(notebook));
            let visible = children
                .iter()
                .filter(|n| filter.as_ref().map_or(true, |f| f.contains(&n.id)))
                .count();
            if filter.is_some() && visible == 0 {
                ui.weak(t("no-results"));
            } else if children.is_empty() {
                ui.add_space(12.0);
                ui.vertical_centered(|ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
                    icons::paint(
                        ui.painter(),
                        rect,
                        Icon::FolderPlus,
                        theme::label_secondary(dark),
                    );
                    ui.label(
                        RichText::new(t("notebook-empty")).color(theme::label_secondary(dark)),
                    );
                });
            }
            for node in children {
                draw_node(
                    ui,
                    state,
                    tree,
                    node,
                    selected,
                    filter.as_ref(),
                    target,
                    &mut actions,
                );
            }
        });
    actions
}

#[allow(clippy::too_many_arguments)]
fn draw_node(
    ui: &mut Ui,
    state: &mut SidebarState,
    tree: &LibraryTree,
    node: &Node,
    selected: Option<NodeId>,
    filter: Option<&HashSet<NodeId>>,
    target: NodeId,
    actions: &mut Vec<Action>,
) {
    if filter.is_some_and(|f| !f.contains(&node.id)) {
        return;
    }
    if !node.kind.is_container() {
        row(ui, state, tree, node, selected, target, actions);
        return;
    }
    let children = tree.children(Some(node.id));
    if filter.is_some() {
        // Recherche : tout est déplié, sans état persistant.
        row(ui, state, tree, node, selected, target, actions);
        ui.indent(("calame-search", node.id), |ui| {
            for child in children {
                draw_node(ui, state, tree, child, selected, filter, target, actions);
            }
        });
    } else {
        CollapsingState::load_with_default_open(ui.ctx(), header_id(node.id), false)
            .show_header(ui, |ui| {
                row(ui, state, tree, node, selected, target, actions);
            })
            .body(|ui| {
                for child in children {
                    draw_node(ui, state, tree, child, selected, filter, target, actions);
                }
            });
    }
}

/// Ligne du sommaire : pictogramme, nom, fond au survol et à la sélection.
fn node_label(
    ui: &mut Ui,
    node: &Node,
    tint: Color32,
    selected: bool,
    highlighted: bool,
) -> Response {
    let dark = ui.visuals().dark_mode;
    let width = ui.available_width().max(80.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, 30.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let accent = theme::accent(dark);
        let background = if selected {
            Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 46)
        } else if highlighted {
            theme::fill_secondary(dark)
        } else if response.hovered() {
            theme::fill_hover(dark)
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, 7.0, background);

        let (icon, icon_color) = match node.kind {
            NodeKind::Page => (
                Icon::Page,
                if selected {
                    accent
                } else {
                    theme::label_secondary(dark)
                },
            ),
            _ => (Icon::Folder, tint),
        };
        let glyph = Rect::from_center_size(
            egui::pos2(rect.left() + 15.0, rect.center().y),
            vec2(18.0, 18.0),
        );
        icons::paint(ui.painter(), glyph, icon, icon_color);

        let text_color = if selected {
            accent
        } else {
            theme::label(dark)
        };
        ui.painter_at(rect.shrink2(vec2(2.0, 0.0))).text(
            egui::pos2(rect.left() + 32.0, rect.center().y),
            Align2::LEFT_CENTER,
            &node.name,
            FontId::proportional(14.0),
            text_color,
        );
    }
    response
}

fn row(
    ui: &mut Ui,
    state: &mut SidebarState,
    tree: &LibraryTree,
    node: &Node,
    selected: Option<NodeId>,
    target: NodeId,
    actions: &mut Vec<Action>,
) {
    if let Some((id, text)) = state.renaming.as_mut().filter(|(id, _)| *id == node.id) {
        let response = ui.add(egui::TextEdit::singleline(text).desired_width(f32::INFINITY));
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

    let tint = theme::to_color32(node_color(tree, node));
    let is_selected = selected == Some(node.id);
    let highlighted = node.kind == NodeKind::Folder && target == node.id;
    let drag = ui.dnd_drag_source(egui::Id::unique(("calame-drag", node.id)), node.id, |ui| {
        node_label(ui, node, tint, is_selected, highlighted)
    });
    let (label, response) = (drag.inner, drag.response);

    if label.clicked() {
        if node.kind == NodeKind::Page {
            state.focus = None;
            actions.push(Action::Select(node.id));
        } else {
            // Une sous-matière choisie devient la destination des prochains ajouts.
            state.focus = Some(node.id);
            expand(ui.ctx(), node.id);
        }
    }
    if let Some(dragged) = response.dnd_hover_payload::<NodeId>() {
        if tree.drop_target(*dragged, node.id).is_some() {
            ui.painter().rect_stroke(
                response.rect,
                7.0,
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

/// Menu contextuel d'une page, d'une sous-matière ou d'un cahier.
pub fn context_menu(ui: &mut Ui, state: &mut SidebarState, node: &Node, actions: &mut Vec<Action>) {
    let create = |ui: &mut Ui, actions: &mut Vec<Action>, kind: NodeKind, label: &str| {
        if ui.button(label).clicked() {
            actions.push(Action::Create {
                parent: Some(node.id),
                kind,
            });
            ui.close();
        }
    };
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
    if node.kind.is_container() && ui.button(t("import-pdf-here")).clicked() {
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
