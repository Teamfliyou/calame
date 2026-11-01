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
        _ => "?",
    }
}

#[cfg(test)]
mod tests {
    use super::t;

    #[test]
    fn known_keys_are_translated() {
        for key in ["tool-pen", "tool-highlighter", "width", "undo", "redo"] {
            assert_ne!(t(key), "?", "clé manquante : {key}");
        }
    }
}
