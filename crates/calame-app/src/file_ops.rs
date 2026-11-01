//! Fonds de page, import et export, sauvegarde : la partie de l'application qui touche aux
//! fichiers et aux images.

use std::path::PathBuf;

use calame_core::{Background, NodeId, NodeKind, Page};
use calame_export::{import_image, BackgroundImage, PageBackground};
use calame_storage::{restore_backup, ImportedPage, Store};

use crate::canvas::Backdrop;
use crate::files::{
    backup_file_name, export_pages, render_pdf_file, sanitize_file_name, Destination, ExportFormat,
    ExportPage, Task, TaskResult,
};
use crate::i18n::t;
use crate::{CalameApp, Screen};

impl CalameApp {
    /// Où ranger un PDF importé : dans un cahier, le dossier de la page ouverte (ou le cahier) ;
    /// dans la bibliothèque, la matière choisie, sinon la première, créée au besoin.
    pub(crate) fn import_target(&mut self) -> Option<NodeId> {
        if let Screen::Notebook(notebook) = self.screen {
            let from_page = self
                .current
                .as_ref()
                .and_then(|open| self.tree.get(open.id))
                .and_then(|page| page.parent_id);
            return Some(from_page.unwrap_or(notebook));
        }
        if let Some(subject) = self
            .home
            .filter()
            .or_else(|| self.tree.children(None).first().map(|n| n.id))
        {
            return Some(subject);
        }
        let created = self
            .store
            .create_node(None, NodeKind::Subject, t("imported-subject"));
        let subject = self.report(created)?.id;
        self.refresh();
        Some(subject)
    }

    // ---- Fonds de page ----------------------------------------------------------------

    /// Lit le fond de la page `id` et prépare sa texture si c'est une image.
    pub(crate) fn load_background(&mut self, ctx: &egui::Context, id: NodeId) {
        let loaded = self.store.background(id);
        self.background = self.report(loaded).unwrap_or_default();
        if let Background::Image(asset) = self.background {
            self.ensure_texture(ctx, asset);
        }
    }

    fn ensure_texture(&mut self, ctx: &egui::Context, asset: calame_core::AssetId) {
        if self.textures.contains_key(&asset) {
            return;
        }
        let loaded = self.store.asset(asset);
        let Some(asset_data) = self.report(loaded) else {
            return;
        };
        match image::load_from_memory(&asset_data.data) {
            Ok(decoded) => {
                let rgba = decoded.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
                let texture = ctx.load_texture(
                    format!("fond-{asset}"),
                    color_image,
                    egui::TextureOptions::LINEAR,
                );
                self.textures.insert(
                    asset,
                    (texture, egui::vec2(asset_data.width, asset_data.height)),
                );
            }
            Err(e) => self.error = Some(format!("Image de fond illisible : {e}")),
        }
    }

    pub(crate) fn backdrop(&self) -> Backdrop {
        let image = match self.background {
            Background::Image(asset) => self
                .textures
                .get(&asset)
                .map(|(texture, size)| (texture.id(), *size)),
            _ => None,
        };
        Backdrop {
            background: self.background,
            image,
        }
    }

    pub(crate) fn set_background(&mut self, ctx: &egui::Context, background: Background) {
        let Some(id) = self.current.as_ref().map(|o| o.id) else {
            return;
        };
        let result = self.store.set_background(id, background);
        if self.report(result).is_some() {
            self.load_background(ctx, id);
        }
    }

    pub(crate) fn pick_background_image(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .set_title(t("bg-pick-image"))
            .add_filter("Images", &["png", "jpg", "jpeg"])
            .pick_file()
        else {
            return;
        };
        let imported = std::fs::read(&path)
            .map_err(|e| format!("Lecture de {} impossible : {e}", path.display()))
            .and_then(|bytes| import_image(bytes).map_err(|e| e.to_string()));
        let image = match imported {
            Ok(image) => image,
            Err(message) => {
                self.error = Some(message);
                return;
            }
        };
        let stored = self
            .store
            .add_asset(&image.mime, image.width, image.height, &image.data);
        if let Some(asset) = self.report(stored) {
            self.set_background(ctx, Background::Image(asset));
        }
    }

    // ---- Export ----------------------------------------------------------------------

    /// Pages à exporter pour un élément : lui-même si c'est une page, sinon toutes les pages de
    /// son sous-arbre dans l'ordre de l'arborescence.
    fn pages_under(&self, id: NodeId) -> Vec<NodeId> {
        fn walk(app: &CalameApp, id: NodeId, out: &mut Vec<NodeId>) {
            match app.tree.get(id).map(|n| n.kind) {
                Some(NodeKind::Page) => out.push(id),
                Some(_) => {
                    for child in app.tree.children(Some(id)) {
                        walk(app, child.id, out);
                    }
                }
                None => {}
            }
        }
        let mut out = Vec::new();
        walk(self, id, &mut out);
        out
    }

    fn export_page_data(&mut self, id: NodeId) -> Option<ExportPage> {
        let name = self.tree.get(id)?.name.clone();
        // La page ouverte peut avoir des modifications pas encore enregistrées.
        let page: Page = match self.current.as_ref().filter(|o| o.id == id) {
            Some(open) => open.page.clone(),
            None => {
                let loaded = self.store.load_page(id);
                self.report(loaded)?
            }
        };
        let background = match self.store.background(id) {
            Ok(Background::Image(asset)) => {
                let loaded = self.store.asset(asset);
                let asset = self.report(loaded)?;
                PageBackground::Image(BackgroundImage {
                    mime: asset.mime,
                    data: asset.data,
                    width: asset.width,
                    height: asset.height,
                })
            }
            Ok(other) => PageBackground::Pattern(other),
            Err(e) => {
                self.error = Some(e.to_string());
                return None;
            }
        };
        Some(ExportPage {
            name,
            page,
            background,
        })
    }

    pub(crate) fn export_node(&mut self, id: NodeId, format: ExportFormat) {
        if self.task.is_some() {
            self.error = Some(t("busy").into());
            return;
        }
        let page_ids = self.pages_under(id);
        let Some(node_name) = self.tree.get(id).map(|n| n.name.clone()) else {
            return;
        };
        if page_ids.is_empty() {
            self.error = Some(t("nothing-to-export").into());
            return;
        }
        // Le choix du fichier ou du dossier se fait d'abord : l'utilisateur peut renoncer.
        let single = page_ids.len() == 1;
        let destination = if format == ExportFormat::Pdf || single {
            let file = format!("{}.{}", sanitize_file_name(&node_name), format.extension());
            rfd::FileDialog::new()
                .set_title(t("export"))
                .set_file_name(file)
                .add_filter(format.extension(), &[format.extension()])
                .save_file()
                .map(Destination::File)
        } else {
            rfd::FileDialog::new()
                .set_title(t("export-folder"))
                .pick_folder()
                .map(Destination::Folder)
        };
        let Some(destination) = destination else {
            return;
        };
        let pages: Vec<ExportPage> = page_ids
            .into_iter()
            .filter_map(|page| self.export_page_data(page))
            .collect();
        self.status = None;
        self.task = Some(Task::spawn(
            t("busy-export").into(),
            move || match export_pages(&pages, format, &destination) {
                Ok(message) => TaskResult::Message(message),
                Err(message) => TaskResult::Failed(message),
            },
        ));
    }

    // ---- Sauvegarde et restauration -----------------------------------------------------

    pub(crate) fn backup_library(&mut self) {
        // La page ouverte doit être dans la base avant d'en prendre une copie.
        self.save_current();
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let Some(path) = rfd::FileDialog::new()
            .set_title(t("backup"))
            .set_file_name(backup_file_name(seconds))
            .add_filter("zip", &["zip"])
            .save_file()
        else {
            return;
        };
        let result = self.store.backup_to(&path);
        if self.report(result).is_some() {
            self.status = Some(format!("Bibliothèque sauvegardée : {}", path.display()));
        }
    }

    pub(crate) fn restore_library(&mut self) {
        if self.task.is_some() {
            self.error = Some(t("busy").into());
            return;
        }
        let Some(zip) = rfd::FileDialog::new()
            .set_title(t("restore-backup"))
            .add_filter("zip", &["zip"])
            .pick_file()
        else {
            return;
        };
        let confirmed = rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Warning)
            .set_title(t("restore-backup"))
            .set_description(t("restore-confirm"))
            .set_buttons(rfd::MessageButtons::YesNo)
            .show()
            == rfd::MessageDialogResult::Yes;
        if !confirmed {
            return;
        }
        self.close_current();
        // Le fichier de la base doit être libéré avant d'être remplacé.
        let placeholder = Store::open_in_memory();
        let Some(placeholder) = self.report(placeholder) else {
            return;
        };
        drop(std::mem::replace(&mut self.store, placeholder));

        let path = crate::database_path();
        let restored = restore_backup(&zip, &path);
        // Dans tous les cas on rouvre la base : la restaurée, ou l'ancienne si la restauration a échoué.
        let reopened = Store::open(&path);
        let Some(store) = self.report(reopened) else {
            return;
        };
        self.store = store;
        self.histories.clear();
        self.textures.clear();
        self.thumbs.clear();
        self.current = None;
        self.background = Background::Blank;
        self.refresh();
        match restored {
            Ok(previous) => {
                self.screen = Screen::Library;
                self.status = Some(match previous {
                    Some(old) => format!(
                        "Bibliothèque restaurée. L'ancienne est conservée : {}",
                        old.display()
                    ),
                    None => "Bibliothèque restaurée.".into(),
                });
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    // ---- Import de PDF ---------------------------------------------------------------

    pub(crate) fn import_pdf(&mut self, parent: NodeId) {
        if self.task.is_some() {
            self.error = Some(t("busy").into());
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .set_title(t("import-pdf"))
            .add_filter("PDF", &["pdf"])
            .pick_file()
        else {
            return;
        };
        self.import_pdf_from(parent, path);
    }

    /// Importe le PDF `path` dans `parent` (fichier choisi, ou déposé sur la fenêtre).
    pub(crate) fn import_pdf_from(&mut self, parent: NodeId, path: PathBuf) {
        if self.task.is_some() {
            self.error = Some(t("busy").into());
            return;
        }
        self.status = None;
        self.task = Some(Task::spawn(
            t("busy-import").into(),
            move || match render_pdf_file(&path) {
                Ok((name, pages)) => TaskResult::ImportedPdf {
                    parent,
                    name,
                    pages,
                },
                Err(message) => TaskResult::Failed(message),
            },
        ));
    }

    /// Récupère le résultat d'une tâche terminée et l'applique.
    pub(crate) fn poll_task(&mut self, ctx: &egui::Context) {
        let Some(task) = &self.task else {
            return;
        };
        let Some(result) = task.poll() else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            return;
        };
        self.task = None;
        match result {
            TaskResult::Message(message) => self.status = Some(message),
            TaskResult::Failed(message) => self.error = Some(message),
            TaskResult::ImportedPdf {
                parent,
                name,
                pages,
            } => {
                let count = pages.len();
                let imported: Vec<ImportedPage> = pages
                    .into_iter()
                    .map(|p| ImportedPage {
                        mime: "image/png".into(),
                        width: p.width,
                        height: p.height,
                        data: p.png,
                    })
                    .collect();
                let result = self.store.import_document(parent, &name, &imported);
                if let Some(root) = self.report(result) {
                    crate::library_ui::expand(ctx, parent);
                    crate::library_ui::expand(ctx, root.id);
                    self.refresh();
                    // La première page importée s'ouvre : on peut annoter tout de suite.
                    if let Some(first) = self.tree.children(Some(root.id)).first().map(|n| n.id) {
                        self.go_to_page(ctx, first);
                    }
                    self.status = Some(format!(
                        "{count} page{} importée{} dans « {} »",
                        if count > 1 { "s" } else { "" },
                        if count > 1 { "s" } else { "" },
                        root.name
                    ));
                }
            }
        }
        ctx.request_repaint();
    }
}
