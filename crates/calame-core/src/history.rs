use std::collections::HashMap;

use uuid::Uuid;

use crate::item::Item;
use crate::page::Page;

/// Modification réversible d'une page, exprimée comme un retrait suivi d'un ajout.
///
/// - `removed` : éléments retirés, avec leur index dans la page *avant* l'opération (croissant) ;
/// - `added` : éléments ajoutés, avec leur index dans la page *après* l'opération (croissant).
///
/// Ajouter, supprimer, déplacer, recolorer, coller ou effacer par zone sont tous des cas particuliers.
#[derive(Clone, Debug)]
struct Command {
    removed: Vec<(usize, Item)>,
    added: Vec<(usize, Item)>,
}

/// Historique annuler/rétablir illimité.
#[derive(Debug, Default)]
pub struct History {
    undo: Vec<Command>,
    redo: Vec<Command>,
    revision: u64,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    /// Compteur incrémenté à chaque modification de la page (ajout, suppression, annuler, rétablir).
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn add_item(&mut self, page: &mut Page, item: Item) {
        self.add_items(page, vec![item]);
    }

    /// Ajoute des éléments au-dessus des autres, en une seule étape d'historique.
    pub fn add_items(&mut self, page: &mut Page, items: Vec<Item>) {
        if items.is_empty() {
            return;
        }
        let start = page.items.len();
        let added = items
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, item)| (start + i, item))
            .collect();
        page.items.extend(items);
        self.record(Command {
            removed: Vec::new(),
            added,
        });
    }

    /// Supprime les éléments dont l'identifiant est donné ; ignore les identifiants inconnus.
    pub fn remove_items(&mut self, page: &mut Page, ids: &[Uuid]) {
        let replacements = ids.iter().map(|id| (*id, Vec::new())).collect();
        self.replace_items(page, replacements);
    }

    /// Remplace chaque élément désigné, sur place, par zéro, un ou plusieurs éléments
    /// (suppression, transformation, découpe), en une seule étape d'historique.
    /// Sans effet, et sans étape d'historique, si aucun identifiant ne correspond.
    pub fn replace_items(&mut self, page: &mut Page, mut replacements: HashMap<Uuid, Vec<Item>>) {
        let mut removed = Vec::new();
        let mut added = Vec::new();
        let mut next = Vec::with_capacity(page.items.len());
        for (index, item) in std::mem::take(&mut page.items).into_iter().enumerate() {
            match replacements.remove(&item.id()) {
                Some(new_items) => {
                    removed.push((index, item));
                    for new_item in new_items {
                        added.push((next.len(), new_item.clone()));
                        next.push(new_item);
                    }
                }
                None => next.push(item),
            }
        }
        page.items = next;
        if !removed.is_empty() {
            self.record(Command { removed, added });
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self, page: &mut Page) -> bool {
        let Some(cmd) = self.undo.pop() else {
            return false;
        };
        apply(page, &cmd.added, &cmd.removed);
        self.redo.push(cmd);
        self.revision += 1;
        true
    }

    pub fn redo(&mut self, page: &mut Page) -> bool {
        let Some(cmd) = self.redo.pop() else {
            return false;
        };
        apply(page, &cmd.removed, &cmd.added);
        self.undo.push(cmd);
        self.revision += 1;
        true
    }

    fn record(&mut self, cmd: Command) {
        self.undo.push(cmd);
        self.redo.clear();
        self.revision += 1;
    }
}

/// Retire `to_remove` (index de l'état courant, en partant de la fin) puis insère `to_add`
/// (index de l'état final, dans l'ordre croissant).
fn apply(page: &mut Page, to_remove: &[(usize, Item)], to_add: &[(usize, Item)]) {
    for (index, _) in to_remove.iter().rev() {
        if *index < page.items.len() {
            page.items.remove(*index);
        }
    }
    for (index, item) in to_add {
        page.items
            .insert((*index).min(page.items.len()), item.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stroke::{Brush, Stroke};

    fn item() -> Item {
        Item::Stroke(Stroke::new(Brush::default()))
    }

    #[test]
    fn undo_redo_add_and_remove_roundtrip() {
        let (mut page, mut h) = (Page::default(), History::new());
        let items: Vec<Item> = (0..3).map(|_| item()).collect();
        for i in &items {
            h.add_item(&mut page, i.clone());
        }
        h.remove_items(&mut page, &[items[0].id(), items[2].id()]);
        assert_eq!(page.items, vec![items[1].clone()]);

        assert!(h.undo(&mut page));
        assert_eq!(page.items, items);
        assert!(h.redo(&mut page));
        assert_eq!(page.items, vec![items[1].clone()]);
    }

    #[test]
    fn replace_can_transform_and_split_in_place() {
        let (mut page, mut h) = (Page::default(), History::new());
        let items: Vec<Item> = (0..3).map(|_| item()).collect();
        h.add_items(&mut page, items.clone());
        let (a, b) = (item(), item());
        let mut map = HashMap::new();
        map.insert(items[1].id(), vec![a.clone(), b.clone()]);
        h.replace_items(&mut page, map);
        assert_eq!(
            page.items,
            vec![items[0].clone(), a.clone(), b.clone(), items[2].clone()]
        );

        h.undo(&mut page);
        assert_eq!(page.items, items);
        h.redo(&mut page);
        assert_eq!(page.items.len(), 4);
        assert_eq!(page.items[1], a);
    }

    #[test]
    fn noop_operations_leave_no_history_entry() {
        let (mut page, mut h) = (Page::default(), History::new());
        h.add_items(&mut page, Vec::new());
        h.remove_items(&mut page, &[Uuid::new_v4()]);
        assert!(!h.can_undo());
        assert_eq!(h.revision(), 0);
    }

    #[test]
    fn new_action_clears_redo_and_revision_grows() {
        let (mut page, mut h) = (Page::default(), History::new());
        h.add_item(&mut page, item());
        h.undo(&mut page);
        assert!(h.can_redo());
        h.add_item(&mut page, item());
        assert!(!h.can_redo());
        assert_eq!(h.revision(), 3);
        assert!(!History::new().undo(&mut Page::default()));
    }
}
