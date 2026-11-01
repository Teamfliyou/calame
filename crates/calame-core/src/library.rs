//! Arborescence de la bibliothèque : matières > cahiers > sous-dossiers > pages.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::geometry::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodeId(Uuid);

impl NodeId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for NodeId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    /// Dossier de premier niveau (Maths, Histoire…).
    Subject,
    Notebook,
    /// Sous-dossier imbriqué sans limite de profondeur.
    Folder,
    Page,
}

impl NodeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Subject => "subject",
            Self::Notebook => "notebook",
            Self::Folder => "folder",
            Self::Page => "page",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "subject" => Some(Self::Subject),
            "notebook" => Some(Self::Notebook),
            "folder" => Some(Self::Folder),
            "page" => Some(Self::Page),
            _ => None,
        }
    }

    /// Un cahier ou un sous-dossier contient des sous-dossiers et des pages ;
    /// une matière ne contient que des cahiers ; une page ne contient rien.
    pub fn can_contain(self, child: NodeKind) -> bool {
        matches!(
            (self, child),
            (Self::Subject, Self::Notebook)
                | (Self::Notebook | Self::Folder, Self::Folder | Self::Page)
        )
    }

    pub fn is_container(self) -> bool {
        self != Self::Page
    }

    pub fn default_icon(self) -> &'static str {
        match self {
            Self::Subject => "📚",
            Self::Notebook => "📓",
            Self::Folder => "📂",
            Self::Page => "📄",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum LibraryError {
    #[error("placement invalide : {child:?} ne peut pas aller dans {parent:?}")]
    InvalidParent {
        parent: Option<NodeKind>,
        child: NodeKind,
    },
    #[error("impossible de déplacer un élément dans l'un de ses descendants")]
    Cycle,
}

/// Vérifie qu'un élément de type `child` peut être placé sous `parent` (`None` = racine).
pub fn check_placement(parent: Option<NodeKind>, child: NodeKind) -> Result<(), LibraryError> {
    let ok = match parent {
        None => child == NodeKind::Subject,
        Some(p) => p.can_contain(child),
    };
    if ok {
        Ok(())
    } else {
        Err(LibraryError::InvalidParent { parent, child })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub id: NodeId,
    pub parent_id: Option<NodeId>,
    pub kind: NodeKind,
    pub name: String,
    pub color: Color,
    pub icon: String,
    pub position: i64,
}

/// Vue en mémoire de l'arborescence (éléments hors corbeille), pour l'affichage.
#[derive(Clone, Debug, Default)]
pub struct LibraryTree {
    nodes: HashMap<NodeId, Node>,
    children: HashMap<Option<NodeId>, Vec<NodeId>>,
}

impl LibraryTree {
    pub fn from_nodes(mut nodes: Vec<Node>) -> Self {
        nodes.sort_by(|a, b| {
            a.position
                .cmp(&b.position)
                .then_with(|| a.name.cmp(&b.name))
        });
        let mut tree = Self::default();
        for n in nodes {
            tree.children.entry(n.parent_id).or_default().push(n.id);
            tree.nodes.insert(n.id, n);
        }
        tree
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(&id)
    }

    /// Enfants de `parent` (`None` = matières), dans l'ordre d'affichage.
    pub fn children(&self, parent: Option<NodeId>) -> Vec<&Node> {
        self.children
            .get(&parent)
            .map(|ids| ids.iter().filter_map(|i| self.nodes.get(i)).collect())
            .unwrap_or_default()
    }

    /// Chemin de la racine jusqu'à `id` inclus.
    pub fn path(&self, id: NodeId) -> Vec<&Node> {
        let mut path = Vec::new();
        let mut cur = self.get(id);
        while let Some(n) = cur {
            path.push(n);
            cur = n.parent_id.and_then(|p| self.get(p));
        }
        path.reverse();
        path
    }

    /// `true` si `id` est `ancestor` ou se trouve sous lui.
    pub fn is_within(&self, id: NodeId, ancestor: NodeId) -> bool {
        self.path(id).iter().any(|n| n.id == ancestor)
    }

    /// Où atterrit `dragged` s'il est déposé sur `target` : `(nouveau parent, index)`.
    ///
    /// Sur un conteneur qui peut l'accueillir, l'élément y entre à la fin ; sinon il se place
    /// juste avant `target` parmi ses frères, si ce placement est valide. `None` : dépôt refusé.
    pub fn drop_target(
        &self,
        dragged: NodeId,
        target: NodeId,
    ) -> Option<(Option<NodeId>, Option<usize>)> {
        let (dragged, target) = (self.get(dragged)?, self.get(target)?);
        if dragged.id == target.id || self.is_within(target.id, dragged.id) {
            return None;
        }
        if target.kind.can_contain(dragged.kind) {
            return Some((Some(target.id), None));
        }
        let parent_kind = target.parent_id.and_then(|p| self.get(p)).map(|p| p.kind);
        check_placement(parent_kind, dragged.kind).ok()?;
        let index = self
            .children(target.parent_id)
            .iter()
            .filter(|n| n.id != dragged.id)
            .position(|n| n.id == target.id)?;
        Some((target.parent_id, Some(index)))
    }

    /// Première page dans l'ordre de l'arborescence.
    pub fn first_page(&self) -> Option<NodeId> {
        fn walk(tree: &LibraryTree, parent: Option<NodeId>) -> Option<NodeId> {
            for n in tree.children(parent) {
                if n.kind == NodeKind::Page {
                    return Some(n.id);
                }
                if let Some(found) = walk(tree, Some(n.id)) {
                    return Some(found);
                }
            }
            None
        }
        walk(self, None)
    }

    /// Première page sous `root` (inclus), dans l'ordre de l'arborescence.
    pub fn first_page_in(&self, root: NodeId) -> Option<NodeId> {
        fn walk(tree: &LibraryTree, id: NodeId) -> Option<NodeId> {
            let node = tree.get(id)?;
            if node.kind == NodeKind::Page {
                return Some(id);
            }
            tree.children(Some(id))
                .into_iter()
                .find_map(|child| walk(tree, child.id))
        }
        walk(self, root)
    }

    /// Cahier qui contient `id` (ou `id` lui-même si c'est un cahier).
    pub fn notebook_of(&self, id: NodeId) -> Option<NodeId> {
        self.path(id)
            .iter()
            .find(|n| n.kind == NodeKind::Notebook)
            .map(|n| n.id)
    }

    /// Nombre de pages sous `id` (inclus).
    pub fn page_count(&self, id: NodeId) -> usize {
        let Some(node) = self.get(id) else {
            return 0;
        };
        if node.kind == NodeKind::Page {
            return 1;
        }
        self.children(Some(id))
            .into_iter()
            .map(|child| self.page_count(child.id))
            .sum()
    }

    /// Éléments dont le nom contient `query` (insensible à la casse), plus leurs ancêtres.
    pub fn matching(&self, query: &str) -> HashSet<NodeId> {
        let query = query.trim().to_lowercase();
        let mut keep = HashSet::new();
        if query.is_empty() {
            return keep;
        }
        for n in self.nodes.values() {
            if n.name.to_lowercase().contains(&query) {
                keep.extend(self.path(n.id).iter().map(|a| a.id));
            }
        }
        keep
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(parent: Option<NodeId>, kind: NodeKind, name: &str, position: i64) -> Node {
        Node {
            id: NodeId::new(),
            parent_id: parent,
            kind,
            name: name.into(),
            color: Color::BLACK,
            icon: String::new(),
            position,
        }
    }

    #[test]
    fn placement_rules() {
        use NodeKind::*;
        assert!(check_placement(None, Subject).is_ok());
        assert!(check_placement(None, Notebook).is_err());
        assert!(check_placement(Some(Subject), Notebook).is_ok());
        assert!(check_placement(Some(Subject), Page).is_err());
        assert!(check_placement(Some(Notebook), Folder).is_ok());
        assert!(check_placement(Some(Folder), Folder).is_ok());
        assert!(check_placement(Some(Folder), Page).is_ok());
        assert!(check_placement(Some(Page), Page).is_err());
        assert!(check_placement(Some(Folder), Notebook).is_err());
    }

    #[test]
    fn kind_string_roundtrip() {
        for k in [
            NodeKind::Subject,
            NodeKind::Notebook,
            NodeKind::Folder,
            NodeKind::Page,
        ] {
            assert_eq!(NodeKind::parse(k.as_str()), Some(k));
        }
        assert_eq!(NodeKind::parse("autre"), None);
    }

    #[test]
    fn tree_orders_children_and_finds_paths() {
        let math = node(None, NodeKind::Subject, "Maths", 0);
        let histoire = node(None, NodeKind::Subject, "Histoire", 1);
        let cahier = node(Some(math.id), NodeKind::Notebook, "Cahier", 0);
        let dossier = node(Some(cahier.id), NodeKind::Folder, "Chapitre 1", 0);
        let page_b = node(Some(dossier.id), NodeKind::Page, "B", 1);
        let page_a = node(Some(dossier.id), NodeKind::Page, "A", 0);
        let (math_id, page_a_id, dossier_id) = (math.id, page_a.id, dossier.id);
        let tree = LibraryTree::from_nodes(vec![page_b, histoire, page_a, dossier, cahier, math]);

        let names: Vec<_> = tree.children(None).iter().map(|n| n.name.clone()).collect();
        assert_eq!(names, ["Maths", "Histoire"]);
        let pages: Vec<_> = tree
            .children(Some(dossier_id))
            .iter()
            .map(|n| n.name.clone())
            .collect();
        assert_eq!(pages, ["A", "B"]);
        let path: Vec<_> = tree
            .path(page_a_id)
            .iter()
            .map(|n| n.name.clone())
            .collect();
        assert_eq!(path, ["Maths", "Cahier", "Chapitre 1", "A"]);
        assert!(tree.is_within(page_a_id, math_id));
        assert_eq!(tree.first_page(), Some(page_a_id));
    }

    #[test]
    fn drop_target_enters_containers_or_reorders_before_target() {
        let s1 = node(None, NodeKind::Subject, "S1", 0);
        let s2 = node(None, NodeKind::Subject, "S2", 1);
        let nb = node(Some(s1.id), NodeKind::Notebook, "Cahier", 0);
        let fo = node(Some(nb.id), NodeKind::Folder, "Dossier", 0);
        let p1 = node(Some(nb.id), NodeKind::Page, "P1", 1);
        let p2 = node(Some(nb.id), NodeKind::Page, "P2", 2);
        let (s1_id, s2_id, nb_id, fo_id) = (s1.id, s2.id, nb.id, fo.id);
        let (p1_id, p2_id) = (p1.id, p2.id);
        let tree = LibraryTree::from_nodes(vec![s1, s2, nb, fo, p1, p2]);

        // Un cahier déposé sur une autre matière y entre.
        assert_eq!(tree.drop_target(nb_id, s2_id), Some((Some(s2_id), None)));
        // Une page déposée sur un dossier y entre.
        assert_eq!(tree.drop_target(p1_id, fo_id), Some((Some(fo_id), None)));
        // Une page déposée sur une autre page se place avant elle.
        assert_eq!(tree.drop_target(p2_id, p1_id), Some((Some(nb_id), Some(1))));
        // Une matière déposée sur une matière se réordonne à la racine.
        assert_eq!(tree.drop_target(s2_id, s1_id), Some((None, Some(0))));
        // Refus : sur soi-même, dans son descendant, ou placement invalide.
        assert_eq!(tree.drop_target(nb_id, nb_id), None);
        assert_eq!(tree.drop_target(nb_id, fo_id), None);
        assert_eq!(tree.drop_target(p1_id, s1_id), None);
    }

    #[test]
    fn notebook_helpers_find_pages_and_counts() {
        let s = node(None, NodeKind::Subject, "S", 0);
        let nb = node(Some(s.id), NodeKind::Notebook, "Cahier", 0);
        let sub = node(Some(nb.id), NodeKind::Folder, "Sous-matière", 0);
        let deep = node(Some(sub.id), NodeKind::Page, "Profonde", 0);
        let top = node(Some(nb.id), NodeKind::Page, "Haute", 1);
        let empty = node(Some(s.id), NodeKind::Notebook, "Vide", 1);
        let (s_id, nb_id, deep_id, empty_id) = (s.id, nb.id, deep.id, empty.id);
        let tree = LibraryTree::from_nodes(vec![s, nb, sub, deep, top, empty]);

        assert_eq!(tree.first_page_in(nb_id), Some(deep_id));
        assert_eq!(tree.first_page_in(empty_id), None);
        assert_eq!(tree.notebook_of(deep_id), Some(nb_id));
        assert_eq!(tree.notebook_of(s_id), None);
        assert_eq!(tree.page_count(nb_id), 2);
        assert_eq!(tree.page_count(s_id), 2);
        assert_eq!(tree.page_count(empty_id), 0);
    }

    #[test]
    fn matching_keeps_ancestors_and_ignores_case() {
        let s = node(None, NodeKind::Subject, "Physique", 0);
        let c = node(Some(s.id), NodeKind::Notebook, "Optique", 0);
        let other = node(None, NodeKind::Subject, "Arts", 1);
        let (s_id, c_id, other_id) = (s.id, c.id, other.id);
        let tree = LibraryTree::from_nodes(vec![s, c, other]);
        let hits = tree.matching("OPTI");
        assert!(hits.contains(&s_id) && hits.contains(&c_id));
        assert!(!hits.contains(&other_id));
        assert!(tree.matching("  ").is_empty());
    }
}
