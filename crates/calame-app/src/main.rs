//! Calame : prise de notes manuscrites.

mod canvas;
mod i18n;
mod library_ui;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context as _};
use calame_core::{Brush, BrushKind, Color, History, LibraryTree, Node, NodeId, NodeKind, Page};
use calame_storage::Store;
use canvas::{Canvas, View};
use library_ui::{Action, SidebarState};

/// Délai sans modification avant l'enregistrement automatique de la page ouverte.
const AUTOSAVE_DELAY: Duration = Duration::from_secs(1);

/// Page affichée dans le canevas, avec son historique et son état d'enregistrement.
struct OpenPage {
    id: NodeId,
    page: Page,
    history: History,
    /// Révision de l'historique au dernier enregistrement.
    saved_revision: u64,
    /// Dernière révision observée, pour dater la dernière modification.
    seen_revision: u64,
    last_edit: Instant,
}

impl OpenPage {
    fn new(id: NodeId, page: Page, history: History) -> Self {
        let revision = history.revision();
        Self {
            id,
            page,
            history,
            saved_revision: revision,
            seen_revision: revision,
            last_edit: Instant::now(),
        }
    }

    fn is_dirty(&self) -> bool {
        self.history.revision() != self.saved_revision
    }
}

struct CalameApp {
    store: Store,
    tree: LibraryTree,
    trash: Vec<Node>,
    current: Option<OpenPage>,
    /// Historiques des pages visitées pendant la session, pour annuler après un changement de page.
    histories: HashMap<NodeId, History>,
    canvas: Canvas,
    brush: Brush,
    dark: bool,
    sidebar: SidebarState,
    error: Option<String>,
}

fn database_path() -> PathBuf {
    if let Some(path) = std::env::var_os("CALAME_DB") {
        return PathBuf::from(path);
    }
    directories::ProjectDirs::from("org", "calame", "Calame")
        .map_or_else(|| PathBuf::from("."), |d| d.data_dir().to_path_buf())
        .join("calame.db")
}

impl CalameApp {
    fn new(cc: &eframe::CreationContext<'_>) -> anyhow::Result<Self> {
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        let path = database_path();
        let store = Store::open(&path)
            .with_context(|| format!("ouverture de la bibliothèque {}", path.display()))?;
        let mut app = Self {
            store,
            tree: LibraryTree::default(),
            trash: Vec::new(),
            current: None,
            histories: HashMap::new(),
            canvas: Canvas::default(),
            brush: Brush::default(),
            dark: false,
            sidebar: SidebarState::default(),
            error: None,
        };
        app.refresh();
        if app.tree.is_empty() {
            app.create_welcome_content(&cc.egui_ctx);
        }
        if let Some(first) = app.tree.first_page() {
            app.open_page(first);
        }
        Ok(app)
    }

    /// Première utilisation : une matière, un cahier et une page pour pouvoir écrire tout de suite.
    fn create_welcome_content(&mut self, ctx: &egui::Context) {
        let result = (|| -> calame_storage::Result<()> {
            let subject =
                self.store
                    .create_node(None, NodeKind::Subject, i18n::t("welcome-subject"))?;
            let notebook = self.store.create_node(
                Some(subject.id),
                NodeKind::Notebook,
                i18n::t("welcome-notebook"),
            )?;
            self.store
                .create_node(Some(notebook.id), NodeKind::Page, i18n::t("welcome-page"))?;
            library_ui::expand(ctx, subject.id);
            library_ui::expand(ctx, notebook.id);
            Ok(())
        })();
        self.report(result);
        self.refresh();
    }

    /// Affiche l'erreur éventuelle dans la barre d'outils et renvoie le résultat sinon.
    fn report<T>(&mut self, result: calame_storage::Result<T>) -> Option<T> {
        match result {
            Ok(v) => Some(v),
            Err(e) => {
                self.error = Some(e.to_string());
                None
            }
        }
    }

    fn refresh(&mut self) {
        let nodes = self.store.all_nodes();
        if let Some(nodes) = self.report(nodes) {
            self.tree = LibraryTree::from_nodes(nodes);
        }
        let trash = self.store.trashed();
        if let Some(trash) = self.report(trash) {
            self.trash = trash;
        }
        // La page ouverte a pu être supprimée, par exemple avec son cahier.
        if let Some(open) = &self.current {
            if self.tree.get(open.id).is_none() {
                self.current = None;
            }
        }
    }

    fn save_current(&mut self) {
        let Some(open) = &self.current else {
            return;
        };
        if !open.is_dirty() {
            return;
        }
        let (id, revision) = (open.id, open.history.revision());
        let result = self.store.save_page(id, &open.page);
        if self.report(result).is_some() {
            if let Some(open) = &mut self.current {
                open.saved_revision = revision;
            }
        }
    }

    fn close_current(&mut self) {
        self.save_current();
        if let Some(open) = self.current.take() {
            self.histories.insert(open.id, open.history);
        }
    }

    fn open_page(&mut self, id: NodeId) {
        if self.current.as_ref().is_some_and(|o| o.id == id) {
            return;
        }
        self.close_current();
        let loaded = self.store.load_page(id);
        if let Some(page) = self.report(loaded) {
            let history = self.histories.remove(&id).unwrap_or_default();
            self.current = Some(OpenPage::new(id, page, history));
            self.canvas = Canvas::default();
        }
    }

    fn autosave(&mut self, ctx: &egui::Context) {
        let Some(open) = &mut self.current else {
            return;
        };
        if open.history.revision() != open.seen_revision {
            open.seen_revision = open.history.revision();
            open.last_edit = Instant::now();
        }
        if open.is_dirty() {
            if open.last_edit.elapsed() >= AUTOSAVE_DELAY {
                self.save_current();
            } else {
                ctx.request_repaint_after(AUTOSAVE_DELAY);
            }
        }
    }

    fn apply(&mut self, ctx: &egui::Context, actions: Vec<Action>) {
        if actions.is_empty() {
            return;
        }
        for action in actions {
            match action {
                Action::Select(id) => self.open_page(id),
                Action::Create { parent, kind } => self.create(ctx, parent, kind),
                Action::Rename(id, name) => {
                    let r = self.store.rename(id, &name);
                    self.report(r);
                }
                Action::Duplicate(id) => {
                    self.save_current();
                    let r = self.store.duplicate(id);
                    self.report(r);
                }
                Action::Trash(id) => {
                    self.save_current();
                    let r = self.store.trash(id);
                    self.report(r);
                }
                Action::MoveBy(id, delta) => {
                    let r = self.store.move_by(id, delta);
                    self.report(r);
                }
                Action::Move(id, parent, index) => {
                    let r = self.store.move_node(id, parent, index);
                    if self.report(r).is_some() {
                        if let Some(p) = parent {
                            library_ui::expand(ctx, p);
                        }
                    }
                }
                Action::SetStyle(id, color, icon) => {
                    let r = self.store.set_style(id, color, &icon);
                    self.report(r);
                }
                Action::Restore(id) => {
                    let r = self.store.restore(id);
                    self.report(r);
                }
                Action::DeleteForever(id) => {
                    let r = self.store.delete_permanently(id);
                    self.report(r);
                }
                Action::EmptyTrash => {
                    let r = self.store.empty_trash();
                    self.report(r);
                }
            }
        }
        self.refresh();
    }

    fn create(&mut self, ctx: &egui::Context, parent: Option<NodeId>, kind: NodeKind) {
        let name = match kind {
            NodeKind::Subject => i18n::t("default-subject"),
            NodeKind::Notebook => i18n::t("default-notebook"),
            NodeKind::Folder => i18n::t("default-folder"),
            NodeKind::Page => i18n::t("default-page"),
        };
        let created = self.store.create_node(parent, kind, name);
        let Some(node) = self.report(created) else {
            return;
        };
        if let Some(p) = parent {
            library_ui::expand(ctx, p);
        }
        self.sidebar.start_rename(node.id, &node.name);
        if kind == NodeKind::Page {
            // `refresh` n'a pas encore eu lieu : on ouvre la page après la mise à jour de l'arbre.
            self.refresh();
            self.open_page(node.id);
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let pen = self.brush.kind == BrushKind::Pen;
            if ui.selectable_label(pen, i18n::t("tool-pen")).clicked() {
                self.brush.kind = BrushKind::Pen;
                self.brush.width = 3.0;
                self.brush.color = Color::BLACK;
            }
            if ui
                .selectable_label(!pen, i18n::t("tool-highlighter"))
                .clicked()
            {
                self.brush.kind = BrushKind::Highlighter;
                self.brush.width = 18.0;
                self.brush.color = Color {
                    r: 255,
                    g: 235,
                    b: 59,
                    a: 110,
                };
            }
            ui.separator();
            ui.add(egui::Slider::new(&mut self.brush.width, 1.0..=40.0).text(i18n::t("width")));
            ui.separator();
            let (can_undo, can_redo) = self.current.as_ref().map_or((false, false), |o| {
                (o.history.can_undo(), o.history.can_redo())
            });
            if ui
                .add_enabled(can_undo, egui::Button::new(i18n::t("undo")))
                .clicked()
            {
                self.undo();
            }
            if ui
                .add_enabled(can_redo, egui::Button::new(i18n::t("redo")))
                .clicked()
            {
                self.redo();
            }
            ui.separator();
            if ui.button(i18n::t("reset-view")).clicked() {
                self.canvas.view = View::default();
            }
            if ui.checkbox(&mut self.dark, i18n::t("dark-theme")).changed() {
                ui.ctx().set_visuals(if self.dark {
                    egui::Visuals::dark()
                } else {
                    egui::Visuals::light()
                });
            }
            if let Some(open) = &self.current {
                let path: Vec<&str> = self
                    .tree
                    .path(open.id)
                    .iter()
                    .map(|n| n.name.as_str())
                    .collect();
                ui.separator();
                ui.weak(path.join(" › "));
            }
        });
        if let Some(message) = self.error.clone() {
            ui.horizontal(|ui| {
                ui.colored_label(ui.visuals().error_fg_color, message);
                if ui.small_button(i18n::t("dismiss")).clicked() {
                    self.error = None;
                }
            });
        }
    }

    fn undo(&mut self) {
        if let Some(o) = &mut self.current {
            o.history.undo(&mut o.page);
        }
    }

    fn redo(&mut self) {
        if let Some(o) = &mut self.current {
            o.history.redo(&mut o.page);
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let undo = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Z);
        let redo = egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::Z,
        );
        let redo_y = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Y);
        // La combinaison la plus spécifique d'abord, sinon Ctrl+Maj+Z déclencherait « annuler ».
        if ctx.input_mut(|i| i.consume_shortcut(&redo) || i.consume_shortcut(&redo_y)) {
            self.redo();
        } else if ctx.input_mut(|i| i.consume_shortcut(&undo)) {
            self.undo();
        }
    }
}

impl eframe::App for CalameApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.shortcuts(&ctx);

        let mut actions = Vec::new();
        egui::Panel::left("library")
            .default_size(260.0)
            .resizable(true)
            .show(ui, |ui| {
                let selected = self.current.as_ref().map(|o| o.id);
                actions =
                    library_ui::show(ui, &mut self.sidebar, &self.tree, &self.trash, selected);
            });
        egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| match &mut self.current {
                Some(open) => {
                    self.canvas
                        .show(ui, &mut open.page, &mut open.history, self.brush);
                }
                None => {
                    ui.centered_and_justified(|ui| ui.weak(i18n::t("no-page")));
                }
            });

        self.apply(&ctx, actions);
        self.autosave(&ctx);
    }

    fn on_exit(&mut self) {
        self.save_current();
    }
}

fn main() -> anyhow::Result<()> {
    let mut wgpu_options = eframe::WgpuConfiguration::default();
    // Sous Windows, le choix automatique retient parfois Vulkan, dont certains pilotes plantent
    // au démarrage : on préfère DX12, avec OpenGL en secours. `WGPU_BACKEND` reste prioritaire.
    if cfg!(windows) && std::env::var_os("WGPU_BACKEND").is_none() {
        if let eframe::egui_wgpu::WgpuSetup::CreateNew(create_new) = &mut wgpu_options.wgpu_setup {
            create_new.instance_descriptor.backends =
                eframe::wgpu::Backends::DX12 | eframe::wgpu::Backends::GL;
        }
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Calame")
            .with_inner_size([1200.0, 800.0]),
        wgpu_options,
        ..Default::default()
    };
    eframe::run_native(
        "Calame",
        options,
        Box::new(|cc| Ok(Box::new(CalameApp::new(cc)?))),
    )
    .map_err(|e| anyhow!("échec du lancement de la fenêtre : {e}"))
}
