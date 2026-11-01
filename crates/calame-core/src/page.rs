use serde::{Deserialize, Serialize};

use crate::stroke::Stroke;

/// Une page : une liste ordonnée de traits (le dernier est dessiné au-dessus).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Page {
    pub strokes: Vec<Stroke>,
}
