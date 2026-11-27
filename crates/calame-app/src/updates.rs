//! Mise à jour de l'application : recherche de la dernière publication GitHub, téléchargement,
//! vérification de sa signature et lancement de l'installeur.
//!
//! Repose sur `cargo-packager-updater`, la version autonome de l'outil de mise à jour de Tauri :
//! même clé de signature, même fichier `latest.json`. Une mise à jour dont la signature ne
//! correspond pas à la clé publique intégrée au programme est refusée.

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

use cargo_packager_updater::{
    semver::Version, url::Url, Config, Update, UpdaterBuilder, WindowsConfig,
    WindowsUpdateInstallMode,
};

use crate::i18n::t;
use crate::theme::{self, Weight};
use crate::widgets::{Button, ButtonStyle};

/// Description de la dernière version, jointe à chaque publication.
const ENDPOINT: &str = "https://github.com/Teamfliyou/calame/releases/latest/download/latest.json";
const DOWNLOAD_PAGE: &str = "https://github.com/Teamfliyou/calame/releases/latest";
/// Clé publique de signature, écrite par `scripts\setup-updater.ps1`. Vide : pas de mise à jour.
const PUBLIC_KEY: &str = include_str!("../updater.pub");
const CHECK_TIMEOUT: Duration = Duration::from_secs(20);
/// Options passées à l'installeur Inno Setup : sans questions, puis relance de Calame.
const INSTALLER_ARGS: [&str; 5] = [
    "/SILENT",
    "/SP-",
    "/SUPPRESSMSGBOXES",
    "/NORESTART",
    "/RELAUNCH",
];

type CheckResult = Result<Option<Box<Update>>, String>;
type DownloadResult = Result<(Box<Update>, Vec<u8>), String>;

#[derive(Default)]
enum State {
    #[default]
    Idle,
    Checking {
        /// Demandée par l'utilisateur : on annonce aussi « à jour » et les échecs.
        asked: bool,
        result: Receiver<CheckResult>,
    },
    Available(Box<Update>),
    Downloading(Receiver<DownloadResult>),
}

/// Ce que l'application doit faire à la suite d'une recherche ou d'un téléchargement.
pub enum Event {
    Status(String),
    Error(String),
    /// Installeur téléchargé et vérifié : enregistrer le travail, puis appeler [`install`].
    Install(Box<Update>, Vec<u8>),
}

#[derive(Default)]
pub struct Updates {
    state: State,
}

/// Calame a-t-il été posé par l'installeur ? La version portable ne se remplace pas toute seule.
fn installed() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("unins000.exe").exists()))
        .unwrap_or(false)
}

fn config() -> Result<Config, String> {
    let endpoint = Url::parse(ENDPOINT).map_err(|e| e.to_string())?;
    Ok(Config {
        endpoints: vec![endpoint],
        pubkey: PUBLIC_KEY.trim().to_owned(),
        windows: Some(WindowsConfig {
            installer_args: Some(INSTALLER_ARGS.map(String::from).to_vec()),
            // Les options de ce mode sont propres à NSIS : on donne les nôtres.
            install_mode: Some(WindowsUpdateInstallMode::BasicUi),
        }),
    })
}

fn look_for_update() -> CheckResult {
    let version = Version::parse(env!("CARGO_PKG_VERSION")).map_err(|e| e.to_string())?;
    let updater = UpdaterBuilder::new(version, config()?)
        .timeout(CHECK_TIMEOUT)
        .build()
        .map_err(|e| e.to_string())?;
    let found = updater.check().map_err(|e| e.to_string())?;
    Ok(found.map(|mut update| {
        // Le délai vaut pour la recherche, pas pour un téléchargement sur une connexion lente.
        update.timeout = None;
        Box::new(update)
    }))
}

/// Lance l'installeur ; sous Windows, le programme se ferme aussitôt.
pub fn install(update: &Update, bytes: Vec<u8>) -> Result<(), String> {
    update
        .install(bytes)
        .map_err(|e| format!("{} : {e}", t("update-failed")))
}

impl Updates {
    /// Cherche une mise à jour dans un fil séparé. Au démarrage (`asked` faux), tout se passe en
    /// silence tant qu'il n'y a rien à installer.
    pub fn check(&mut self, ctx: &egui::Context, asked: bool) -> Option<Event> {
        if !matches!(self.state, State::Idle | State::Available(_)) {
            return None;
        }
        let refusal = if !cfg!(windows) {
            Some("update-unsupported")
        } else if PUBLIC_KEY.trim().is_empty() {
            Some("update-not-configured")
        } else {
            None
        };
        if let Some(key) = refusal {
            return asked.then(|| Event::Status(t(key).into()));
        }
        let (sender, result) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = sender.send(look_for_update());
            ctx.request_repaint();
        });
        self.state = State::Checking { asked, result };
        asked.then(|| Event::Status(t("update-checking").into()))
    }

    /// Relève le résultat des fils et affiche la proposition de mise à jour.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<Event> {
        match std::mem::take(&mut self.state) {
            State::Idle => None,
            State::Checking { asked, result } => match result.try_recv() {
                Err(TryRecvError::Empty) => {
                    self.state = State::Checking { asked, result };
                    None
                }
                Ok(Ok(Some(update))) => {
                    self.state = State::Available(update);
                    ctx.request_repaint();
                    // Retire « Recherche en cours » : la proposition le remplace.
                    asked.then(|| Event::Status(String::new()))
                }
                Ok(Ok(None)) => asked.then(|| {
                    Event::Status(format!(
                        "{} ({})",
                        t("update-up-to-date"),
                        env!("CARGO_PKG_VERSION")
                    ))
                }),
                Ok(Err(e)) => asked.then(|| Event::Error(format!("{} : {e}", t("update-failed")))),
                Err(TryRecvError::Disconnected) => None,
            },
            State::Available(update) => self.offer(ctx, update),
            State::Downloading(result) => match result.try_recv() {
                Err(TryRecvError::Empty) => {
                    window(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(t("update-downloading"));
                        });
                    });
                    self.state = State::Downloading(result);
                    None
                }
                Ok(Ok((update, bytes))) => Some(Event::Install(update, bytes)),
                Ok(Err(e)) => Some(Event::Error(format!("{} : {e}", t("update-failed")))),
                Err(TryRecvError::Disconnected) => None,
            },
        }
    }

    fn offer(&mut self, ctx: &egui::Context, update: Box<Update>) -> Option<Event> {
        let automatic = installed();
        let (mut accept, mut later) = (false, false);
        window(ctx, |ui| {
            ui.label(format!("{} {}", t("update-available"), update.version));
            if !automatic {
                ui.label(t("update-portable-hint"));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let key = if automatic {
                    "update-install"
                } else {
                    "update-download"
                };
                accept = Button::new(t(key), ButtonStyle::Primary)
                    .height(32.0)
                    .show(ui)
                    .clicked();
                later = Button::new(t("update-later"), ButtonStyle::Ghost)
                    .height(32.0)
                    .show(ui)
                    .clicked();
            });
        });
        if later {
            return None;
        }
        if !accept {
            self.state = State::Available(update);
            return None;
        }
        if !automatic {
            ctx.open_url(egui::OpenUrl::new_tab(DOWNLOAD_PAGE));
            return None;
        }
        let (sender, result) = mpsc::channel();
        let repaint = ctx.clone();
        std::thread::spawn(move || {
            let downloaded = update
                .download()
                .map(|bytes| (update, bytes))
                .map_err(|e| e.to_string());
            let _ = sender.send(downloaded);
            repaint.request_repaint();
        });
        self.state = State::Downloading(result);
        ctx.request_repaint();
        None
    }
}

/// Petite fenêtre en haut à droite.
fn window(ctx: &egui::Context, content: impl FnOnce(&mut egui::Ui)) {
    egui::Window::new(
        egui::RichText::new(t("update-title")).font(theme::font(15.0, Weight::Semibold)),
    )
    .id(egui::Id::unique("calame-update"))
    .collapsible(false)
    .resizable(false)
    .anchor(egui::Align2::RIGHT_TOP, [-16.0, 68.0])
    .show(ctx, content);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn the_endpoint_is_valid_and_the_installer_relaunches_the_app() {
        let config = config().expect("adresse valide");
        let windows = config.windows.expect("réglages Windows");
        assert!(windows
            .installer_args
            .is_some_and(|a| a.iter().any(|x| x == "/RELAUNCH")));
    }

    /// Le fichier qu'écrit `scripts\sign-update.ps1` est bien relu par l'outil de mise à jour.
    #[test]
    fn the_published_manifest_is_understood() {
        let manifest = r#"{
            "version": "9.8.7",
            "pub_date": "2026-11-27T10:00:00Z",
            "platforms": {
                "windows-x86_64": {
                    "signature": "c2lnbmF0dXJl",
                    "url": "https://example.test/calame-9.8.7-setup.exe",
                    "format": "nsis"
                }
            }
        }"#;
        let release: cargo_packager_updater::RemoteRelease =
            serde_json::from_str(manifest).expect("manifeste lisible");
        assert_eq!(release.version.to_string(), "9.8.7");
        assert!(release.download_url("windows-x86_64").is_ok());
    }
}
