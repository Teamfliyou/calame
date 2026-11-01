//! Menu Fichier, fonds de page, import et export : la partie de l'application qui touche aux
//! fichiers et aux images.

use calame_core::background::PATTERN_BACKGROUNDS;
use calame_core::{Background, NodeId, NodeKind, Page};
use calame_export::{import_image, BackgroundImage, PageBackground};
use calame_storage::ImportedPage;

use crate::canvas::Backdrop;
use crate::files::{
    export_pages, render_pdf_file, sanitize_file_name, Destination, ExportFormat, ExportPage, Task,
    TaskResult,
};
use crate::i18n::t;
use crate::CalameApp;

fn background_key(background: Background) -> &'static str {
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

impl CalameApp {
    /// Barre de menus : fichier (import, export) et fond de la page.
    pub(crate) fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let has_page = self.current.is_some();
        let mut export = None;
        let mut import_pdf = false;
        let mut import_image = false;
        let mut new_background = None;
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button(t("menu-file"), |ui| {
                if ui.button(t("import-pdf")).clicked() {
                    import_pdf = true;
                    ui.close();
                }
                ui.add_enabled_ui(has_page, |ui| {
                    ui.menu_button(t("export-page"), |ui| {
                        for (format, key) in [
                            (ExportFormat::Pdf, "export-pdf"),
                            (ExportFormat::Png, "export-png"),
                            (ExportFormat::Svg, "export-svg"),
                        ] {
                            if ui.button(t(key)).clicked() {
                                export = Some(format);
                                ui.close();
                            }
                        }
                    });
                });
            });
            ui.add_enabled_ui(has_page, |ui| {
                ui.menu_button(t("menu-background"), |ui| {
                    for background in PATTERN_BACKGROUNDS {
                        let selected = self.background == background;
                        if ui
                            .selectable_label(selected, t(background_key(background)))
                            .clicked()
                        {
                            new_background = Some(background);
                            ui.close();
                        }
                    }
                    ui.separator();
                    let on_image = matches!(self.background, Background::Image(_));
                    if ui.selectable_label(on_image, t("bg-pick-image")).clicked() {
                        import_image = true;
                        ui.close();
                    }
                });
            });
        });
        if let Some(background) = new_background {
            self.set_background(ui.ctx(), background);
        }
        if import_image {
            self.pick_background_image(ui.ctx());
        }
        if import_pdf {
            if let Some(target) = self.import_target() {
                self.import_pdf(target);
            }
        }
        if let (Some(format), Some(open)) = (export, &self.current) {
            let id = open.id;
            self.export_node(id, format);
        }
    }

    /// Où ranger un PDF importé depuis le menu : le conteneur de la page ouverte, sinon la
    /// première matière.
    fn import_target(&self) -> Option<NodeId> {
        let from_page = self
            .current
            .as_ref()
            .and_then(|open| self.tree.get(open.id))
            .and_then(|page| page.parent_id);
        from_page.or_else(|| self.tree.children(None).first().map(|n| n.id))
    }

    pub(crate) fn status_line(&mut self, ui: &mut egui::Ui) {
        if let Some(task) = &self.task {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(&task.label);
            });
        } else if let Some(message) = self.status.clone() {
            ui.horizontal(|ui| {
                ui.label(message);
                if ui.small_button(t("dismiss")).clicked() {
                    self.status = None;
                }
            });
        }
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

    fn set_background(&mut self, ctx: &egui::Context, background: Background) {
        let Some(id) = self.current.as_ref().map(|o| o.id) else {
            return;
        };
        let result = self.store.set_background(id, background);
        if self.report(result).is_some() {
            self.load_background(ctx, id);
        }
    }

    fn pick_background_image(&mut self, ctx: &egui::Context) {
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
                    self.refresh();
                    // La première page importée s'ouvre : on peut annoter tout de suite.
                    if let Some(first) = self.tree.children(Some(root.id)).first().map(|n| n.id) {
                        self.open_page(ctx, first);
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
