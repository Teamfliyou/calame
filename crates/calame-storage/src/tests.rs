#![allow(clippy::expect_used)]

use calame_core::{
    Background, Brush, Item, LibraryError, NodeKind, Page, Stroke, StrokePoint, TextBox,
};

use super::*;

fn store() -> Store {
    Store::open_in_memory().expect("base en mémoire")
}

fn names(nodes: &[Node]) -> Vec<&str> {
    nodes.iter().map(|n| n.name.as_str()).collect()
}

fn children(s: &Store, parent: Option<NodeId>) -> Vec<Node> {
    let mut v: Vec<Node> = s
        .all_nodes()
        .expect("lecture")
        .into_iter()
        .filter(|n| n.parent_id == parent)
        .collect();
    v.sort_by_key(|n| n.position);
    v
}

/// Maths > Cahier > Chapitre 1 > Page 1
fn sample(s: &Store) -> (Node, Node, Node, Node) {
    let subject = s
        .create_node(None, NodeKind::Subject, "Maths")
        .expect("matière");
    let notebook = s
        .create_node(Some(subject.id), NodeKind::Notebook, "Cahier")
        .expect("cahier");
    let folder = s
        .create_node(Some(notebook.id), NodeKind::Folder, "Chapitre 1")
        .expect("dossier");
    let page = s
        .create_node(Some(folder.id), NodeKind::Page, "Page 1")
        .expect("page");
    (subject, notebook, folder, page)
}

#[test]
fn rejects_invalid_placement_and_empty_names() {
    let s = store();
    let (subject, _, _, page) = sample(&s);
    assert!(matches!(
        s.create_node(None, NodeKind::Page, "x"),
        Err(StoreError::Library(LibraryError::InvalidParent { .. }))
    ));
    assert!(matches!(
        s.create_node(Some(subject.id), NodeKind::Page, "x"),
        Err(StoreError::Library(_))
    ));
    assert!(matches!(
        s.create_node(Some(page.id), NodeKind::Folder, "x"),
        Err(StoreError::Library(_))
    ));
    assert!(matches!(
        s.create_node(None, NodeKind::Subject, "   "),
        Err(StoreError::EmptyName)
    ));
}

#[test]
fn subjects_get_distinct_default_colors() {
    let s = store();
    let a = s.create_node(None, NodeKind::Subject, "A").expect("a");
    let b = s.create_node(None, NodeKind::Subject, "B").expect("b");
    assert_ne!(a.color, b.color);
}

#[test]
fn rename_and_style_are_persisted() {
    let s = store();
    let (subject, ..) = sample(&s);
    s.rename(subject.id, "  Mathématiques ").expect("renommer");
    s.set_style(subject.id, Color::rgb(1, 2, 3), "🧮")
        .expect("style");
    let n = s.node(subject.id).expect("lecture");
    assert_eq!(n.name, "Mathématiques");
    assert_eq!(n.color, Color::rgb(1, 2, 3));
    assert_eq!(n.icon, "🧮");
    assert!(matches!(
        s.rename(NodeId::new(), "x"),
        Err(StoreError::NotFound(_))
    ));
}

#[test]
fn move_between_containers_and_reject_cycles() {
    let mut s = store();
    let (_, notebook, folder, page) = sample(&s);
    let other = s
        .create_node(Some(notebook.id), NodeKind::Folder, "Exercices")
        .expect("dossier");

    s.move_node(page.id, Some(other.id), None)
        .expect("déplacer");
    assert_eq!(s.node(page.id).expect("lecture").parent_id, Some(other.id));

    // Un dossier dans son propre descendant ou dans lui-même est refusé.
    s.move_node(folder.id, Some(other.id), None)
        .expect("dossier dans frère");
    assert!(matches!(
        s.move_node(folder.id, Some(page.id), None),
        Err(StoreError::Library(_))
    ));
    assert!(matches!(
        s.move_node(other.id, Some(folder.id), None),
        Err(StoreError::Library(LibraryError::Cycle))
    ));
    assert!(matches!(
        s.move_node(folder.id, Some(folder.id), None),
        Err(StoreError::Library(LibraryError::Cycle))
    ));
}

#[test]
fn reorder_with_index_and_move_by() {
    let mut s = store();
    let a = s.create_node(None, NodeKind::Subject, "A").expect("a");
    let b = s.create_node(None, NodeKind::Subject, "B").expect("b");
    let c = s.create_node(None, NodeKind::Subject, "C").expect("c");

    s.move_node(c.id, None, Some(0)).expect("réordonner");
    assert_eq!(names(&children(&s, None)), ["C", "A", "B"]);

    s.move_by(c.id, 1).expect("descendre");
    assert_eq!(names(&children(&s, None)), ["A", "C", "B"]);
    s.move_by(a.id, -5).expect("déjà en haut");
    assert_eq!(names(&children(&s, None)), ["A", "C", "B"]);
    s.move_by(b.id, 5).expect("déjà en bas");
    assert_eq!(names(&children(&s, None)), ["A", "C", "B"]);
}

#[test]
fn duplicate_copies_subtree_and_page_content() {
    let mut s = store();
    let (subject, notebook, _, page) = sample(&s);
    let mut content = Page::default();
    let mut stroke = Stroke::new(Brush::default());
    stroke.push(StrokePoint {
        pos: calame_core::Point2::new(1.0, 2.0),
        pressure: 0.7,
        tilt: None,
    });
    content.items.push(Item::Stroke(stroke));
    s.save_page(page.id, &content).expect("sauvegarde");

    let copy = s.duplicate(notebook.id).expect("dupliquer");
    assert_eq!(copy.name, "Cahier (copie)");
    assert_eq!(copy.parent_id, Some(subject.id));
    assert_eq!(
        names(&children(&s, Some(subject.id))),
        ["Cahier", "Cahier (copie)"]
    );

    let copied_folder = children(&s, Some(copy.id));
    assert_eq!(names(&copied_folder), ["Chapitre 1"]);
    let copied_page = children(&s, Some(copied_folder[0].id));
    assert_eq!(names(&copied_page), ["Page 1"]);
    assert_ne!(copied_page[0].id, page.id);
    assert_eq!(s.load_page(copied_page[0].id).expect("copie"), content);
}

#[test]
fn trash_restore_and_permanent_delete() {
    let s = store();
    let (subject, notebook, folder, page) = sample(&s);

    s.trash(notebook.id).expect("corbeille");
    assert_eq!(names(&s.all_nodes().expect("lecture")), ["Maths"]);
    let trashed = s.trashed().expect("corbeille");
    assert_eq!(
        names(&trashed),
        ["Cahier"],
        "seul le haut du sous-arbre est listé"
    );

    // On ne peut pas supprimer définitivement ce qui n'est pas dans la corbeille.
    assert!(matches!(
        s.delete_permanently(subject.id),
        Err(StoreError::NotTrashed)
    ));

    s.restore(notebook.id).expect("restaurer");
    assert_eq!(s.all_nodes().expect("lecture").len(), 4);
    assert!(s.trashed().expect("corbeille").is_empty());

    s.trash(folder.id).expect("corbeille");
    s.empty_trash().expect("vider");
    assert!(matches!(s.node(page.id), Err(StoreError::NotFound(_))));
    assert_eq!(s.all_nodes().expect("lecture").len(), 2);
}

#[test]
fn page_roundtrip_and_empty_default() {
    let s = store();
    let (subject, _, _, page) = sample(&s);
    assert_eq!(s.load_page(page.id).expect("vide"), Page::default());

    let mut content = Page::default();
    for i in 0..3 {
        let mut stroke = Stroke::new(Brush::default());
        for j in 0..10 {
            stroke.push(StrokePoint {
                pos: calame_core::Point2::new(i as f32, j as f32),
                pressure: 0.1 * j as f32,
                tilt: Some(0.5),
            });
        }
        content.items.push(Item::Stroke(stroke));
    }
    content.items.push(Item::Text(TextBox::new(
        calame_core::Point2::new(3.0, 4.0),
        "Bonjour".into(),
        16.0,
        calame_core::Color::rgb(10, 20, 30),
    )));
    s.save_page(page.id, &content).expect("sauvegarde");
    s.save_page(page.id, &content).expect("écrasement");
    assert_eq!(s.load_page(page.id).expect("lecture"), content);
    assert!(matches!(
        s.save_page(subject.id, &content),
        Err(StoreError::NotAPage)
    ));
}

#[test]
fn search_ignores_case_and_trash() {
    let s = store();
    let (_, _, _, page) = sample(&s);
    assert_eq!(
        names(&s.search("CHAPITRE").expect("recherche")),
        ["Chapitre 1"]
    );
    assert!(s.search("  ").expect("recherche").is_empty());
    s.trash(page.id).expect("corbeille");
    assert!(s.search("page").expect("recherche").is_empty());
}

#[test]
fn data_survives_reopening_the_file() {
    let dir = std::env::temp_dir().join(format!("calame-test-{}", NodeId::new()));
    let path = dir.join("calame.db");
    let page_id = {
        let s = Store::open(&path).expect("ouverture");
        let (_, _, _, page) = sample(&s);
        s.save_page(page.id, &Page::default()).expect("sauvegarde");
        page.id
    };
    let s = Store::open(&path).expect("réouverture");
    assert_eq!(s.node(page_id).expect("lecture").name, "Page 1");
    assert_eq!(s.all_nodes().expect("lecture").len(), 4);
    drop(s);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn legacy_v1_pages_are_converted() {
    let s = store();
    let (_, _, _, page) = sample(&s);
    let stroke = Stroke::new(Brush::default());
    let bytes = postcard::to_allocvec(&PageV1 {
        strokes: vec![stroke.clone()],
    })
    .expect("encodage");
    s.conn
        .execute(
            "INSERT INTO page_data (page_id, format, data, updated_at) VALUES (?1, 1, ?2, 0)",
            params![page.id.to_string(), bytes],
        )
        .expect("insertion");
    assert_eq!(
        s.load_page(page.id).expect("lecture"),
        Page {
            items: vec![Item::Stroke(stroke)]
        }
    );
    // Réenregistrée, la page passe au format courant.
    let loaded = s.load_page(page.id).expect("lecture");
    s.save_page(page.id, &loaded).expect("sauvegarde");
    let format: i64 = s
        .conn
        .query_row("SELECT format FROM page_data", [], |r| r.get(0))
        .expect("format");
    assert_eq!(format, PAGE_FORMAT);
}

#[test]
fn unknown_page_format_is_an_error() {
    let s = store();
    let (_, _, _, page) = sample(&s);
    s.conn
        .execute(
            "INSERT INTO page_data (page_id, format, data, updated_at) VALUES (?1, 99, x'00', 0)",
            [page.id.to_string()],
        )
        .expect("insertion");
    assert!(matches!(
        s.load_page(page.id),
        Err(StoreError::UnsupportedPageFormat(99))
    ));
}

#[test]
fn settings_roundtrip_and_overwrite() {
    let s = store();
    assert_eq!(s.setting("favoris").expect("lecture"), None);
    s.set_setting("favoris", "#000000").expect("écriture");
    s.set_setting("favoris", "#ff0000,#00ff00")
        .expect("écrasement");
    assert_eq!(
        s.setting("favoris").expect("lecture").as_deref(),
        Some("#ff0000,#00ff00")
    );
}

#[test]
fn backgrounds_default_to_blank_and_roundtrip() {
    let s = store();
    let (subject, _, _, page) = sample(&s);
    assert_eq!(s.background(page.id).expect("lecture"), Background::Blank);
    s.set_background(page.id, Background::GridLarge)
        .expect("fond");
    assert_eq!(
        s.background(page.id).expect("lecture"),
        Background::GridLarge
    );
    assert!(matches!(
        s.set_background(subject.id, Background::Dots),
        Err(StoreError::NotAPage)
    ));
    assert!(matches!(
        s.set_background(page.id, Background::Image(calame_core::AssetId::new())),
        Err(StoreError::AssetNotFound(_))
    ));
}

#[test]
fn image_backgrounds_are_stored_copied_and_purged() {
    let mut s = store();
    let (_, notebook, _, page) = sample(&s);
    let asset = s
        .add_asset("image/png", 800.0, 1100.0, &[1, 2, 3])
        .expect("image");
    s.set_background(page.id, Background::Image(asset))
        .expect("fond");
    let loaded = s.asset(asset).expect("lecture");
    assert_eq!(
        (loaded.mime.as_str(), loaded.width, loaded.data.clone()),
        ("image/png", 800.0, vec![1, 2, 3])
    );

    // Dupliquer le cahier garde le fond de la copie, avec la même image.
    let copy = s.duplicate(notebook.id).expect("dupliquer");
    let folder = children(&s, Some(copy.id));
    let copied_page = children(&s, Some(folder[0].id));
    assert_eq!(
        s.background(copied_page[0].id).expect("fond"),
        Background::Image(asset)
    );

    // L'image reste tant qu'une page l'utilise ; elle disparaît avec la dernière.
    s.trash(notebook.id).expect("corbeille");
    s.empty_trash().expect("vider");
    assert!(s.asset(asset).is_ok(), "la copie utilise encore l'image");
    s.trash(copy.id).expect("corbeille");
    s.empty_trash().expect("vider");
    assert!(matches!(s.asset(asset), Err(StoreError::AssetNotFound(_))));
}

#[test]
fn changing_a_background_frees_the_previous_image() {
    let s = store();
    let (_, _, _, page) = sample(&s);
    let asset = s.add_asset("image/png", 10.0, 10.0, &[9]).expect("image");
    s.set_background(page.id, Background::Image(asset))
        .expect("fond");
    s.set_background(page.id, Background::Dots).expect("fond");
    assert!(matches!(s.asset(asset), Err(StoreError::AssetNotFound(_))));
}

#[test]
fn importing_a_document_builds_a_notebook_with_one_page_per_image() {
    let s = store();
    let (subject, notebook, _, _) = sample(&s);
    let pages: Vec<ImportedPage> = (0..3)
        .map(|i| ImportedPage {
            mime: "image/png".into(),
            width: 794.0,
            height: 1123.0,
            data: vec![i],
        })
        .collect();

    let root = s
        .import_document(subject.id, "Cours", &pages)
        .expect("import");
    assert_eq!(root.kind, NodeKind::Notebook);
    let created = children(&s, Some(root.id));
    assert_eq!(names(&created), ["Page 1", "Page 2", "Page 3"]);
    for (page, expected) in created.iter().zip(0u8..) {
        let Background::Image(id) = s.background(page.id).expect("fond") else {
            panic!("fond image attendu");
        };
        assert_eq!(s.asset(id).expect("image").data, vec![expected]);
    }

    // Dans un cahier, le document devient un sous-dossier.
    let folder = s
        .import_document(notebook.id, "Annexe", &pages[..1])
        .expect("import");
    assert_eq!(folder.kind, NodeKind::Folder);
    // Sous une page, c'est refusé et rien ne reste.
    let page_id = children(&s, Some(created[0].parent_id.expect("parent")))[0].id;
    let before = s.all_nodes().expect("lecture").len();
    assert!(s.import_document(page_id, "X", &pages).is_err());
    assert_eq!(s.all_nodes().expect("lecture").len(), before);
}

#[test]
fn schema_v2_databases_are_upgraded_in_place() {
    let dir = std::env::temp_dir().join(format!("calame-migrate-{}", NodeId::new()));
    let path = dir.join("calame.db");
    std::fs::create_dir_all(&dir).expect("dossier");
    // Une base telle que la phase 4 l'écrivait (schéma 2, sans fonds ni images).
    {
        let conn = Connection::open(&path).expect("base");
        conn.execute_batch(
            "CREATE TABLE nodes (id TEXT PRIMARY KEY, parent_id TEXT REFERENCES nodes(id) ON DELETE CASCADE,
                kind TEXT NOT NULL, name TEXT NOT NULL, color INTEGER NOT NULL, icon TEXT NOT NULL,
                position INTEGER NOT NULL, created_at INTEGER NOT NULL, trashed_at INTEGER);
             CREATE TABLE page_data (page_id TEXT PRIMARY KEY, format INTEGER NOT NULL, data BLOB NOT NULL, updated_at INTEGER NOT NULL);
             CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO nodes VALUES ('11111111-1111-1111-1111-111111111111', NULL, 'subject', 'Ancienne', 0, '', 0, 0, NULL);
             PRAGMA user_version = 2;",
        )
        .expect("schéma 2");
    }
    let s = Store::open(&path).expect("migration");
    let old: NodeId = "11111111-1111-1111-1111-111111111111".parse().expect("id");
    assert_eq!(s.node(old).expect("lecture").name, "Ancienne");
    let page = s
        .create_node(
            Some(
                s.create_node(Some(old), NodeKind::Notebook, "C")
                    .expect("cahier")
                    .id,
            ),
            NodeKind::Page,
            "P",
        )
        .expect("page");
    assert_eq!(s.background(page.id).expect("fond"), Background::Blank);
    drop(s);
    let _ = std::fs::remove_dir_all(dir);
}
