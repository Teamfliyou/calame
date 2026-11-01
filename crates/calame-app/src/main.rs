//! Calame : prise de notes manuscrites.

mod canvas;
mod i18n;
mod instruments_ui;
mod library_ui;
mod toolbar;
mod tools;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context as _};
use calame_core::{History, Item, LibraryTree, Node, NodeId, NodeKind, Page};
use calame_storage::Store;
use canvas::{Canvas, View};
use library_ui::{Action, SidebarState};
use toolbar::{Command, ToolbarState};
use tools::{Tool, ToolSettings};

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
    tools: ToolSettings,
    /// Éléments copiés ou coupés, collables dans n'importe quelle page.
    clipboard: Vec<Item>,
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
            tools: ToolSettings::default(),
            clipboard: Vec::new(),
            dark: false,
            sidebar: SidebarState::default(),
            error: None,
        };
        app.load_settings(&cc.egui_ctx);
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

    fn load_settings(&mut self, ctx: &egui::Context) {
        let favorites = self.store.setting("favorites");
        if let Some(Some(text)) = self.report(favorites) {
            self.tools.set_favorites_from_string(&text);
        }
        let shapes = self.store.setting("shapes");
        if let Some(Some(text)) = self.report(shapes) {
            self.tools.shape_recognition = text == "1";
        }
        let dark = self.store.setting("dark");
        if let Some(Some(text)) = self.report(dark) {
            self.dark = text == "1";
        }
        self.apply_theme(ctx);
    }

    fn apply_theme(&self, ctx: &egui::Context) {
        ctx.set_visuals(if self.dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
    }

    fn save_setting(&mut self, key: &str, value: &str) {
        let result = self.store.set_setting(key, value);
        self.report(result);
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
            // Les instruments suivent l'utilisateur d'une page à l'autre.
            let instruments = std::mem::take(&mut self.canvas.instruments);
            self.canvas = Canvas::default();
            self.canvas.instruments = instruments;
            self.canvas.sync_tool(self.tools.tool);
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
        let breadcrumb = self.current.as_ref().map(|open| {
            self.tree
                .path(open.id)
                .iter()
                .map(|n| n.name.as_str())
                .collect::<Vec<_>>()
                .join(" › ")
        });
        let (can_undo, can_redo) = self.current.as_ref().map_or((false, false), |o| {
            (o.history.can_undo(), o.history.can_redo())
        });
        let state = ToolbarState {
            can_undo,
            can_redo,
            has_selection: self.canvas.has_selection(),
            can_paste: !self.clipboard.is_empty(),
            has_active_instrument: self.canvas.has_active_instrument(),
            instrument_count: self.canvas.instrument_count(),
            dark: self.dark,
            breadcrumb: breadcrumb.as_deref(),
        };
        for command in toolbar::show(ui, &mut self.tools, &state) {
            self.run(ui.ctx(), command);
        }
        if let Some(message) = self.error.clone() {
            ui.horizontal(|ui| {
                ui.colored_label(ui.visuals().error_fg_color, message);
                if ui.small_button(i18n::t("dismiss")).clicked() {
                    self.error = None;
                }
            });
        }
    }

    fn run(&mut self, ctx: &egui::Context, command: Command) {
        match command {
            Command::Undo => self.undo(),
            Command::Redo => self.redo(),
            Command::ResetView => self.canvas.view = View::default(),
            Command::SetDark(dark) => {
                self.dark = dark;
                self.apply_theme(ctx);
                self.save_setting("dark", if dark { "1" } else { "0" });
            }
            Command::Copy => self.copy_selection(),
            Command::Cut => {
                self.copy_selection();
                self.delete_selection();
            }
            Command::Paste => self.paste(),
            Command::Delete => self.delete_selection(),
            Command::Recolor => {
                if let Some(o) = &mut self.current {
                    self.canvas
                        .recolor_selection(&mut o.page, &mut o.history, self.tools.color);
                }
            }
            Command::ShapesChanged => {
                let value = if self.tools.shape_recognition {
                    "1"
                } else {
                    "0"
                };
                self.save_setting("shapes", value);
            }
            Command::AddInstrument(kind) => {
                self.canvas.add_instrument(kind);
                self.tools.tool = Tool::Instruments;
            }
            Command::RemoveInstrument => self.canvas.remove_active_instrument(),
            Command::ClearInstruments => self.canvas.clear_instruments(),
            Command::FavoritesChanged => {
                let text = self.tools.favorites_to_string();
                self.save_setting("favorites", &text);
            }
        }
    }

    fn copy_selection(&mut self) {
        if let Some(o) = &self.current {
            let items = self.canvas.selected_items(&o.page);
            if !items.is_empty() {
                self.clipboard = items;
            }
        }
    }

    fn delete_selection(&mut self) {
        if let Some(o) = &mut self.current {
            self.canvas.delete_selection(&mut o.page, &mut o.history);
        }
    }

    fn paste(&mut self) {
        if self.clipboard.is_empty() {
            return;
        }
        if let Some(o) = &mut self.current {
            // Le collage sélectionne les nouveaux éléments : on passe au lasso pour les manipuler.
            self.tools.tool = Tool::Lasso;
            self.canvas.sync_tool(Tool::Lasso);
            self.canvas
                .paste(&mut o.page, &mut o.history, &self.clipboard);
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
        // Pendant une saisie (texte, renommage), les raccourcis appartiennent au champ.
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let undo = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Z);
        let redo = egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::Z,
        );
        let redo_y = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Y);
        let select_all = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::A);
        // La combinaison la plus spécifique d'abord, sinon Ctrl+Maj+Z déclencherait « annuler ».
        if ctx.input_mut(|i| i.consume_shortcut(&redo) || i.consume_shortcut(&redo_y)) {
            self.redo();
        } else if ctx.input_mut(|i| i.consume_shortcut(&undo)) {
            self.undo();
        }
        let (mut copy, mut cut, mut paste) = (false, false, false);
        let (delete, all, escape) = ctx.input_mut(|i| {
            for event in &i.events {
                match event {
                    egui::Event::Copy => copy = true,
                    egui::Event::Cut => cut = true,
                    egui::Event::Paste(_) => paste = true,
                    _ => {}
                }
            }
            (
                i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
                i.consume_shortcut(&select_all),
                i.key_pressed(egui::Key::Escape),
            )
        });
        if copy || cut {
            self.copy_selection();
        }
        if cut || delete {
            self.delete_selection();
        }
        if paste {
            self.paste();
        }
        if all {
            if let Some(o) = &self.current {
                self.tools.tool = Tool::Lasso;
                self.canvas.sync_tool(Tool::Lasso);
                self.canvas.select_all(&o.page);
            }
        }
        if escape {
            self.canvas.clear_selection();
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
                        .show(ui, &mut open.page, &mut open.history, &mut self.tools);
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
