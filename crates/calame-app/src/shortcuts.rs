//! Raccourcis clavier des outils du bandeau : réglables par l'utilisateur et enregistrés.
//!
//! Chaque raccourci mène à un outil du bandeau (stylo, surligneur, gomme…) ou à un stylo de la
//! trousse. On les règle depuis la fenêtre « Raccourcis des outils » (voir `shortcuts_window`).

use eframe::egui::{self, Key, Modifiers};

use crate::i18n::t;
use crate::theme;
use crate::tools::{PenPreset, Tool, ToolSettings, MAX_PENS};

/// Ce qu'un raccourci déclenche.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Tool(Tool),
    /// Le n-ième stylo de la trousse (à partir de 0).
    Pen(usize),
}

/// Les outils du bandeau, dans l'ordre d'affichage.
pub const TOOL_TARGETS: [Tool; 7] = [
    Tool::Pen,
    Tool::Highlighter,
    Tool::EraserStroke,
    Tool::Lasso,
    Tool::Text,
    Tool::Pipette,
    Tool::Instruments,
];

fn tool_id(tool: Tool) -> &'static str {
    match tool {
        Tool::Pen => "pen",
        Tool::Highlighter => "highlighter",
        Tool::EraserStroke | Tool::EraserArea => "eraser",
        Tool::Lasso => "lasso",
        Tool::Text => "text",
        Tool::Pipette => "pipette",
        Tool::Instruments => "instruments",
    }
}

impl Target {
    fn id(self) -> String {
        match self {
            Target::Tool(tool) => tool_id(tool).to_string(),
            Target::Pen(i) => format!("slot{}", i + 1),
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        if let Some(n) = id.strip_prefix("slot") {
            let n: usize = n.parse().ok()?;
            return (1..=MAX_PENS).contains(&n).then(|| Target::Pen(n - 1));
        }
        TOOL_TARGETS
            .iter()
            .find(|tool| tool_id(**tool) == id)
            .map(|tool| Target::Tool(*tool))
    }
}

/// Une touche avec ses modificateurs (Ctrl ou Cmd, Alt, Maj).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    pub key: Key,
    pub command: bool,
    pub alt: bool,
    pub shift: bool,
}

impl Chord {
    pub const fn plain(key: Key) -> Self {
        Self {
            key,
            command: false,
            alt: false,
            shift: false,
        }
    }

    pub fn from_event(key: Key, modifiers: Modifiers) -> Self {
        Self {
            key,
            command: modifiers.command,
            alt: modifiers.alt,
            shift: modifiers.shift,
        }
    }

    pub fn modifiers(self) -> Modifiers {
        let mut m = Modifiers::NONE;
        if self.command {
            m |= Modifiers::COMMAND;
        }
        if self.alt {
            m |= Modifiers::ALT;
        }
        if self.shift {
            m |= Modifiers::SHIFT;
        }
        m
    }

    /// Nombre de modificateurs : les combinaisons les plus précises sont testées d'abord.
    fn weight(self) -> u8 {
        u8::from(self.command) + u8::from(self.alt) + u8::from(self.shift)
    }

    pub fn to_setting(self) -> String {
        let mut text = String::new();
        if self.command {
            text.push_str("ctrl+");
        }
        if self.alt {
            text.push_str("alt+");
        }
        if self.shift {
            text.push_str("shift+");
        }
        text.push_str(self.key.name());
        text
    }

    pub fn from_setting(text: &str) -> Option<Self> {
        let mut chord = Chord::plain(Key::A);
        let mut parts = text.split('+').peekable();
        while let Some(part) = parts.next() {
            if parts.peek().is_some() {
                match part {
                    "ctrl" => chord.command = true,
                    "alt" => chord.alt = true,
                    "shift" => chord.shift = true,
                    _ => return None,
                }
            } else {
                chord.key = Key::from_name(part)?;
            }
        }
        Some(chord)
    }

    /// Texte affiché, par exemple « Ctrl + Maj + P ».
    pub fn label(self) -> String {
        let mut parts: Vec<&str> = Vec::new();
        if self.command {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Maj");
        }
        let key = self.key.symbol_or_name();
        let mut text = parts.join(" + ");
        if !text.is_empty() {
            text.push_str(" + ");
        }
        text.push_str(key);
        text
    }

    /// Combinaisons déjà prises par l'application (fichier, édition, plein écran…).
    pub fn is_reserved(self) -> bool {
        if matches!(
            self.key,
            Key::Escape | Key::Delete | Key::Backspace | Key::F11 | Key::Enter | Key::Tab
        ) {
            return true;
        }
        self.command
            && !self.alt
            && matches!(
                self.key,
                Key::K
                    | Key::N
                    | Key::E
                    | Key::P
                    | Key::Z
                    | Key::Y
                    | Key::A
                    | Key::C
                    | Key::V
                    | Key::X
            )
    }
}

/// Raccourcis des outils.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolShortcuts {
    bindings: Vec<(Target, Chord)>,
}

impl Default for ToolShortcuts {
    fn default() -> Self {
        Self {
            bindings: vec![
                (Target::Tool(Tool::Pen), Chord::plain(Key::P)),
                (Target::Tool(Tool::Highlighter), Chord::plain(Key::H)),
                (Target::Tool(Tool::EraserStroke), Chord::plain(Key::E)),
                (Target::Tool(Tool::Lasso), Chord::plain(Key::L)),
                (Target::Tool(Tool::Text), Chord::plain(Key::T)),
                (Target::Tool(Tool::Pipette), Chord::plain(Key::I)),
                (Target::Tool(Tool::Instruments), Chord::plain(Key::R)),
            ],
        }
    }
}

impl ToolShortcuts {
    fn norm(target: Target) -> Target {
        match target {
            Target::Tool(Tool::EraserArea) => Target::Tool(Tool::EraserStroke),
            other => other,
        }
    }

    pub fn get(&self, target: Target) -> Option<Chord> {
        let target = Self::norm(target);
        self.bindings
            .iter()
            .find(|(t, _)| *t == target)
            .map(|(_, c)| *c)
    }

    /// Affecte `chord` à `target` ; la combinaison est retirée de son ancien propriétaire.
    pub fn set(&mut self, target: Target, chord: Chord) {
        let target = Self::norm(target);
        self.bindings.retain(|(t, c)| *t != target && *c != chord);
        self.bindings.push((target, chord));
    }

    pub fn clear(&mut self, target: Target) {
        let target = Self::norm(target);
        self.bindings.retain(|(t, _)| *t != target);
    }

    /// Texte du raccourci d'un outil pour les info-bulles : « P », « Ctrl + 1 »…
    pub fn tool_label(&self, tool: Tool) -> Option<String> {
        self.get(Target::Tool(tool)).map(Chord::label)
    }

    /// Premier raccourci appuyé cette image, consommé pour qu'aucun champ ne le reçoive.
    pub fn pressed(&self, ctx: &egui::Context) -> Option<Target> {
        let mut order: Vec<&(Target, Chord)> = self.bindings.iter().collect();
        order.sort_by_key(|(_, c)| std::cmp::Reverse(c.weight()));
        for (target, chord) in order {
            if ctx.input_mut(|i| i.consume_key(chord.modifiers(), chord.key)) {
                return Some(*target);
            }
        }
        None
    }

    pub fn to_setting(&self) -> String {
        self.bindings
            .iter()
            .map(|(target, chord)| format!("{}={}", target.id(), chord.to_setting()))
            .collect::<Vec<_>>()
            .join(";")
    }

    /// Relit les raccourcis enregistrés ; les entrées illisibles sont ignorées. Un texte vide
    /// veut dire « aucun raccourci ».
    pub fn from_setting(text: &str) -> Self {
        let mut shortcuts = Self {
            bindings: Vec::new(),
        };
        for entry in text.split(';') {
            let Some((id, chord)) = entry.split_once('=') else {
                continue;
            };
            if let (Some(target), Some(chord)) = (Target::from_id(id), Chord::from_setting(chord)) {
                if !chord.is_reserved() {
                    shortcuts.set(target, chord);
                }
            }
        }
        shortcuts
    }
}

/// Première touche pressée cette image (avec ses modificateurs), pour l'enregistrement d'un raccourci.
pub fn captured_chord(ctx: &egui::Context) -> Option<Chord> {
    ctx.input(|i| {
        i.events.iter().find_map(|event| match event {
            egui::Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
                ..
            } => Some(Chord::from_event(*key, *modifiers)),
            _ => None,
        })
    })
}

fn target_label(target: Target, tools: &ToolSettings) -> String {
    match target {
        Target::Tool(tool) => crate::toolbar::tool_title(tool).to_string(),
        Target::Pen(i) => match tools.pens.get(i) {
            Some(pen) => crate::toolbar::pen_label(pen),
            None => format!("{} {}", t("tool-pen"), i + 1),
        },
    }
}

/// Fenêtre de réglage des raccourcis. `recording` est la ligne qui attend une touche. Renvoie
/// `true` quand les raccourcis ont changé (à enregistrer).
pub fn shortcuts_window(
    ctx: &egui::Context,
    open: &mut bool,
    recording: &mut Option<Target>,
    tools: &mut ToolSettings,
    message: &mut Option<&'static str>,
) -> bool {
    if !*open {
        return false;
    }
    let dark = ctx.global_style().visuals.dark_mode;
    let mut changed = false;
    let mut keep_open = true;
    let mut done = false;

    // Touche pressée pendant l'enregistrement : elle devient le raccourci (Échap annule).
    if let Some(target) = *recording {
        if let Some(chord) = captured_chord(ctx) {
            if chord.key == Key::Escape && !chord.command && !chord.alt && !chord.shift {
                *recording = None;
                *message = None;
            } else if chord.is_reserved() {
                *message = Some("shortcut-reserved");
            } else {
                tools.shortcuts.set(target, chord);
                *recording = None;
                *message = None;
                changed = true;
            }
        }
    }

    let mut rows: Vec<Target> = TOOL_TARGETS.iter().map(|tool| Target::Tool(*tool)).collect();
    rows.extend((0..tools.pens.len()).map(Target::Pen));

    egui::Window::new(t("shortcuts"))
        .open(&mut keep_open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .frame(
            egui::Frame::window(&ctx.global_style())
                .fill(theme::surface(dark))
                .inner_margin(16),
        )
        .show(ctx, |ui| {
            ui.set_width(460.0);
            ui.label(
                egui::RichText::new(t("shortcuts-hint")).color(theme::label_secondary(dark)),
            );
            if let Some(key) = *message {
                ui.colored_label(theme::danger(dark), t(key));
            }
            ui.add_space(8.0);
            egui::Grid::new("calame-raccourcis")
                .num_columns(3)
                .spacing([14.0, 8.0])
                .show(ui, |ui| {
                    for target in rows.iter().copied() {
                        if matches!(target, Target::Pen(0)) {
                            // Séparation entre les outils et les stylos de la trousse.
                            ui.separator();
                            ui.separator();
                            ui.separator();
                            ui.end_row();
                        }
                        ui.label(target_label(target, tools));
                        let waiting = *recording == Some(target);
                        let text = if waiting {
                            t("shortcut-press").to_string()
                        } else {
                            tools
                                .shortcuts
                                .get(target)
                                .map_or_else(|| t("shortcut-none").to_string(), Chord::label)
                        };
                        let button = egui::Button::new(text).selected(waiting);
                        if ui.add_sized([170.0, 28.0], button).clicked() {
                            *recording = if waiting { None } else { Some(target) };
                            *message = None;
                        }
                        let has = tools.shortcuts.get(target).is_some();
                        if ui
                            .add_enabled(has, egui::Button::new(t("shortcut-clear")))
                            .clicked()
                        {
                            tools.shortcuts.clear(target);
                            *recording = None;
                            changed = true;
                        }
                        ui.end_row();
                    }
                });
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.button(t("shortcut-reset")).clicked() {
                    tools.shortcuts = ToolShortcuts::default();
                    *recording = None;
                    *message = None;
                    changed = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(t("done")).clicked() {
                        done = true;
                    }
                });
            });
        });

    if !keep_open || done {
        *open = false;
        *recording = None;
        *message = None;
    }
    changed
}

/// Applique un raccourci déclenché : choisit l'outil ou le stylo de la trousse.
pub fn trigger_pen(tools: &mut ToolSettings, index: usize) {
    if let Some(pen) = tools.pens.get(index).copied() {
        let pen: PenPreset = pen;
        tools.apply_pen(pen);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_old_letters() {
        let s = ToolShortcuts::default();
        assert_eq!(s.get(Target::Tool(Tool::Pen)), Some(Chord::plain(Key::P)));
        assert_eq!(
            s.get(Target::Tool(Tool::EraserArea)),
            Some(Chord::plain(Key::E))
        );
    }

    #[test]
    fn set_steals_chord_and_roundtrips() {
        let mut s = ToolShortcuts::default();
        let chord = Chord {
            key: Key::Num1,
            command: false,
            alt: true,
            shift: true,
        };
        s.set(Target::Pen(2), chord);
        s.set(Target::Tool(Tool::Lasso), chord);
        assert_eq!(s.get(Target::Pen(2)), None, "la combinaison a changé de main");
        s.set(Target::Pen(0), Chord::plain(Key::P));
        assert_eq!(s.get(Target::Tool(Tool::Pen)), None);

        let text = s.to_setting();
        assert_eq!(ToolShortcuts::from_setting(&text), s);
        s.clear(Target::Tool(Tool::Text));
        assert_eq!(ToolShortcuts::from_setting(&s.to_setting()), s);
        assert!(ToolShortcuts::from_setting("").bindings.is_empty());
        assert!(ToolShortcuts::from_setting("pen=ctrl+P;foo=Q;lasso=zzz").bindings.is_empty());
    }
}
