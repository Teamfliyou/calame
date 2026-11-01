//! Modèle de données de Calame, indépendant de l'interface.

pub mod geometry;
pub mod history;
pub mod library;
pub mod page;
pub mod stroke;

pub use geometry::{Color, Point2};
pub use history::History;
pub use library::{LibraryError, LibraryTree, Node, NodeId, NodeKind};
pub use page::Page;
pub use stroke::{Brush, BrushKind, Stroke, StrokePoint};
