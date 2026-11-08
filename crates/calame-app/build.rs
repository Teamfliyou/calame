//! Sous Windows, intègre `assets/icon.ico` à `calame.exe` (icône de l'Explorateur, de la barre des
//! tâches, du menu Démarrer). Sans ce fichier, la compilation continue : l'exécutable garde l'icône
//! par défaut de Windows.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=../../assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");

    let icon = Path::new("../../assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") || !icon.exists() {
        return;
    }
    let result = if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") {
        embed_with_windres(icon)
    } else {
        embed_with_winresource(icon)
    };
    if let Err(error) = result {
        println!("cargo:warning=icône de l'exécutable non intégrée : {error}");
    }
}

/// Chaîne MSVC : `rc.exe`, par `winresource`.
fn embed_with_winresource(icon: &Path) -> Result<(), String> {
    let mut resources = winresource::WindowsResource::new();
    resources.set_icon(&icon.to_string_lossy());
    resources.set("ProductName", "Calame");
    resources.set("FileDescription", "Calame");
    resources.compile().map_err(|error| error.to_string())
}

/// Chaîne MinGW : `windres` appelé directement. `winresource` lui passe `-I <dossier du projet>`, ce
/// qui échoue quand ce chemin contient une espace (« prise de note »).
fn embed_with_windres(icon: &Path) -> Result<(), String> {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").map_err(|error| error.to_string())?);
    let icon = icon.canonicalize().map_err(|error| error.to_string())?;
    let icon = icon.to_string_lossy();
    let icon = icon.strip_prefix(r"\\?\").unwrap_or(&icon).replace('\\', "/");

    let script = out_dir.join("calame-icon.rc");
    std::fs::write(&script, format!("1 ICON \"{icon}\"\n")).map_err(|error| error.to_string())?;
    let object = out_dir.join("calame-icon.o");

    let windres = std::env::var("WINDRES").unwrap_or_else(|_| "windres".to_owned());
    let status = Command::new(windres)
        .args(["--target", "pe-x86-64"])
        .arg(&script)
        .arg(&object)
        .status()
        .map_err(|error| format!("windres introuvable : {error}"))?;
    if !status.success() {
        return Err("windres a échoué".to_owned());
    }
    println!("cargo:rustc-link-arg-bins={}", object.display());
    Ok(())
}
