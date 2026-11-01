//! Stockage SQLite de la bibliothèque : hiérarchie, métadonnées, corbeille et contenu des pages.
//!
//! Le schéma est documenté dans `docs/format.md`.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use calame_core::library::check_placement;
use calame_core::{
    AssetId, Background, Color, Item, LibraryError, Node, NodeId, NodeKind, Page, Stroke,
};
use rusqlite::types::Type;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const SCHEMA_VERSION: i32 = 3;
/// Version du codec des pages (`page_data.format`).
///
/// 1 : traits seuls ; 2 : éléments (traits et textes).
const PAGE_FORMAT: i64 = 2;

/// Page telle qu'enregistrée en format 1, lue puis convertie.
#[derive(Serialize, Deserialize)]
struct PageV1 {
    strokes: Vec<Stroke>,
}

/// Couleurs proposées par défaut aux nouvelles matières, à tour de rôle.
const SUBJECT_PALETTE: [Color; 6] = [
    Color::rgb(0x23, 0x40, 0xC4),
    Color::rgb(0xC2, 0x4E, 0x3A),
    Color::rgb(0x2F, 0x7A, 0x5C),
    Color::rgb(0xE9, 0xA9, 0x3A),
    Color::rgb(0x7A, 0x4F, 0xC4),
    Color::rgb(0x16, 0x76, 0x76),
];

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("erreur SQLite : {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("erreur d'accès au disque : {0}")]
    Io(#[from] std::io::Error),
    #[error("page illisible : {0}")]
    Codec(#[from] postcard::Error),
    #[error(transparent)]
    Library(#[from] LibraryError),
    #[error("élément introuvable : {0}")]
    NotFound(NodeId),
    #[error("le nom ne peut pas être vide")]
    EmptyName,
    #[error("cet élément n'est pas une page")]
    NotAPage,
    #[error("cet élément n'est pas dans la corbeille")]
    NotTrashed,
    #[error("format de page inconnu : {0}")]
    UnsupportedPageFormat(i64),
    #[error("image de fond introuvable : {0}")]
    AssetNotFound(AssetId),
    #[error("archive zip : {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("ce fichier n'est pas une sauvegarde Calame valide")]
    NotABackup,
    #[error("base créée par une version plus récente de Calame (schéma {0})")]
    UnsupportedVersion(i32),
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// Image enregistrée (fond de page) ; `width` et `height` sont en unités de page.
#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    pub id: AssetId,
    pub mime: String,
    pub width: f32,
    pub height: f32,
    pub data: Vec<u8>,
}

/// Une page à importer : son image de fond et sa taille en unités de page.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedPage {
    pub mime: String,
    pub width: f32,
    pub height: f32,
    pub data: Vec<u8>,
}

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        // WAL : une page écrite n'est jamais à moitié sauvegardée après un plantage.
        conn.execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON; PRAGMA synchronous = NORMAL;",
        )?;
        let version: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(StoreError::UnsupportedVersion(version));
        }
        if version < 1 {
            conn.execute_batch(
                "CREATE TABLE nodes (
                    id TEXT PRIMARY KEY,
                    parent_id TEXT REFERENCES nodes(id) ON DELETE CASCADE,
                    kind TEXT NOT NULL CHECK (kind IN ('subject','notebook','folder','page')),
                    name TEXT NOT NULL,
                    color INTEGER NOT NULL,
                    icon TEXT NOT NULL,
                    position INTEGER NOT NULL,
                    created_at INTEGER NOT NULL,
                    trashed_at INTEGER
                );
                CREATE INDEX idx_nodes_parent ON nodes(parent_id, position);
                CREATE TABLE page_data (
                    page_id TEXT PRIMARY KEY REFERENCES nodes(id) ON DELETE CASCADE,
                    format INTEGER NOT NULL,
                    data BLOB NOT NULL,
                    updated_at INTEGER NOT NULL
                );
                PRAGMA user_version = 1;",
            )?;
        }
        if version < 2 {
            conn.execute_batch(
                "CREATE TABLE settings (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );
                PRAGMA user_version = 2;",
            )?;
        }
        if version < 3 {
            // Fond de chaque page, et images (ou pages de PDF rendues) qui peuvent le servir.
            conn.execute_batch(
                "ALTER TABLE nodes ADD COLUMN background TEXT NOT NULL DEFAULT 'blank';
                CREATE TABLE assets (
                    id TEXT PRIMARY KEY,
                    mime TEXT NOT NULL,
                    width REAL NOT NULL,
                    height REAL NOT NULL,
                    data BLOB NOT NULL,
                    created_at INTEGER NOT NULL
                );
                PRAGMA user_version = 3;",
            )?;
        }
        Ok(Self { conn })
    }

    // ---- Lecture -------------------------------------------------------------------------

    pub fn node(&self, id: NodeId) -> Result<Node> {
        self.conn
            .query_row(
                &format!("SELECT {NODE_COLUMNS} FROM nodes WHERE id = ?1"),
                [id.to_string()],
                node_from_row,
            )
            .optional()?
            .ok_or(StoreError::NotFound(id))
    }

    /// Tous les éléments hors corbeille.
    pub fn all_nodes(&self) -> Result<Vec<Node>> {
        self.query_nodes(
            &format!("SELECT {NODE_COLUMNS} FROM nodes WHERE trashed_at IS NULL"),
            [],
        )
    }

    /// Date de dernière modification (secondes Unix) de chaque page enregistrée hors corbeille.
    /// Une page jamais écrite n'y figure pas.
    pub fn page_timestamps(&self) -> Result<HashMap<NodeId, i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT d.page_id, d.updated_at FROM page_data d
             JOIN nodes n ON n.id = d.page_id WHERE n.trashed_at IS NULL",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        let mut out = HashMap::new();
        for row in rows {
            let (id, at) = row?;
            if let Ok(id) = id.parse() {
                out.insert(id, at);
            }
        }
        Ok(out)
    }

    /// Éléments supprimés de plus haut niveau (leurs descendants les suivent), du plus récent au plus ancien.
    pub fn trashed(&self) -> Result<Vec<Node>> {
        self.query_nodes(
            &format!(
                "SELECT {NODE_COLUMNS} FROM nodes
                 WHERE trashed_at IS NOT NULL
                   AND (parent_id IS NULL
                        OR parent_id NOT IN (SELECT id FROM nodes WHERE trashed_at IS NOT NULL))
                 ORDER BY trashed_at DESC, name"
            ),
            [],
        )
    }

    /// Recherche par nom (insensible à la casse), hors corbeille.
    pub fn search(&self, query: &str) -> Result<Vec<Node>> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        Ok(self
            .all_nodes()?
            .into_iter()
            .filter(|n| n.name.to_lowercase().contains(&query))
            .collect())
    }

    fn query_nodes<P: rusqlite::Params>(&self, sql: &str, params: P) -> Result<Vec<Node>> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params, node_from_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    // ---- Création et modification --------------------------------------------------------

    pub fn create_node(&self, parent: Option<NodeId>, kind: NodeKind, name: &str) -> Result<Node> {
        let name = clean_name(name)?;
        let parent_kind = parent.map(|p| self.node(p)).transpose()?.map(|p| p.kind);
        check_placement(parent_kind, kind)?;

        let (next_position, siblings): (i64, i64) = self.conn.query_row(
            "SELECT COALESCE(MAX(position) + 1, 0), COUNT(*) FROM nodes WHERE parent_id IS ?1",
            [parent.map(|p| p.to_string())],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let color = if kind == NodeKind::Subject {
            SUBJECT_PALETTE[siblings as usize % SUBJECT_PALETTE.len()]
        } else {
            Color::rgb(128, 128, 128)
        };
        let node = Node {
            id: NodeId::new(),
            parent_id: parent,
            kind,
            name,
            color,
            icon: kind.default_icon().to_owned(),
            position: next_position,
        };
        self.conn.execute(
            "INSERT INTO nodes (id, parent_id, kind, name, color, icon, position, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                node.id.to_string(),
                node.parent_id.map(|p| p.to_string()),
                kind.as_str(),
                node.name,
                pack_color(node.color),
                node.icon,
                node.position,
                now(),
            ],
        )?;
        Ok(node)
    }

    pub fn rename(&self, id: NodeId, name: &str) -> Result<()> {
        let name = clean_name(name)?;
        self.expect_changed(
            id,
            self.conn.execute(
                "UPDATE nodes SET name = ?2 WHERE id = ?1",
                params![id.to_string(), name],
            )?,
        )
    }

    pub fn set_style(&self, id: NodeId, color: Color, icon: &str) -> Result<()> {
        self.expect_changed(
            id,
            self.conn.execute(
                "UPDATE nodes SET color = ?2, icon = ?3 WHERE id = ?1",
                params![id.to_string(), pack_color(color), icon.trim()],
            )?,
        )
    }

    /// Déplace `id` sous `new_parent` (`None` = racine) à la position `index` (`None` = à la fin).
    /// Sert aussi à réordonner parmi ses frères.
    pub fn move_node(
        &mut self,
        id: NodeId,
        new_parent: Option<NodeId>,
        index: Option<usize>,
    ) -> Result<()> {
        let node = self.node(id)?;
        let parent_kind = new_parent
            .map(|p| self.node(p))
            .transpose()?
            .map(|p| p.kind);
        check_placement(parent_kind, node.kind)?;
        if let Some(p) = new_parent {
            if p == id || self.is_in_subtree(id, p)? {
                return Err(LibraryError::Cycle.into());
            }
        }
        let mut siblings = self.sibling_ids(new_parent)?;
        siblings.retain(|s| *s != id);
        let at = index.unwrap_or(siblings.len()).min(siblings.len());
        siblings.insert(at, id);

        let tx = self.conn.transaction()?;
        tx.execute(
            "UPDATE nodes SET parent_id = ?2 WHERE id = ?1",
            params![id.to_string(), new_parent.map(|p| p.to_string())],
        )?;
        write_positions(&tx, &siblings)?;
        tx.commit()?;
        Ok(())
    }

    /// Décale `id` de `delta` rangs parmi ses frères (négatif = vers le haut).
    pub fn move_by(&mut self, id: NodeId, delta: i32) -> Result<()> {
        let node = self.node(id)?;
        let siblings = self.sibling_ids(node.parent_id)?;
        let Some(current) = siblings.iter().position(|s| *s == id) else {
            return Ok(());
        };
        let target = (current as i64 + i64::from(delta)).clamp(0, siblings.len() as i64 - 1);
        self.move_node(id, node.parent_id, Some(target as usize))
    }

    /// Copie récursive (avec le contenu des pages), placée juste après l'original.
    pub fn duplicate(&mut self, id: NodeId) -> Result<Node> {
        let original = self.node(id)?;
        let tx = self.conn.transaction()?;
        let copy_id = copy_subtree(&tx, id, original.parent_id, Some(" (copie)"))?;
        let mut siblings: Vec<NodeId> = sibling_ids_in(&tx, original.parent_id)?;
        siblings.retain(|s| *s != copy_id);
        let after = siblings
            .iter()
            .position(|s| *s == id)
            .map_or(siblings.len(), |i| i + 1);
        siblings.insert(after, copy_id);
        write_positions(&tx, &siblings)?;
        tx.commit()?;
        self.node(copy_id)
    }

    // ---- Corbeille -----------------------------------------------------------------------

    pub fn trash(&self, id: NodeId) -> Result<()> {
        self.node(id)?;
        self.conn.execute(
            &format!(
                "{SUBTREE_CTE} UPDATE nodes SET trashed_at = ?2 WHERE id IN (SELECT id FROM sub)"
            ),
            params![id.to_string(), now()],
        )?;
        Ok(())
    }

    pub fn restore(&self, id: NodeId) -> Result<()> {
        self.node(id)?;
        self.conn.execute(
            &format!(
                "{SUBTREE_CTE} UPDATE nodes SET trashed_at = NULL WHERE id IN (SELECT id FROM sub)"
            ),
            [id.to_string()],
        )?;
        Ok(())
    }

    /// Supprime définitivement un élément de la corbeille, avec ses descendants.
    pub fn delete_permanently(&self, id: NodeId) -> Result<()> {
        let trashed: Option<i64> = self
            .conn
            .query_row(
                "SELECT trashed_at FROM nodes WHERE id = ?1",
                [id.to_string()],
                |r| r.get(0),
            )
            .optional()?
            .ok_or(StoreError::NotFound(id))?;
        if trashed.is_none() {
            return Err(StoreError::NotTrashed);
        }
        self.conn
            .execute("DELETE FROM nodes WHERE id = ?1", [id.to_string()])?;
        self.purge_unused_assets()
    }

    pub fn empty_trash(&self) -> Result<()> {
        for node in self.trashed()? {
            self.delete_permanently(node.id)?;
        }
        Ok(())
    }

    // ---- Contenu des pages ---------------------------------------------------------------

    /// Charge une page ; une page jamais enregistrée est vide.
    pub fn load_page(&self, id: NodeId) -> Result<Page> {
        let row: Option<(i64, Vec<u8>)> = self
            .conn
            .query_row(
                "SELECT format, data FROM page_data WHERE page_id = ?1",
                [id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        match row {
            Some((format, bytes)) => decode_page(format, &bytes),
            None => {
                self.node(id)?;
                Ok(Page::default())
            }
        }
    }

    pub fn save_page(&self, id: NodeId, page: &Page) -> Result<()> {
        if self.node(id)?.kind != NodeKind::Page {
            return Err(StoreError::NotAPage);
        }
        let bytes = postcard::to_allocvec(page)?;
        self.conn.execute(
            "INSERT INTO page_data (page_id, format, data, updated_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(page_id) DO UPDATE SET
                format = excluded.format, data = excluded.data, updated_at = excluded.updated_at",
            params![id.to_string(), PAGE_FORMAT, bytes, now()],
        )?;
        Ok(())
    }

    // ---- Fonds de page et images ---------------------------------------------------------

    pub fn background(&self, page: NodeId) -> Result<Background> {
        let text: Option<String> = self
            .conn
            .query_row(
                "SELECT background FROM nodes WHERE id = ?1",
                [page.to_string()],
                |r| r.get(0),
            )
            .optional()?;
        let text = text.ok_or(StoreError::NotFound(page))?;
        // Un fond illisible (base d'une version future, par exemple) retombe sur une page unie.
        Ok(Background::from_db(&text).unwrap_or_default())
    }

    pub fn set_background(&self, page: NodeId, background: Background) -> Result<()> {
        if self.node(page)?.kind != NodeKind::Page {
            return Err(StoreError::NotAPage);
        }
        if let Some(id) = background.image() {
            self.asset(id)?;
        }
        self.conn.execute(
            "UPDATE nodes SET background = ?2 WHERE id = ?1",
            params![page.to_string(), background.to_db()],
        )?;
        self.purge_unused_assets()?;
        Ok(())
    }

    /// Enregistre une image (PNG ou JPEG) ; `width` et `height` sont sa taille en unités de page.
    pub fn add_asset(&self, mime: &str, width: f32, height: f32, data: &[u8]) -> Result<AssetId> {
        let id = AssetId::new();
        self.conn.execute(
            "INSERT INTO assets (id, mime, width, height, data, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id.to_string(), mime, width, height, data, now()],
        )?;
        Ok(id)
    }

    pub fn asset(&self, id: AssetId) -> Result<Asset> {
        self.conn
            .query_row(
                "SELECT mime, width, height, data FROM assets WHERE id = ?1",
                [id.to_string()],
                |r| {
                    Ok(Asset {
                        id,
                        mime: r.get(0)?,
                        width: r.get(1)?,
                        height: r.get(2)?,
                        data: r.get(3)?,
                    })
                },
            )
            .optional()?
            .ok_or(StoreError::AssetNotFound(id))
    }

    /// Supprime les images qu'aucune page n'utilise plus (y compris dans la corbeille).
    fn purge_unused_assets(&self) -> Result<()> {
        self.conn.execute(
            "DELETE FROM assets WHERE 'asset:' || id NOT IN (SELECT background FROM nodes)",
            [],
        )?;
        Ok(())
    }

    /// Importe un document : un cahier sous une matière, ou un sous-dossier ailleurs, avec une page
    /// par élément de `pages`, chacune ayant son image pour fond. Tout ou rien.
    pub fn import_document(
        &self,
        parent: NodeId,
        name: &str,
        pages: &[ImportedPage],
    ) -> Result<Node> {
        let container = match self.node(parent)?.kind {
            NodeKind::Subject => NodeKind::Notebook,
            _ => NodeKind::Folder,
        };
        self.conn.execute_batch("SAVEPOINT import_document")?;
        let result = (|| -> Result<Node> {
            let root = self.create_node(Some(parent), container, name)?;
            for (i, page) in pages.iter().enumerate() {
                let node =
                    self.create_node(Some(root.id), NodeKind::Page, &format!("Page {}", i + 1))?;
                let asset = self.add_asset(&page.mime, page.width, page.height, &page.data)?;
                self.set_background(node.id, Background::Image(asset))?;
            }
            Ok(root)
        })();
        match &result {
            Ok(_) => self.conn.execute_batch("RELEASE import_document")?,
            Err(_) => self
                .conn
                .execute_batch("ROLLBACK TO import_document; RELEASE import_document")?,
        }
        result
    }

    // ---- Réglages ------------------------------------------------------------------------

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    // ---- Utilitaires internes ------------------------------------------------------------

    fn expect_changed(&self, id: NodeId, changed: usize) -> Result<()> {
        if changed == 0 {
            Err(StoreError::NotFound(id))
        } else {
            Ok(())
        }
    }

    fn is_in_subtree(&self, root: NodeId, candidate: NodeId) -> Result<bool> {
        Ok(self.conn.query_row(
            &format!("{SUBTREE_CTE} SELECT EXISTS(SELECT 1 FROM sub WHERE id = ?2)"),
            params![root.to_string(), candidate.to_string()],
            |r| r.get(0),
        )?)
    }

    fn sibling_ids(&self, parent: Option<NodeId>) -> Result<Vec<NodeId>> {
        sibling_ids_in(&self.conn, parent)
    }
}

const NODE_COLUMNS: &str = "id, parent_id, kind, name, color, icon, position";

/// Sous-arbre de `?1`, lui-même inclus (table temporaire `sub`).
const SUBTREE_CTE: &str = "WITH RECURSIVE sub(id) AS (
    SELECT ?1
    UNION ALL
    SELECT n.id FROM nodes n JOIN sub s ON n.parent_id = s.id
)";

fn sibling_ids_in(conn: &Connection, parent: Option<NodeId>) -> Result<Vec<NodeId>> {
    let mut stmt = conn.prepare(
        "SELECT id FROM nodes WHERE parent_id IS ?1 AND trashed_at IS NULL
         ORDER BY position, created_at, name",
    )?;
    let rows = stmt.query_map([parent.map(|p| p.to_string())], |r| {
        parse_id(r.get::<_, String>(0)?, 0)
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn write_positions(conn: &Connection, ids: &[NodeId]) -> Result<()> {
    let mut stmt = conn.prepare("UPDATE nodes SET position = ?2 WHERE id = ?1")?;
    for (i, id) in ids.iter().enumerate() {
        stmt.execute(params![id.to_string(), i as i64])?;
    }
    Ok(())
}

/// Copie `src` et ses descendants sous `parent` ; `suffix` s'ajoute au nom de la racine copiée.
fn copy_subtree(
    conn: &Connection,
    src: NodeId,
    parent: Option<NodeId>,
    suffix: Option<&str>,
) -> Result<NodeId> {
    let node: Node = conn
        .query_row(
            &format!("SELECT {NODE_COLUMNS} FROM nodes WHERE id = ?1"),
            [src.to_string()],
            node_from_row,
        )
        .optional()?
        .ok_or(StoreError::NotFound(src))?;
    let new_id = NodeId::new();
    let name = format!("{}{}", node.name, suffix.unwrap_or(""));
    conn.execute(
        "INSERT INTO nodes (id, parent_id, kind, name, color, icon, position, created_at, background)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                 (SELECT background FROM nodes WHERE id = ?9))",
        params![
            new_id.to_string(),
            parent.map(|p| p.to_string()),
            node.kind.as_str(),
            name,
            pack_color(node.color),
            node.icon,
            node.position,
            now(),
            src.to_string(),
        ],
    )?;
    conn.execute(
        "INSERT INTO page_data (page_id, format, data, updated_at)
         SELECT ?2, format, data, updated_at FROM page_data WHERE page_id = ?1",
        params![src.to_string(), new_id.to_string()],
    )?;
    let children: Vec<NodeId> = {
        let mut stmt =
            conn.prepare("SELECT id FROM nodes WHERE parent_id = ?1 AND trashed_at IS NULL")?;
        let rows = stmt.query_map([src.to_string()], |r| parse_id(r.get::<_, String>(0)?, 0))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    for child in children {
        copy_subtree(conn, child, Some(new_id), None)?;
    }
    Ok(new_id)
}

fn decode_page(format: i64, bytes: &[u8]) -> Result<Page> {
    match format {
        1 => {
            let legacy: PageV1 = postcard::from_bytes(bytes)?;
            Ok(Page {
                items: legacy.strokes.into_iter().map(Item::Stroke).collect(),
            })
        }
        2 => Ok(postcard::from_bytes(bytes)?),
        other => Err(StoreError::UnsupportedPageFormat(other)),
    }
}

fn clean_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        Err(StoreError::EmptyName)
    } else {
        Ok(name.to_owned())
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

fn pack_color(c: Color) -> i64 {
    i64::from(u32::from_be_bytes([c.r, c.g, c.b, c.a]))
}

fn unpack_color(v: i64) -> Color {
    let [r, g, b, a] = (v as u32).to_be_bytes();
    Color { r, g, b, a }
}

fn parse_id(text: String, column: usize) -> rusqlite::Result<NodeId> {
    text.parse()
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(column, Type::Text, Box::new(e)))
}

fn node_from_row(row: &Row<'_>) -> rusqlite::Result<Node> {
    let parent_id = row
        .get::<_, Option<String>>(1)?
        .map(|p| parse_id(p, 1))
        .transpose()?;
    let kind_text: String = row.get(2)?;
    let kind = NodeKind::parse(&kind_text).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            2,
            Type::Text,
            format!("type inconnu : {kind_text}").into(),
        )
    })?;
    Ok(Node {
        id: parse_id(row.get::<_, String>(0)?, 0)?,
        parent_id,
        kind,
        name: row.get(3)?,
        color: unpack_color(row.get(4)?),
        icon: row.get(5)?,
        position: row.get(6)?,
    })
}

mod backup;
pub use backup::{restore_backup, DB_ENTRY};

#[cfg(test)]
mod tests;
