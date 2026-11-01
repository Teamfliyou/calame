//! Sauvegarde et restauration complètes de la bibliothèque dans un fichier zip.
//!
//! Le zip contient une copie cohérente de la base SQLite (`calame.db`) et un court texte
//! d'explication. La restauration lit uniquement l'entrée `calame.db` par son nom : aucun autre
//! nom de fichier du zip n'est utilisé pour écrire sur le disque.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::{Result, Store, StoreError, SCHEMA_VERSION};

/// Nom de la base dans le zip.
pub const DB_ENTRY: &str = "calame.db";
const INFO_ENTRY: &str = "LISEZMOI.txt";
/// Taille maximale acceptée pour la base extraite (protège d'un zip piégé) : 8 Gio.
const MAX_DB_BYTES: u64 = 8 * 1024 * 1024 * 1024;

impl Store {
    /// Écrit une sauvegarde complète dans `zip_path`, sans interrompre l'utilisation de la base.
    pub fn backup_to(&self, zip_path: &Path) -> Result<()> {
        // `VACUUM INTO` produit une copie cohérente, journal compris, même pendant les écritures.
        let snapshot = sibling(zip_path, "instantane");
        remove_if_exists(&snapshot)?;
        let result = self.write_backup(&snapshot, zip_path);
        let _ = fs::remove_file(&snapshot);
        if result.is_err() {
            let _ = fs::remove_file(zip_path);
        }
        result
    }

    fn write_backup(&self, snapshot: &Path, zip_path: &Path) -> Result<()> {
        self.conn
            .execute("VACUUM INTO ?1", [snapshot.to_string_lossy().as_ref()])?;
        let mut zip = ZipWriter::new(File::create(zip_path)?);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip.start_file(DB_ENTRY, options)?;
        io::copy(&mut File::open(snapshot)?, &mut zip)?;
        zip.start_file(INFO_ENTRY, options)?;
        zip.write_all(
            format!(
                "Sauvegarde de la bibliothèque Calame.\n\
                 Version du schéma : {SCHEMA_VERSION}\n\
                 Créée le : {} (secondes Unix)\n\n\
                 Pour restaurer : Fichier > Restaurer une sauvegarde dans Calame.\n",
                now_seconds()
            )
            .as_bytes(),
        )?;
        zip.finish()?;
        Ok(())
    }
}

/// Remplace la base `db_path` par celle de la sauvegarde `zip_path`.
///
/// La base ne doit pas être ouverte pendant l'appel. L'ancienne base, si elle existe, est conservée
/// à côté sous un nom daté ; son chemin est renvoyé. Si la sauvegarde est invalide, rien n'est modifié.
pub fn restore_backup(zip_path: &Path, db_path: &Path) -> Result<Option<PathBuf>> {
    let mut archive = ZipArchive::new(File::open(zip_path)?)?;
    let entry = archive
        .by_name(DB_ENTRY)
        .map_err(|_| StoreError::NotABackup)?;
    if entry.size() > MAX_DB_BYTES {
        return Err(StoreError::NotABackup);
    }

    // On extrait d'abord à côté de la vraie base, et on vérifie avant de toucher à quoi que ce soit.
    let staging = sibling(db_path, "restauration");
    remove_if_exists(&staging)?;
    let extracted = extract_limited(entry, &staging);
    let checked = extracted.and_then(|()| check_database(&staging));
    if let Err(e) = checked {
        remove_database_files(&staging);
        return Err(e);
    }

    let previous = if db_path.exists() {
        let kept = sibling(db_path, &format!("avant-restauration-{}", now_seconds()));
        fs::rename(db_path, &kept)?;
        Some(kept)
    } else {
        None
    };
    // Des fichiers de journal de l'ancienne base ne doivent pas s'appliquer à la nouvelle.
    let _ = fs::remove_file(sibling_suffix(db_path, "-wal"));
    let _ = fs::remove_file(sibling_suffix(db_path, "-shm"));
    fs::rename(&staging, db_path)?;
    Ok(previous)
}

fn extract_limited(entry: impl Read, target: &Path) -> Result<()> {
    let mut out = File::create(target)?;
    let copied = io::copy(&mut entry.take(MAX_DB_BYTES + 1), &mut out)?;
    if copied > MAX_DB_BYTES {
        return Err(StoreError::NotABackup);
    }
    out.flush()?;
    Ok(())
}

/// Vérifie que le fichier est une base Calame saine, sans la modifier : une base d'une version plus
/// récente ou abîmée est refusée.
fn check_database(path: &Path) -> Result<()> {
    {
        let conn = Connection::open(path).map_err(|_| StoreError::NotABackup)?;
        let version: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(|_| StoreError::NotABackup)?;
        if version > SCHEMA_VERSION {
            return Err(StoreError::UnsupportedVersion(version));
        }
        let status: String = conn
            .query_row("PRAGMA quick_check", [], |r| r.get(0))
            .map_err(|_| StoreError::NotABackup)?;
        if status != "ok" {
            return Err(StoreError::NotABackup);
        }
        let has_nodes: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'nodes'",
                [],
                |r| r.get(0),
            )
            .map_err(|_| StoreError::NotABackup)?;
        if has_nodes == 0 {
            return Err(StoreError::NotABackup);
        }
    }
    // Ouvrir la base normalement la met au schéma courant si la sauvegarde est plus ancienne.
    Store::open(path).map(drop)
}

fn sibling(path: &Path, tag: &str) -> PathBuf {
    let name = path
        .file_name()
        .map_or_else(|| "calame".into(), |n| n.to_string_lossy().into_owned());
    path.with_file_name(format!("{name}.{tag}"))
}

fn sibling_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

fn remove_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn remove_database_files(path: &Path) {
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(sibling_suffix(path, "-wal"));
    let _ = fs::remove_file(sibling_suffix(path, "-shm"));
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use calame_core::{
        Background, Brush, Item, NodeId, NodeKind, Page, Point2, Stroke, StrokePoint,
    };

    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("calame-backup-{tag}-{}", NodeId::new()));
        fs::create_dir_all(&dir).expect("dossier");
        dir
    }

    /// Une bibliothèque avec une page dessinée, un fond image et un nom accentué.
    fn populated(path: &Path) -> (Store, NodeId) {
        let s = Store::open(path).expect("base");
        let subject = s
            .create_node(None, NodeKind::Subject, "Géométrie")
            .expect("matière");
        let book = s
            .create_node(Some(subject.id), NodeKind::Notebook, "Cahier")
            .expect("cahier");
        let page = s
            .create_node(Some(book.id), NodeKind::Page, "Page 1")
            .expect("page");
        let mut stroke = Stroke::new(Brush::default());
        stroke.push(StrokePoint {
            pos: Point2::new(1.0, 2.0),
            pressure: 0.7,
            tilt: None,
        });
        s.save_page(
            page.id,
            &Page {
                items: vec![Item::Stroke(stroke)],
            },
        )
        .expect("page");
        let asset = s
            .add_asset("image/png", 100.0, 200.0, &[7, 7, 7])
            .expect("image");
        s.set_background(page.id, Background::Image(asset))
            .expect("fond");
        s.set_setting("favorites", "#ff0000").expect("réglage");
        (s, page.id)
    }

    #[test]
    fn a_backup_restores_pages_backgrounds_images_and_settings() {
        let dir = temp_dir("roundtrip");
        let (source, page_id) = populated(&dir.join("source.db"));
        let zip = dir.join("sauvegarde.zip");
        source.backup_to(&zip).expect("sauvegarde");

        let names: Vec<String> = {
            let archive = ZipArchive::new(File::open(&zip).expect("zip")).expect("archive");
            archive.file_names().map(str::to_owned).collect()
        };
        assert!(names.contains(&DB_ENTRY.to_owned()) && names.contains(&INFO_ENTRY.to_owned()));
        // Aucun fichier temporaire ne traîne à côté.
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .expect("lecture")
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains("instantane"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");

        let target = dir.join("cible.db");
        assert_eq!(restore_backup(&zip, &target).expect("restauration"), None);
        let restored = Store::open(&target).expect("ouverture");
        assert_eq!(restored.all_nodes().expect("arbre").len(), 3);
        assert_eq!(
            restored.load_page(page_id).expect("page"),
            source.load_page(page_id).expect("page")
        );
        let Background::Image(asset) = restored.background(page_id).expect("fond") else {
            panic!("fond image attendu");
        };
        assert_eq!(restored.asset(asset).expect("image").data, vec![7, 7, 7]);
        assert_eq!(
            restored.setting("favorites").expect("réglage").as_deref(),
            Some("#ff0000")
        );
        drop((source, restored));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn restoring_keeps_the_previous_library_next_to_the_new_one() {
        let dir = temp_dir("keep");
        let (source, _) = populated(&dir.join("source.db"));
        let zip = dir.join("s.zip");
        source.backup_to(&zip).expect("sauvegarde");

        let target = dir.join("calame.db");
        {
            let old = Store::open(&target).expect("ancienne");
            old.create_node(None, NodeKind::Subject, "Ancienne matière")
                .expect("matière");
        }
        let kept = restore_backup(&zip, &target)
            .expect("restauration")
            .expect("l'ancienne base est conservée");
        assert!(kept.exists());
        let old = Store::open(&kept).expect("ancienne conservée");
        assert_eq!(old.all_nodes().expect("arbre")[0].name, "Ancienne matière");
        let new = Store::open(&target).expect("nouvelle");
        assert_eq!(new.all_nodes().expect("arbre").len(), 3);
        drop((source, old, new));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn invalid_backups_are_refused_and_change_nothing() {
        let dir = temp_dir("invalid");
        let target = dir.join("calame.db");
        {
            let s = Store::open(&target).expect("base");
            s.create_node(None, NodeKind::Subject, "À garder")
                .expect("matière");
        }
        let before = fs::read(&target).expect("octets");

        // Pas un zip.
        let garbage = dir.join("garbage.zip");
        fs::write(&garbage, b"ceci n'est pas un zip").expect("fichier");
        assert!(restore_backup(&garbage, &target).is_err());

        // Un zip sans base.
        let empty = dir.join("vide.zip");
        {
            let mut zip = ZipWriter::new(File::create(&empty).expect("zip"));
            zip.start_file("autre.txt", SimpleFileOptions::default())
                .expect("entrée");
            zip.write_all(b"bonjour").expect("écriture");
            zip.finish().expect("fin");
        }
        assert!(matches!(
            restore_backup(&empty, &target),
            Err(StoreError::NotABackup)
        ));

        // Un zip dont « calame.db » n'est pas une base SQLite.
        let fake = dir.join("fausse.zip");
        {
            let mut zip = ZipWriter::new(File::create(&fake).expect("zip"));
            zip.start_file(DB_ENTRY, SimpleFileOptions::default())
                .expect("entrée");
            zip.write_all(&[0xAB; 4096]).expect("écriture");
            zip.finish().expect("fin");
        }
        assert!(restore_backup(&fake, &target).is_err());

        assert_eq!(
            fs::read(&target).expect("octets"),
            before,
            "la base actuelle est intacte"
        );
        let names: Vec<String> = fs::read_dir(&dir)
            .expect("lecture")
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            names.iter().all(|n| !n.contains("restauration")),
            "{names:?}"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_backup_from_a_newer_version_is_refused() {
        let dir = temp_dir("future");
        let db = dir.join("future.db");
        {
            let conn = Connection::open(&db).expect("base");
            conn.execute_batch(&format!(
                "CREATE TABLE nodes (id TEXT); PRAGMA user_version = {};",
                SCHEMA_VERSION + 1
            ))
            .expect("schéma");
        }
        let zip = dir.join("futur.zip");
        {
            let mut z = ZipWriter::new(File::create(&zip).expect("zip"));
            z.start_file(DB_ENTRY, SimpleFileOptions::default())
                .expect("entrée");
            z.write_all(&fs::read(&db).expect("octets"))
                .expect("écriture");
            z.finish().expect("fin");
        }
        let target = dir.join("calame.db");
        assert!(matches!(
            restore_backup(&zip, &target),
            Err(StoreError::UnsupportedVersion(_))
        ));
        assert!(!target.exists());
        let _ = fs::remove_dir_all(dir);
    }
}
