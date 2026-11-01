//! Textes de l'interface, externalisés pour permettre d'autres langues.
//!
//! Pour l'instant : une table française statique. Le passage à `fluent` (fichiers `.ftl`)
//! se fera sans changer les appelants (`t("clé")`).

pub fn t(key: &str) -> &'static str {
    match key {
        "tool-pen" => "Stylo",
        "tool-highlighter" => "Surligneur",
        "width" => "Épaisseur",
        "undo" => "Annuler",
        "redo" => "Rétablir",
        "reset-view" => "Recentrer",
        "dark-theme" => "Thème sombre",
        "library" => "Bibliothèque",
        "search-hint" => "Rechercher…",
        "new-subject" => "+ Matière",
        "new-notebook" => "Nouveau cahier",
        "new-folder" => "Nouveau sous-dossier",
        "new-page" => "Nouvelle page",
        "default-subject" => "Nouvelle matière",
        "default-notebook" => "Nouveau cahier",
        "default-folder" => "Nouveau dossier",
        "default-page" => "Nouvelle page",
        "welcome-subject" => "Mes notes",
        "welcome-notebook" => "Cahier 1",
        "welcome-page" => "Page 1",
        "rename" => "Renommer",
        "duplicate" => "Dupliquer",
        "move-up" => "Monter",
        "move-down" => "Descendre",
        "to-trash" => "Mettre à la corbeille",
        "style" => "Style…",
        "style-title" => "Style de la matière",
        "color" => "Couleur",
        "icon" => "Icône",
        "apply" => "Appliquer",
        "cancel" => "Annuler",
        "trash" => "Corbeille",
        "trash-empty" => "La corbeille est vide",
        "restore" => "Restaurer",
        "delete-forever" => "Supprimer définitivement",
        "empty-trash" => "Vider la corbeille",
        "no-results" => "Aucun résultat",
        "no-page" => "Sélectionne ou crée une page dans la bibliothèque.",
        "dismiss" => "Fermer",
        _ => "?",
    }
}

#[cfg(test)]
mod tests {
    use super::t;

    #[test]
    fn known_keys_are_translated() {
        for key in [
            "tool-pen",
            "tool-highlighter",
            "width",
            "undo",
            "redo",
            "library",
            "new-subject",
            "to-trash",
            "no-page",
        ] {
            assert_ne!(t(key), "?", "clé manquante : {key}");
        }
    }
}
