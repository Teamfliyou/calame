use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::item::Item;

/// Une page : une liste ordonnée d'éléments (le dernier est dessiné au-dessus).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Page {
    pub items: Vec<Item>,
}

impl Page {
    pub fn item(&self, id: Uuid) -> Option<&Item> {
        self.items.iter().find(|i| i.id() == id)
    }
}
