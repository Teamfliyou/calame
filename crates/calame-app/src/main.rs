//! Calame : prise de notes manuscrites.

mod canvas;
mod dates;
mod file_ops;
mod files;
mod home_ui;
mod i18n;
mod icons;
mod instruments_ui;
mod library_ui;
mod palette_ui;
mod render_cache;
mod svg_path;
mod theme;
mod thumbs;
mod toolbar;
mod tools;
mod widgets;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context as _};
use calame_core::{AssetId, Background, History, Item, LibraryTree, Node, NodeId, NodeKind, Page};
use calame_storage::Store;
use canvas::Canvas;
use files::{ExportFormat, Task};
use home_ui::{HomeData, HomeState};
use library_ui::{Action, SidebarState};
use palette_ui::{PaletteData, PaletteState};
use thumbs::{ThumbSource, Thumbs};
use toolbar::{Command, EditorState, EditorUi, SaveState};
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

/// Écran affiché : la bibliothèque, ou l'intérieur d'un cahier.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Library,
    Notebook(NodeId),
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
    /// Thème sombre choisi (le mode concentration est toujours sombre).
    dark: bool,
    /// Mode concentration : ni barre du haut ni sommaire, mini-dock, thème sombre.
    focus: bool,
    sidebar: SidebarState,
    home: HomeState,
    editor: EditorUi,
    palette: Option<PaletteState>,
    screen: Screen,
    sidebar_visible: bool,
    /// Dernière page ouverte dans chaque cahier, pour la retrouver en y revenant.
    last_pages: HashMap<NodeId, NodeId>,
    error: Option<String>,
    /// Message de réussite (export terminé, import réussi), affiché en haut de l'écran.
    status: Option<String>,
    /// Fond de la page ouverte.
    background: Background,
    /// Textures des images de fond déjà chargées, avec leur taille en unités de page.
    textures: HashMap<AssetId, (egui::TextureHandle, egui::Vec2)>,
    /// Import ou export en cours dans un fil séparé.
    task: Option<Task>,
    /// Date de dernière modification de chaque page (secondes Unix).
    stamps: HashMap<NodeId, i64>,
    thumbs: Thumbs,
    /// Papier des nouvelles pages de chaque cahier.
    papers: HashMap<NodeId, Background>,
    /// Cahiers favoris, dans l'ordre où ils ont été ajoutés.
    favorites: Vec<NodeId>,
    /// Dernière matière créée : destination d'un cahier créé dans la même feuille.
    last_subject: Option<NodeId>,
}

fn database_path() -> PathBuf {
    if let Some(path) = std::env::var_os("CALAME_DB") {
        return PathBuf::from(path);
    }
    directories::ProjectDirs::from("org", "calame", "Calame")
        .map_or_else(|| PathBuf::from("."), |d| d.data_dir().to_path_buf())
        .join("calame.db")
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

fn paper_key(notebook: NodeId) -> String {
    format!("paper:{notebook}")
}

impl CalameApp {
    fn new(cc: &eframe::CreationContext<'_>) -> anyhow::Result<Self> {
        let path = database_path();
        let store = Store::open(&path)
            .with_context(|| format!("ouverture de la bibliothèque {}", path.display()))?;
        theme::install_fonts(&cc.egui_ctx);
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
            focus: false,
            sidebar: SidebarState::default(),
            home: HomeState::default(),
            editor: EditorUi::default(),
            palette: None,
            screen: Screen::Library,
            sidebar_visible: true,
            last_pages: HashMap::new(),
            error: None,
            status: None,
            background: Background::Blank,
            textures: HashMap::new(),
            task: None,
            stamps: HashMap::new(),
            thumbs: Thumbs::default(),
            papers: HashMap::new(),
            favorites: Vec::new(),
            last_subject: None,
        };
        app.load_settings(&cc.egui_ctx);
        app.refresh();
        Ok(app)
    }

    /// Affiche l'erreur éventuelle et renvoie le résultat sinon.
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
        let stamps = self.store.page_timestamps();
        if let Some(stamps) = self.report(stamps) {
            self.stamps = stamps;
        }
        let notebooks: Vec<NodeId> = self
            .tree
            .children(None)
            .iter()
            .flat_map(|s| self.tree.children(Some(s.id)))
            .map(|n| n.id)
            .collect();
        self.papers.clear();
        for notebook in notebooks {
            if let Ok(Some(text)) = self.store.setting(&paper_key(notebook)) {
                if let Some(paper) = Background::from_db(&text) {
                    self.papers.insert(notebook, paper);
                }
            }
        }
        let tree = &self.tree;
        self.favorites
            .retain(|id| tree.get(*id).is_some_and(|n| n.kind == NodeKind::Notebook));
        // La page ouverte a pu être supprimée, par exemple avec son cahier.
        if let Some(open) = &self.current {
            if self.tree.get(open.id).is_none() {
                self.current = None;
                self.focus = false;
            }
        }
        if let Screen::Notebook(id) = self.screen {
            if self.tree.get(id).is_none() {
                self.screen = Screen::Library;
            }
        }
    }

    fn load_settings(&mut self, ctx: &egui::Context) {
        let read = |app: &mut Self, key: &str| {
            let value = app.store.setting(key);
            app.report(value).flatten()
        };
        if let Some(text) = read(self, "favorites") {
            self.tools.set_favorites_from_string(&text);
        }
        if let Some(text) = read(self, "pens") {
            self.tools.set_pens_from_string(&text);
        }
        if let Some(text) = read(self, "shapes") {
            self.tools.shape_recognition = text == "1";
        }
        if let Some(text) = read(self, "dark") {
            self.dark = text == "1";
        }
        if let Some(text) = read(self, "favorite-notebooks") {
            self.favorites = text.split(',').filter_map(|id| id.parse().ok()).collect();
        }
        // Le premier lancement commence avec le premier stylo de la trousse.
        if let Some(pen) = self.tools.pens.first().copied() {
            self.tools.apply_pen(pen);
        }
        self.apply_theme(ctx);
    }

    /// Thème affiché : sombre si choisi, et toujours en mode concentration.
    fn shown_dark(&self) -> bool {
        self.dark || self.focus
    }

    fn apply_theme(&self, ctx: &egui::Context) {
        theme::apply(ctx, self.shown_dark());
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
            self.stamps.insert(id, unix_now());
        }
    }

    fn close_current(&mut self) {
        self.save_current();
        if let Some(open) = self.current.take() {
            self.histories.insert(open.id, open.history);
        }
    }

    fn open_page(&mut self, ctx: &egui::Context, id: NodeId) {
        if self.current.as_ref().is_some_and(|o| o.id == id) {
            return;
        }
        self.close_current();
        let loaded = self.store.load_page(id);
        if let Some(page) = self.report(loaded) {
            let history = self.histories.remove(&id).unwrap_or_default();
            self.current = Some(OpenPage::new(id, page, history));
            if let Some(notebook) = self.tree.notebook_of(id) {
                self.last_pages.insert(notebook, id);
            }
            // Les dossiers qui contiennent la page sont dépliés dans le sommaire.
            for ancestor in self.tree.path(id) {
                if ancestor.kind == NodeKind::Folder {
                    library_ui::expand(ctx, ancestor.id);
                }
            }
            self.load_background(ctx, id);
            // Les instruments suivent l'utilisateur d'une page à l'autre.
            let instruments = std::mem::take(&mut self.canvas.instruments);
            self.canvas = Canvas::default();
            self.canvas.instruments = instruments;
            self.canvas.sync_tool(self.tools.tool);
        }
    }

    /// Entre dans un cahier et rouvre sa dernière page (ou la première).
    fn open_notebook(&mut self, ctx: &egui::Context, id: NodeId) {
        if self.tree.get(id).is_none() {
            return;
        }
        self.close_current();
        self.screen = Screen::Notebook(id);
        self.sidebar = SidebarState::default();
        let remembered = self
            .last_pages
            .get(&id)
            .copied()
            .filter(|page| self.tree.is_within(*page, id));
        if let Some(page) = remembered.or_else(|| self.tree.first_page_in(id)) {
            self.open_page(ctx, page);
        }
    }

    /// Ouvre une page de n'importe quel cahier.
    fn go_to_page(&mut self, ctx: &egui::Context, page: NodeId) {
        let Some(notebook) = self.tree.notebook_of(page) else {
            return;
        };
        if self.screen != Screen::Notebook(notebook) {
            self.close_current();
            self.screen = Screen::Notebook(notebook);
            self.sidebar = SidebarState::default();
        }
        self.open_page(ctx, page);
    }

    fn go_library(&mut self, ctx: &egui::Context) {
        self.close_current();
        self.set_focus(ctx, false);
        self.screen = Screen::Library;
    }

    fn set_focus(&mut self, ctx: &egui::Context, on: bool) {
        let on = on && self.current.is_some();
        if on == self.focus {
            return;
        }
        self.focus = on;
        self.editor.bubble = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(on));
        self.apply_theme(ctx);
    }

    /// Papier des nouvelles pages d'un cahier (uni si aucun n'a été choisi).
    fn paper_of(&self, node: NodeId) -> Background {
        self.tree
            .notebook_of(node)
            .and_then(|nb| self.papers.get(&nb).copied())
            .unwrap_or(Background::Blank)
    }

    /// Crée une page dans `parent`, avec le papier du cahier.
    fn create_page(&mut self, parent: NodeId, name: &str) -> Option<Node> {
        let created = self.store.create_node(Some(parent), NodeKind::Page, name);
        let node = self.report(created)?;
        let paper = self.paper_of(parent);
        if paper != Background::Blank {
            let set = self.store.set_background(node.id, paper);
            self.report(set);
        }
        Some(node)
    }

    /// Crée un cahier avec sa première page, pour pouvoir écrire tout de suite.
    fn create_notebook(
        &mut self,
        ctx: &egui::Context,
        subject: Option<NodeId>,
        name: &str,
        color: calame_core::Color,
        paper: Background,
    ) {
        let subject = subject
            .or(self.last_subject)
            .filter(|s| {
                self.tree
                    .get(*s)
                    .is_some_and(|n| n.kind == NodeKind::Subject)
            })
            .or_else(|| self.tree.children(None).first().map(|s| s.id));
        let subject = match subject {
            Some(s) => s,
            None => {
                let created =
                    self.store
                        .create_node(None, NodeKind::Subject, i18n::t("welcome-subject"));
                match self.report(created) {
                    Some(node) => node.id,
                    None => return,
                }
            }
        };
        let created = self
            .store
            .create_node(Some(subject), NodeKind::Notebook, name);
        let Some(notebook) = self.report(created) else {
            return;
        };
        let styled = self
            .store
            .set_style(notebook.id, color, NodeKind::Notebook.default_icon());
        self.report(styled);
        self.save_setting(&paper_key(notebook.id), &paper.to_db());
        self.refresh();
        self.create_page(notebook.id, i18n::t("welcome-page"));
        self.refresh();
        self.open_notebook(ctx, notebook.id);
    }

    /// Note rapide : une page neuve dans le cahier « Notes rapides », créé au besoin.
    fn quick_note(&mut self, ctx: &egui::Context) {
        let remembered = self.store.setting("quick-notebook");
        let notebook = self
            .report(remembered)
            .flatten()
            .and_then(|text| text.parse::<NodeId>().ok())
            .filter(|id| {
                self.tree
                    .get(*id)
                    .is_some_and(|n| n.kind == NodeKind::Notebook)
            });
        let notebook = match notebook {
            Some(id) => id,
            None => {
                let subject = match self.tree.children(None).first().map(|s| s.id) {
                    Some(s) => s,
                    None => {
                        let created = self.store.create_node(
                            None,
                            NodeKind::Subject,
                            i18n::t("welcome-subject"),
                        );
                        match self.report(created) {
                            Some(node) => node.id,
                            None => return,
                        }
                    }
                };
                let created = self.store.create_node(
                    Some(subject),
                    NodeKind::Notebook,
                    i18n::t("quick-notes"),
                );
                let Some(node) = self.report(created) else {
                    return;
                };
                let styled = self.store.set_style(
                    node.id,
                    theme::COVER_COLORS[3],
                    NodeKind::Notebook.default_icon(),
                );
                self.report(styled);
                self.save_setting("quick-notebook", &node.id.to_string());
                node.id
            }
        };
        self.refresh();
        if let Some(page) = self.create_page(notebook, &dates::note_title(dates::now())) {
            self.refresh();
            self.go_to_page(ctx, page.id);
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
                Action::Select(id) => {
                    // Les pages créées juste avant ne sont dans l'arbre qu'après une mise à jour.
                    self.refresh();
                    self.open_page(ctx, id);
                }
                Action::OpenPage(id) => {
                    self.refresh();
                    self.go_to_page(ctx, id);
                }
                Action::OpenNotebook(id) => {
                    self.refresh();
                    self.open_notebook(ctx, id);
                }
                Action::CreateWith {
                    parent,
                    kind,
                    name,
                    color,
                    icon,
                } => {
                    let created = self.store.create_node(parent, kind, &name);
                    if let Some(node) = self.report(created) {
                        let styled = self.store.set_style(node.id, color, &icon);
                        self.report(styled);
                        if kind == NodeKind::Subject {
                            self.last_subject = Some(node.id);
                        }
                        self.refresh();
                    }
                }
                Action::CreateNotebook {
                    subject,
                    name,
                    color,
                    paper,
                } => self.create_notebook(ctx, subject, &name, color, paper),
                Action::EditNotebook {
                    id,
                    name,
                    color,
                    subject,
                    paper,
                } => {
                    let r = self.store.rename(id, &name);
                    self.report(r);
                    let icon = self
                        .tree
                        .get(id)
                        .map(|n| n.icon.clone())
                        .unwrap_or_default();
                    let r = self.store.set_style(id, color, &icon);
                    self.report(r);
                    let parent = self.tree.get(id).and_then(|n| n.parent_id);
                    if subject.is_some() && subject != parent {
                        let r = self.store.move_node(id, subject, None);
                        self.report(r);
                    }
                    self.save_setting(&paper_key(id), &paper.to_db());
                }
                Action::Export(id, format) => self.export_node(id, format),
                Action::ImportPdf(parent) => self.import_pdf(parent),
                Action::ImportPdfPath(parent, path) => {
                    if let Some(parent) = parent.or_else(|| self.import_target()) {
                        self.import_pdf_from(parent, path);
                    }
                }
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
                Action::ToggleFavorite(id) => {
                    if let Some(i) = self.favorites.iter().position(|f| *f == id) {
                        self.favorites.remove(i);
                    } else {
                        self.favorites.push(id);
                    }
                    let text = self
                        .favorites
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(",");
                    self.save_setting("favorite-notebooks", &text);
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
                Action::Command(command) => self.run(ctx, command),
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
        let node = match (kind, parent) {
            (NodeKind::Page, Some(parent)) => self.create_page(parent, name),
            _ => {
                let created = self.store.create_node(parent, kind, name);
                self.report(created)
            }
        };
        let Some(node) = node else {
            return;
        };
        if let Some(p) = parent {
            library_ui::expand(ctx, p);
        }
        self.sidebar.start_rename(node.id, &node.name);
        if kind == NodeKind::Page {
            // `refresh` n'a pas encore eu lieu : on ouvre la page après la mise à jour de l'arbre.
            self.refresh();
            self.open_page(ctx, node.id);
        }
    }

    /// Dossier où iraient une nouvelle page ou un nouveau dossier du cahier ouvert.
    fn add_target(&self) -> Option<NodeId> {
        match self.screen {
            Screen::Notebook(notebook) => Some(self.sidebar.target(
                &self.tree,
                notebook,
                self.current.as_ref().map(|o| o.id),
            )),
            Screen::Library => None,
        }
    }

    fn select_tool(&mut self, tool: Tool) {
        let tool = if matches!(tool, Tool::EraserStroke | Tool::EraserArea) {
            self.tools.eraser
        } else {
            tool
        };
        if tool == Tool::Pipette && self.tools.tool != Tool::Pipette {
            self.tools.previous = self.tools.tool;
        }
        self.tools.tool = tool;
    }

    fn run(&mut self, ctx: &egui::Context, command: Command) {
        match command {
            Command::Undo => self.undo(),
            Command::Redo => self.redo(),
            Command::ResetView => self.canvas.recenter(),
            Command::ZoomIn => self.canvas.zoom_by(1.25),
            Command::ZoomOut => self.canvas.zoom_by(0.8),
            Command::BackToLibrary => self.go_library(ctx),
            Command::ToggleSidebar => self.sidebar_visible = !self.sidebar_visible,
            Command::ToggleFocus => self.set_focus(ctx, !self.focus),
            Command::OpenPalette => {
                self.palette = if self.palette.is_some() {
                    None
                } else {
                    Some(PaletteState::new())
                };
            }
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
            Command::SelectAll => {
                if let Some(o) = &self.current {
                    self.tools.tool = Tool::Lasso;
                    self.canvas.sync_tool(Tool::Lasso);
                    self.canvas.select_all(&o.page);
                }
            }
            Command::Recolor => {
                if let Some(o) = &mut self.current {
                    self.canvas
                        .recolor_selection(&mut o.page, &mut o.history, self.tools.color);
                }
            }
            Command::Duplicate => {
                if let Some(o) = &mut self.current {
                    self.canvas.duplicate_selection(&mut o.page, &mut o.history);
                }
            }
            Command::Straighten => {
                if let Some(o) = &mut self.current {
                    let count = self
                        .canvas
                        .straighten_selection(&mut o.page, &mut o.history);
                    self.status = Some(if count == 0 {
                        i18n::t("straighten-none").to_owned()
                    } else {
                        format!("{count} {}", i18n::t("straighten-done"))
                    });
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
            Command::PensChanged => {
                let text = self.tools.pens_to_string();
                self.save_setting("pens", &text);
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
            Command::ExportPage(format) => {
                if let Some(id) = self.current.as_ref().map(|o| o.id) {
                    self.export_node(id, format);
                }
            }
            Command::ExportNotebook(format) => {
                if let Screen::Notebook(id) = self.screen {
                    self.export_node(id, format);
                }
            }
            Command::ImportPdf => {
                if let Some(target) = self.import_target() {
                    self.import_pdf(target);
                }
            }
            Command::Backup => self.backup_library(),
            Command::RestoreBackup => self.restore_library(),
            Command::SetBackground(background) => self.set_background(ctx, background),
            Command::PickBackgroundImage => self.pick_background_image(ctx),
            Command::NewPage => match self.add_target() {
                Some(target) => self.create(ctx, Some(target), NodeKind::Page),
                None => self.quick_note(ctx),
            },
            Command::NewFolder => {
                if let Some(target) = self.add_target() {
                    self.create(ctx, Some(target), NodeKind::Folder);
                }
            }
            Command::QuickNote => self.quick_note(ctx),
            Command::NewNotebook => {
                let subject = match self.screen {
                    Screen::Notebook(id) => self.tree.get(id).and_then(|n| n.parent_id),
                    Screen::Library => None,
                };
                self.home.open_new_notebook(&self.tree, subject);
            }
            Command::NewSubject => self.home.open_new_subject(&self.tree),
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
        // Une feuille ou la palette ouverte gère son propre clavier.
        if self.palette.is_some() || self.home.sheet.is_some() {
            return;
        }
        let command = |key| egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, key);
        if ctx.input_mut(|i| i.consume_shortcut(&command(egui::Key::K))) {
            self.palette = Some(PaletteState::new());
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F11)) {
            self.set_focus(ctx, !self.focus);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&command(egui::Key::N))) {
            self.run(ctx, Command::NewPage);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&command(egui::Key::E))) {
            self.run(ctx, Command::ExportPage(ExportFormat::Pdf));
        }
        // Pendant une saisie (texte, renommage), les autres raccourcis appartiennent au champ.
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let undo = command(egui::Key::Z);
        let redo = egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::Z,
        );
        let redo_y = command(egui::Key::Y);
        let select_all = command(egui::Key::A);
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
            self.run(ctx, Command::SelectAll);
        }
        if escape {
            if self.editor.bubble {
                self.editor.bubble = false;
            } else if self.canvas.has_selection() {
                self.canvas.clear_selection();
            } else if self.focus {
                self.set_focus(ctx, false);
            }
        }
        // Outils : une lettre, sans modificateur.
        if self.current.is_some() {
            for (key, tool) in [
                (egui::Key::P, Tool::Pen),
                (egui::Key::H, Tool::Highlighter),
                (egui::Key::E, Tool::EraserStroke),
                (egui::Key::L, Tool::Lasso),
                (egui::Key::T, Tool::Text),
                (egui::Key::I, Tool::Pipette),
                (egui::Key::R, Tool::Instruments),
            ] {
                if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key)) {
                    self.select_tool(tool);
                }
            }
        }
    }
}

impl CalameApp {
    fn editor_state(&self) -> EditorState {
        let notebook = match self.screen {
            Screen::Notebook(id) => self.tree.get(id),
            Screen::Library => None,
        };
        let subject = notebook
            .and_then(|n| n.parent_id)
            .and_then(|p| self.tree.get(p))
            .map(|s| (s.name.clone(), theme::to_color32(s.color)));
        let page = self
            .current
            .as_ref()
            .and_then(|o| self.tree.get(o.id))
            .map(|n| n.name.clone());
        let save = match &self.current {
            Some(open) if open.is_dirty() => SaveState::Saving,
            Some(_) => SaveState::Saved,
            None => SaveState::Nothing,
        };
        let (can_undo, can_redo) = self.current.as_ref().map_or((false, false), |o| {
            (o.history.can_undo(), o.history.can_redo())
        });
        EditorState {
            can_undo,
            can_redo,
            has_selection: self.canvas.has_selection(),
            can_paste: !self.clipboard.is_empty(),
            has_page: self.current.is_some(),
            has_active_instrument: self.canvas.has_active_instrument(),
            instrument_count: self.canvas.instrument_count(),
            dark: self.shown_dark(),
            subject,
            notebook: notebook.map_or_else(String::new, |n| n.name.clone()),
            page,
            save,
            zoom: self.canvas.view.zoom,
            background: self.background,
        }
    }

    /// Intérieur d'un cahier : barre du haut, sommaire, page et dock.
    fn notebook_screen(&mut self, ui: &mut egui::Ui, id: NodeId, actions: &mut Vec<Action>) {
        let ctx = ui.ctx().clone();
        let dark = self.shown_dark();
        let state = self.editor_state();
        let mut commands = Vec::new();
        if !self.focus {
            egui::Panel::top("barre-du-haut")
                .exact_size(56.0)
                .frame(
                    egui::Frame::new()
                        .fill(theme::surface(dark))
                        .inner_margin(egui::Margin::symmetric(12, 6)),
                )
                .show(ui, |ui| commands.extend(toolbar::top_bar(ui, &state)));
            if self.sidebar_visible {
                egui::Panel::left("sommaire")
                    .default_size(248.0)
                    .size_range(210.0..=420.0)
                    .resizable(true)
                    .frame(egui::Frame::new().fill(theme::sidebar(dark)))
                    .show(ui, |ui| {
                        let selected = self.current.as_ref().map(|o| o.id);
                        let open = self
                            .current
                            .as_ref()
                            .map(|o| (o.id, &o.page, o.history.revision(), self.background));
                        let mut thumbs = ThumbSource {
                            store: &self.store,
                            cache: &mut self.thumbs,
                            stamps: &self.stamps,
                            open,
                        };
                        actions.extend(library_ui::show(
                            ui,
                            &mut self.sidebar,
                            &self.tree,
                            id,
                            selected,
                            &mut thumbs,
                        ));
                    });
            }
        }
        let backdrop = self.backdrop();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::desk(dark)))
            .show(ui, |ui| match &mut self.current {
                Some(open) => {
                    self.canvas.show(
                        ui,
                        &mut open.page,
                        &mut open.history,
                        &mut self.tools,
                        &backdrop,
                    );
                }
                None => {
                    let top = ui.available_height() * 0.35;
                    ui.vertical_centered(|ui| {
                        ui.add_space(top);
                        ui.label(
                            egui::RichText::new(i18n::t("no-page"))
                                .color(theme::label_secondary(dark)),
                        );
                        ui.add_space(12.0);
                        if widgets::Button::new(
                            i18n::t("new-page-big"),
                            widgets::ButtonStyle::Primary,
                        )
                        .icon(icons::Icon::Plus)
                        .show(ui)
                        .clicked()
                        {
                            actions.push(Action::Create {
                                parent: Some(id),
                                kind: NodeKind::Page,
                            });
                        }
                    });
                }
            });

        if self.current.is_some() {
            let canvas = self.canvas_rect(&ctx);
            if self.focus {
                let label = self.focus_label();
                commands.extend(toolbar::focus_overlays(&ctx, canvas, &label));
                let can_undo = state.can_undo;
                commands.extend(toolbar::mini_dock(
                    &ctx,
                    &mut self.tools,
                    &mut self.editor,
                    can_undo,
                ));
                commands.extend(toolbar::bubble(
                    &ctx,
                    &mut self.tools,
                    &mut self.editor,
                    &state,
                    90.0,
                ));
            } else {
                commands.extend(toolbar::dock(&ctx, &mut self.tools, &mut self.editor, dark));
                commands.extend(toolbar::bubble(
                    &ctx,
                    &mut self.tools,
                    &mut self.editor,
                    &state,
                    88.0,
                ));
                commands.extend(toolbar::zoom_pill(&ctx, canvas, state.zoom, dark));
            }
            if let Some(open) = &self.current {
                if let Some(selection) = self.canvas.selection_screen_rect(&open.page) {
                    if canvas.intersects(selection) {
                        commands.extend(toolbar::selection_bar(
                            &ctx,
                            selection,
                            canvas,
                            self.tools.color,
                        ));
                    }
                }
            }
        }
        for command in commands {
            self.run(&ctx, command);
        }
    }

    fn canvas_rect(&self, ctx: &egui::Context) -> egui::Rect {
        let rect = self.canvas.rect();
        if rect.is_positive() {
            rect
        } else {
            ctx.content_rect()
        }
    }

    /// « Cahier · page N » en mode concentration.
    fn focus_label(&self) -> String {
        let Some(open) = &self.current else {
            return String::new();
        };
        let notebook = self.tree.notebook_of(open.id);
        let name = notebook
            .and_then(|n| self.tree.get(n))
            .map_or("", |n| n.name.as_str());
        let n = notebook
            .map(|nb| library_ui::page_order(&self.tree, nb))
            .and_then(|order| order.iter().position(|p| *p == open.id))
            .map_or(0, |i| i + 1);
        format!("{name} · {} {n}", i18n::t("page").to_lowercase())
    }

    /// Messages passagers (erreur, réussite, tâche en cours) en haut au centre.
    fn toasts(&mut self, ctx: &egui::Context) {
        let (text, error, busy) = if let Some(task) = &self.task {
            (task.label.clone(), false, true)
        } else if let Some(message) = &self.error {
            (message.clone(), true, false)
        } else if let Some(message) = &self.status {
            (message.clone(), false, false)
        } else {
            return;
        };
        let top = if matches!(self.screen, Screen::Notebook(_)) && !self.focus {
            68.0
        } else {
            16.0
        };
        let mut dismiss = false;
        egui::Area::new(egui::Id::unique("calame-message"))
            .anchor(egui::Align2::CENTER_TOP, [0.0, top])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(if error {
                        theme::danger(false)
                    } else {
                        theme::INK_BUTTON
                    })
                    .corner_radius(12)
                    .shadow(theme::dock_shadow(false))
                    .inner_margin(egui::Margin {
                        left: 16,
                        right: 6,
                        top: 6,
                        bottom: 6,
                    })
                    .show(ui, |ui| {
                        ui.set_max_width(560.0);
                        ui.horizontal(|ui| {
                            ui.visuals_mut().dark_mode = true;
                            if busy {
                                ui.add(egui::Spinner::new().color(egui::Color32::WHITE));
                            }
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(text)
                                        .font(theme::font(13.0, theme::Weight::Medium))
                                        .color(egui::Color32::WHITE),
                                )
                                .wrap(),
                            );
                            if !busy {
                                let (rect, response) = ui.allocate_exact_size(
                                    egui::Vec2::splat(32.0),
                                    egui::Sense::click(),
                                );
                                if response.hovered() {
                                    ui.painter().rect_filled(
                                        rect,
                                        8.0,
                                        egui::Color32::from_white_alpha(28),
                                    );
                                }
                                icons::paint(
                                    ui.painter(),
                                    rect.shrink(9.0),
                                    icons::Icon::Close,
                                    egui::Color32::WHITE,
                                );
                                if response.on_hover_text(i18n::t("dismiss")).clicked() {
                                    dismiss = true;
                                }
                            }
                        });
                    });
            });
        if dismiss {
            if error {
                self.error = None;
            } else {
                self.status = None;
            }
        }
    }

    /// PDF déposés sur la fenêtre : importés dans la matière choisie ou le cahier ouvert.
    fn dropped_files(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if dropped.is_empty() {
            return;
        }
        let pdf = dropped
            .into_iter()
            .map(|f| f.path().to_path_buf())
            .find(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
            });
        match pdf {
            Some(path) => actions.push(Action::ImportPdfPath(None, path)),
            None => self.error = Some(i18n::t("drop-only-pdf").into()),
        }
    }
}

impl eframe::App for CalameApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.shortcuts(&ctx);
        if self.focus && self.current.is_none() {
            self.set_focus(&ctx, false);
        }

        let mut actions = Vec::new();
        let dark = self.shown_dark();
        match self.screen {
            Screen::Library => {
                let dragging_files = ctx.input(|i| !i.raw.hovered_files.is_empty());
                let open = self
                    .current
                    .as_ref()
                    .map(|o| (o.id, &o.page, o.history.revision(), self.background));
                let mut thumbs = ThumbSource {
                    store: &self.store,
                    cache: &mut self.thumbs,
                    stamps: &self.stamps,
                    open,
                };
                let data = HomeData {
                    tree: &self.tree,
                    trash: &self.trash,
                    stamps: &self.stamps,
                    favorites: &self.favorites,
                    papers: &self.papers,
                    dark,
                    dragging_files,
                };
                actions = home_ui::show(ui, &mut self.home, &data, &mut thumbs);
            }
            Screen::Notebook(id) => self.notebook_screen(ui, id, &mut actions),
        }

        home_ui::sheet_window(&ctx, &mut self.home, &self.tree, &mut actions);
        if self.palette.is_some() {
            let notebook = match self.screen {
                Screen::Notebook(id) => Some(id),
                Screen::Library => None,
            };
            let data = PaletteData {
                tree: &self.tree,
                stamps: &self.stamps,
                notebook,
                target: self.add_target(),
                has_page: self.current.is_some(),
                can_paste: !self.clipboard.is_empty(),
                dark,
            };
            let open = self
                .current
                .as_ref()
                .map(|o| (o.id, &o.page, o.history.revision(), self.background));
            let mut thumbs = ThumbSource {
                store: &self.store,
                cache: &mut self.thumbs,
                stamps: &self.stamps,
                open,
            };
            actions.extend(palette_ui::show(
                &ctx,
                &mut self.palette,
                &data,
                &mut thumbs,
            ));
        }
        self.toasts(&ctx);
        self.dropped_files(&ctx, &mut actions);

        self.apply(&ctx, actions);
        self.autosave(&ctx);
        self.poll_task(&ctx);
    }

    fn on_exit(&mut self) {
        self.save_current();
    }
}

/// Icône de la fenêtre, intégrée au programme (voir `assets/make_icon.py`).
fn window_icon() -> Option<egui::IconData> {
    let png = include_bytes!("../../../assets/icon.png");
    let image = image::load_from_memory(png).ok()?.to_rgba8();
    Some(egui::IconData {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    })
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
    // Latence minimale : CALAME_VSYNC=0 présente les images sans attendre le rafraîchissement de
    // l'écran, au prix possible d'un déchirement. Par défaut, la synchronisation reste active.
    if std::env::var("CALAME_VSYNC").is_ok_and(|v| v == "0") {
        wgpu_options.surface.present_mode = eframe::wgpu::PresentMode::AutoNoVsync;
    }
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Calame")
        .with_app_id("calame")
        .with_inner_size([1280.0, 840.0])
        .with_min_inner_size([900.0, 560.0])
        .with_drag_and_drop(true);
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }
    let options = eframe::NativeOptions {
        viewport,
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

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn the_embedded_window_icon_is_a_valid_square_image() {
        let icon = window_icon().expect("icône lisible");
        assert_eq!(icon.width, icon.height);
        assert_eq!(icon.rgba.len(), (icon.width * icon.height * 4) as usize);
        // Le fond arrondi est opaque au centre et transparent dans le coin.
        let at = |x: u32, y: u32| icon.rgba[((y * icon.width + x) * 4 + 3) as usize];
        assert_eq!(at(icon.width / 2, icon.height - 40), 255);
        assert_eq!(at(0, 0), 0);
    }

    #[test]
    fn notebook_papers_have_their_own_setting_key() {
        let id = NodeId::new();
        assert_eq!(paper_key(id), format!("paper:{id}"));
    }
}
