//! Sommaire d'un cahier : dossiers repliables et pages en miniature, glisser-déposer et menus
//! contextuels ; ou l'aperçu de toutes les pages en grille.
//!
//! La bibliothèque elle-même (matières et cahiers) est dans `home_ui`.

use std::collections::HashMap;
use std::path::PathBuf;

use calame_core::{Background, Color, LibraryTree, Node, NodeId, NodeKind};
use egui::{pos2, vec2, Align2, Color32, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::files::ExportFormat;
use crate::i18n::t;
use crate::icons::{self, Icon};
use crate::theme::{self, Weight};
use crate::thumbs::ThumbSource;
use crate::toolbar::Command;
use crate::widgets::{self, Button, ButtonStyle};

/// Ce que l'utilisateur a demandé dans l'interface ; appliqué ensuite par l'application.
pub enum Action {
    /// Ouvrir une page du cahier courant.
    Select(NodeId),
    /// Ouvrir une page de n'importe quel cahier (bibliothèque, recherche).
    OpenPage(NodeId),
    /// Entrer dans un cahier de la bibliothèque.
    OpenNotebook(NodeId),
    /// Exporter une page, un cahier, une sous-matière ou une matière.
    Export(NodeId, ExportFormat),
    /// Importer un PDF dans une matière, un cahier ou une sous-matière.
    ImportPdf(NodeId),
    /// Importer un PDF déposé sur la fenêtre (`None` : la matière choisie, ou la première).
    ImportPdfPath(Option<NodeId>, PathBuf),
    /// Créer avec le nom par défaut, puis renommer sur place.
    Create {
        parent: Option<NodeId>,
        kind: NodeKind,
    },
    /// Créer une matière avec le nom, la couleur et le pictogramme choisis.
    CreateWith {
        parent: Option<NodeId>,
        kind: NodeKind,
        name: String,
        color: Color,
        icon: String,
    },
    /// Créer un cahier (et sa matière s'il n'y en a pas), avec son papier, puis l'ouvrir.
    CreateNotebook {
        subject: Option<NodeId>,
        name: String,
        color: Color,
        paper: Background,
    },
    /// Modifier un cahier : nom, couverture, matière et papier des nouvelles pages.
    EditNotebook {
        id: NodeId,
        name: String,
        color: Color,
        subject: Option<NodeId>,
        paper: Background,
    },
    Rename(NodeId, String),
    Duplicate(NodeId),
    Trash(NodeId),
    MoveBy(NodeId, i32),
    Move(NodeId, Option<NodeId>, Option<usize>),
    SetStyle(NodeId, Color, String),
    ToggleFavorite(NodeId),
    Restore(NodeId),
    DeleteForever(NodeId),
    EmptyTrash,
    /// Une commande de l'éditeur ou de l'application (palette, menus de la bibliothèque).
    Command(Command),
}

#[derive(Default)]
pub struct SidebarState {
    renaming: Option<(NodeId, String)>,
    /// Vrai pendant la première image de l'édition, pour donner le focus au champ.
    rename_fresh: bool,
    /// Dossier choisi comme destination des prochains ajouts.
    focus: Option<NodeId>,
    /// 0 : arborescence des pages ; 1 : aperçu en grille.
    view: usize,
}

impl SidebarState {
    pub fn start_rename(&mut self, id: NodeId, current: &str) {
        self.renaming = Some((id, current.to_owned()));
        self.rename_fresh = true;
    }

    /// Où iront une nouvelle page ou un nouveau dossier : le dossier choisi, sinon le dossier de
    /// la page ouverte, sinon la racine du cahier.
    pub fn target(&self, tree: &LibraryTree, notebook: NodeId, selected: Option<NodeId>) -> NodeId {
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

fn open_id(id: NodeId) -> egui::Id {
    egui::Id::unique(("calame-ouvert", id))
}

fn is_open(ctx: &egui::Context, id: NodeId) -> bool {
    ctx.data(|d| d.get_temp::<bool>(open_id(id)))
        .unwrap_or(false)
}

fn set_open(ctx: &egui::Context, id: NodeId, open: bool) {
    ctx.data_mut(|d| d.insert_temp(open_id(id), open));
}

/// Déplie `id` dans l'arborescence (par exemple après y avoir créé un élément).
pub fn expand(ctx: &egui::Context, id: NodeId) {
    set_open(ctx, id, true);
}

/// Couleur d'affichage d'un élément : les cahiers et dossiers enregistrés sans couleur propre
/// prennent celle de leur matière.
pub fn node_color(tree: &LibraryTree, node: &Node) -> Color {
    if node.kind == NodeKind::Subject || node.color != theme::LEGACY_NEUTRAL {
        return node.color;
    }
    tree.path(node.id)
        .iter()
        .find(|n| n.kind == NodeKind::Subject)
        .map_or(theme::COVER_COLORS[0], |s| s.color)
}

/// Pages d'un sous-arbre dans l'ordre de lecture (dossiers dépliés).
pub fn page_order(tree: &LibraryTree, root: NodeId) -> Vec<NodeId> {
    fn walk(tree: &LibraryTree, id: NodeId, out: &mut Vec<NodeId>) {
        for child in tree.children(Some(id)) {
            if child.kind == NodeKind::Page {
                out.push(child.id);
            } else {
                walk(tree, child.id, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(tree, root, &mut out);
    out
}

struct Ctx<'a, 'b> {
    tree: &'a LibraryTree,
    selected: Option<NodeId>,
    target: NodeId,
    numbers: HashMap<NodeId, usize>,
    thumbs: &'a mut ThumbSource<'b>,
    actions: Vec<Action>,
}

pub fn show(
    ui: &mut Ui,
    state: &mut SidebarState,
    tree: &LibraryTree,
    notebook: NodeId,
    selected: Option<NodeId>,
    thumbs: &mut ThumbSource<'_>,
) -> Vec<Action> {
    if tree.get(notebook).is_none() {
        return Vec::new();
    }
    let dark = ui.visuals().dark_mode;
    let target = state.target(tree, notebook, selected);
    let order = page_order(tree, notebook);
    let numbers = order
        .iter()
        .enumerate()
        .map(|(i, id)| (*id, i + 1))
        .collect();
    let mut cx = Ctx {
        tree,
        selected,
        target,
        numbers,
        thumbs,
        actions: Vec::new(),
    };

    egui::Frame::new().inner_margin(12).show(ui, |ui| {
        widgets::segmented(ui, &[t("pages"), t("overview")], &mut state.view);
    });

    egui::Panel::bottom("sommaire-actions")
        .resizable(false)
        .frame(
            egui::Frame::new()
                .fill(theme::sidebar(dark))
                .inner_margin(12)
                .stroke(Stroke::new(1.0, theme::border(dark))),
        )
        .show(ui, |ui| {
            ui.columns(2, |cols| {
                if Button::new(t("page"), ButtonStyle::Secondary)
                    .icon(Icon::Plus)
                    .height(40.0)
                    .full_width()
                    .show(&mut cols[0])
                    .on_hover_text(t("new-page-hint"))
                    .clicked()
                {
                    cx.actions.push(Action::Create {
                        parent: Some(target),
                        kind: NodeKind::Page,
                    });
                }
                if Button::new(t("folder"), ButtonStyle::Secondary)
                    .icon(Icon::Folder)
                    .height(40.0)
                    .full_width()
                    .show(&mut cols[1])
                    .on_hover_text(t("sub-subject-hint"))
                    .clicked()
                {
                    cx.actions.push(Action::Create {
                        parent: Some(target),
                        kind: NodeKind::Folder,
                    });
                }
            });
        });

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin {
                    left: 8,
                    right: 8,
                    top: 0,
                    bottom: 8,
                })
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    if state.view == 1 {
                        overview(ui, &mut cx, &order);
                        return;
                    }
                    let children = tree.children(Some(notebook));
                    if children.is_empty() {
                        ui.add_space(12.0);
                        ui.vertical_centered(|ui| {
                            let (rect, _) =
                                ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
                            icons::paint(
                                ui.painter(),
                                rect,
                                Icon::FolderPlus,
                                theme::label_secondary(dark),
                            );
                            ui.label(
                                egui::RichText::new(t("notebook-empty"))
                                    .color(theme::label_secondary(dark)),
                            );
                        });
                    }
                    for node in children {
                        draw_node(ui, state, &mut cx, node, 0);
                    }
                });
        });
    cx.actions
}

/// Aperçu : toutes les pages du cahier en grille de deux colonnes.
fn overview(ui: &mut Ui, cx: &mut Ctx<'_, '_>, order: &[NodeId]) {
    let dark = ui.visuals().dark_mode;
    let width = ui.available_width();
    let gap = 10.0;
    let cell = (width - gap) / 2.0;
    let thumb = vec2(cell, cell * 4.0 / 3.0);
    for chunk in order.chunks(2) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for id in chunk {
                let (rect, response) =
                    ui.allocate_exact_size(thumb + vec2(0.0, 22.0), Sense::click());
                if ui.is_rect_visible(rect) {
                    let page = Rect::from_min_size(rect.min, thumb);
                    cx.thumbs
                        .paint(ui.ctx(), ui.painter(), page, *id, 6.0, dark);
                    let active = cx.selected == Some(*id);
                    let (ring, width) = if active {
                        (theme::accent(dark), 2.0)
                    } else if response.hovered() {
                        (theme::field_border(dark), 1.0)
                    } else {
                        (theme::border(dark), 1.0)
                    };
                    ui.painter().rect_stroke(
                        page,
                        6.0,
                        Stroke::new(width, ring),
                        StrokeKind::Outside,
                    );
                    let n = cx.numbers.get(id).copied().unwrap_or(0);
                    ui.painter().text(
                        pos2(page.center().x, page.bottom() + 11.0),
                        Align2::CENTER_CENTER,
                        n.to_string(),
                        theme::font(
                            12.0,
                            if active {
                                Weight::Semibold
                            } else {
                                Weight::Regular
                            },
                        ),
                        if active {
                            theme::accent(dark)
                        } else {
                            theme::label_secondary(dark)
                        },
                    );
                }
                let name = cx.tree.get(*id).map_or("", |n| n.name.as_str());
                response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
                if response
                    .on_hover_text(name)
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    cx.actions.push(Action::Select(*id));
                }
            }
        });
        ui.add_space(6.0);
    }
}

fn draw_node(
    ui: &mut Ui,
    state: &mut SidebarState,
    cx: &mut Ctx<'_, '_>,
    node: &Node,
    depth: usize,
) {
    if !node.kind.is_container() {
        row(ui, state, cx, node, depth);
        return;
    }
    row(ui, state, cx, node, depth);
    if is_open(ui.ctx(), node.id) {
        let children = cx.tree.children(Some(node.id));
        if !children.is_empty() {
            ui.add_space(2.0);
            for child in children {
                draw_node(ui, state, cx, child, depth + 1);
            }
            ui.add_space(4.0);
        }
    }
}

/// Ligne de dossier : chevron, icône, nom et nombre de pages.
fn folder_row(ui: &mut Ui, cx: &Ctx<'_, '_>, node: &Node, depth: usize) -> Response {
    let dark = ui.visuals().dark_mode;
    let open = is_open(ui.ctx(), node.id);
    let width = ui.available_width().max(80.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, 36.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let highlighted = cx.target == node.id;
        let background = if highlighted || response.hovered() {
            theme::hover(dark)
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, 8.0, background);
        let mut x = rect.left() + 8.0 + depth as f32 * 22.0;
        let y = rect.center().y;
        let chevron = if open {
            Icon::ChevronDown
        } else {
            Icon::ChevronRight
        };
        icons::paint(
            ui.painter(),
            Rect::from_center_size(pos2(x + 7.0, y), Vec2::splat(14.0)),
            chevron,
            theme::label_secondary(dark),
        );
        x += 22.0;
        let folder_color = if open || highlighted {
            theme::accent(dark)
        } else {
            theme::label_secondary(dark)
        };
        icons::paint(
            ui.painter(),
            Rect::from_center_size(pos2(x + 8.0, y), Vec2::splat(16.0)),
            Icon::Folder,
            folder_color,
        );
        x += 24.0;
        let count = cx.tree.page_count(node.id).to_string();
        let count_rect = ui.painter().text(
            pos2(rect.right() - 8.0, y),
            Align2::RIGHT_CENTER,
            count,
            theme::font(12.0, Weight::Regular),
            theme::label_secondary(dark),
        );
        let weight = if open {
            Weight::Semibold
        } else {
            Weight::Regular
        };
        let color = if open {
            theme::label(dark)
        } else {
            theme::icon(dark)
        };
        widgets::truncated(
            ui.painter(),
            pos2(x, y),
            Align2::LEFT_CENTER,
            &node.name,
            theme::font(13.0, weight),
            color,
            count_rect.left() - x - 8.0,
        );
    }
    response
}

/// Ligne de page : miniature 46 × 60, titre et « Page N ».
fn page_row(ui: &mut Ui, cx: &mut Ctx<'_, '_>, node: &Node, depth: usize) -> Response {
    let dark = ui.visuals().dark_mode;
    let width = ui.available_width().max(80.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, 72.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let active = cx.selected == Some(node.id);
        let background = if active {
            theme::accent_soft(dark)
        } else if response.hovered() {
            theme::hover(dark)
        } else {
            Color32::TRANSPARENT
        };
        let indent = depth as f32 * 22.0;
        let body = Rect::from_min_max(pos2(rect.left() + indent, rect.top()), rect.max);
        ui.painter().rect_filled(body, 10.0, background);
        let thumb =
            Rect::from_min_size(pos2(body.left() + 8.0, body.top() + 6.0), vec2(46.0, 60.0));
        cx.thumbs
            .paint(ui.ctx(), ui.painter(), thumb, node.id, 5.0, dark);
        if active {
            ui.painter().rect_stroke(
                thumb,
                5.0,
                Stroke::new(2.0, theme::accent(dark)),
                StrokeKind::Outside,
            );
        } else {
            ui.painter().rect_stroke(
                thumb,
                5.0,
                Stroke::new(1.0, theme::border(dark)),
                StrokeKind::Outside,
            );
        }
        let x = thumb.right() + 12.0;
        let max = body.right() - x - 6.0;
        widgets::truncated(
            ui.painter(),
            pos2(x, body.center().y - 9.0),
            Align2::LEFT_CENTER,
            &node.name,
            theme::font(13.0, Weight::Medium),
            theme::label(dark),
            max,
        );
        let n = cx.numbers.get(&node.id).copied().unwrap_or(0);
        ui.painter().text(
            pos2(x, body.center().y + 10.0),
            Align2::LEFT_CENTER,
            format!("{} {n}", t("page")),
            theme::font(12.0, Weight::Regular),
            theme::label_secondary(dark),
        );
    }
    response
}

fn row(ui: &mut Ui, state: &mut SidebarState, cx: &mut Ctx<'_, '_>, node: &Node, depth: usize) {
    if state
        .renaming
        .as_ref()
        .is_some_and(|(id, _)| *id == node.id)
    {
        let mut finished = None;
        ui.horizontal(|ui| {
            ui.add_space(8.0 + depth as f32 * 22.0);
            let Some((id, text)) = state.renaming.as_mut() else {
                return;
            };
            let response = ui.add(egui::TextEdit::singleline(text).desired_width(f32::INFINITY));
            if state.rename_fresh {
                response.request_focus();
                state.rename_fresh = false;
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                finished = Some(None);
            } else if response.lost_focus() {
                finished = Some(Some((*id, text.clone())));
            }
        });
        if let Some(rename) = finished {
            if let Some((id, name)) = rename {
                cx.actions.push(Action::Rename(id, name));
            }
            state.renaming = None;
        }
        return;
    }

    let drag = ui.dnd_drag_source(egui::Id::unique(("calame-drag", node.id)), node.id, |ui| {
        if node.kind == NodeKind::Page {
            page_row(ui, cx, node, depth)
        } else {
            folder_row(ui, cx, node, depth)
        }
    });
    let (label, response) = (drag.inner, drag.response);
    let name = node.name.clone();
    label.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));

    if label
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        if node.kind == NodeKind::Page {
            state.focus = None;
            cx.actions.push(Action::Select(node.id));
        } else {
            // Un dossier choisi devient la destination des prochains ajouts ; un clic le replie.
            let open = is_open(ui.ctx(), node.id);
            if state.focus == Some(node.id) || !open {
                set_open(ui.ctx(), node.id, !open);
            }
            state.focus = Some(node.id);
        }
    }
    if let Some(dragged) = response.dnd_hover_payload::<NodeId>() {
        if cx.tree.drop_target(*dragged, node.id).is_some() {
            ui.painter().rect_stroke(
                response.rect,
                8.0,
                Stroke::new(1.5, theme::accent(ui.visuals().dark_mode)),
                StrokeKind::Outside,
            );
        }
    }
    if let Some(dragged) = response.dnd_release_payload::<NodeId>() {
        if let Some((parent, index)) = cx.tree.drop_target(*dragged, node.id) {
            cx.actions.push(Action::Move(*dragged, parent, index));
        }
    }
    let mut actions = std::mem::take(&mut cx.actions);
    response.context_menu(|ui| context_menu(ui, state, node, &mut actions));
    cx.actions = actions;
}

/// Menu contextuel d'une page, d'un dossier ou d'un cahier.
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
