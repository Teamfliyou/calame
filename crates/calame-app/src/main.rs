//! Calame : prise de notes manuscrites.

mod canvas;
mod i18n;

use anyhow::anyhow;
use calame_core::{Brush, BrushKind, Color, History, Page};
use canvas::{Canvas, View};

struct CalameApp {
    page: Page,
    history: History,
    canvas: Canvas,
    brush: Brush,
    dark: bool,
}

impl CalameApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        Self {
            page: Page::default(),
            history: History::new(),
            canvas: Canvas::default(),
            brush: Brush::default(),
            dark: false,
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
            if ui
                .add_enabled(self.history.can_undo(), egui::Button::new(i18n::t("undo")))
                .clicked()
            {
                self.history.undo(&mut self.page);
            }
            if ui
                .add_enabled(self.history.can_redo(), egui::Button::new(i18n::t("redo")))
                .clicked()
            {
                self.history.redo(&mut self.page);
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
        });
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
            self.history.redo(&mut self.page);
        } else if ctx.input_mut(|i| i.consume_shortcut(&undo)) {
            self.history.undo(&mut self.page);
        }
    }
}

impl eframe::App for CalameApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.shortcuts(&ctx);
        egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                self.canvas
                    .show(ui, &mut self.page, &mut self.history, self.brush);
            });
    }
}

fn main() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Calame")
            .with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Calame",
        options,
        Box::new(|cc| Ok(Box::new(CalameApp::new(cc)))),
    )
    .map_err(|e| anyhow!("échec du lancement de la fenêtre : {e}"))
}
