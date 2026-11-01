//! Modèle de données de Calame, indépendant de l'interface.

pub mod geometry;
pub mod history;
pub mod page;
pub mod stroke;

pub use geometry::{Color, Point2};
pub use history::History;
pub use page::Page;
pub use stroke::{Brush, BrushKind, Stroke, StrokePoint};
