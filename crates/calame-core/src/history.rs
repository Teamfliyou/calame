use crate::page::Page;
use crate::stroke::Stroke;

/// Opération réversible appliquée à une page.
#[derive(Clone, Debug)]
enum Command {
    AddStroke(Stroke),
    /// Traits supprimés avec leur index d'origine (ordre croissant).
    RemoveStrokes(Vec<(usize, Stroke)>),
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

    pub fn add_stroke(&mut self, page: &mut Page, stroke: Stroke) {
        page.strokes.push(stroke.clone());
        self.record(Command::AddStroke(stroke));
    }

    /// Supprime les traits dont l'index est donné ; ignore les index invalides.
    pub fn remove_strokes(&mut self, page: &mut Page, indices: &[usize]) {
        let mut sorted: Vec<usize> = indices
            .iter()
            .copied()
            .filter(|&i| i < page.strokes.len())
            .collect();
        sorted.sort_unstable();
        sorted.dedup();
        if sorted.is_empty() {
            return;
        }
        let mut removed = Vec::with_capacity(sorted.len());
        for &i in sorted.iter().rev() {
            removed.push((i, page.strokes.remove(i)));
        }
        removed.reverse();
        self.record(Command::RemoveStrokes(removed));
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
        match &cmd {
            Command::AddStroke(_) => {
                page.strokes.pop();
            }
            Command::RemoveStrokes(items) => {
                for (i, s) in items {
                    page.strokes.insert((*i).min(page.strokes.len()), s.clone());
                }
            }
        }
        self.redo.push(cmd);
        self.revision += 1;
        true
    }

    pub fn redo(&mut self, page: &mut Page) -> bool {
        let Some(cmd) = self.redo.pop() else {
            return false;
        };
        match &cmd {
            Command::AddStroke(s) => page.strokes.push(s.clone()),
            Command::RemoveStrokes(items) => {
                for (i, _) in items.iter().rev() {
                    if *i < page.strokes.len() {
                        page.strokes.remove(*i);
                    }
                }
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stroke::Brush;

    #[test]
    fn undo_redo_add_and_remove_roundtrip() {
        let (mut page, mut h) = (Page::default(), History::new());
        let strokes: Vec<Stroke> = (0..3).map(|_| Stroke::new(Brush::default())).collect();
        for s in &strokes {
            h.add_stroke(&mut page, s.clone());
        }
        h.remove_strokes(&mut page, &[0, 2]);
        assert_eq!(page.strokes, vec![strokes[1].clone()]);

        assert!(h.undo(&mut page));
        assert_eq!(page.strokes, strokes);
        assert!(h.redo(&mut page));
        assert_eq!(page.strokes, vec![strokes[1].clone()]);
    }

    #[test]
    fn new_action_clears_redo() {
        let (mut page, mut h) = (Page::default(), History::new());
        h.add_stroke(&mut page, Stroke::new(Brush::default()));
        h.undo(&mut page);
        assert!(h.can_redo());
        h.add_stroke(&mut page, Stroke::new(Brush::default()));
        assert!(!h.can_redo());
        assert!(!History::new().undo(&mut Page::default()));
    }
}
