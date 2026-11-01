//! Modèle de données de Calame, indépendant de l'interface.

pub mod geometry;
pub mod history;
pub mod instruments;
pub mod item;
pub mod library;
pub mod page;
pub mod stroke;

pub use geometry::{Color, Point2, Rect2, Similarity};
pub use history::History;
pub use instruments::{
    snap, Edge, Handle, Instrument, InstrumentKind, Pose, UNITS_PER_CM, UNITS_PER_MM,
};
pub use item::{Item, TextBox};
pub use library::{LibraryError, LibraryTree, Node, NodeId, NodeKind};
pub use page::Page;
pub use stroke::{Brush, BrushKind, Stroke, StrokePoint};
