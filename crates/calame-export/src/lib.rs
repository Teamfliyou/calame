//! Export des pages en SVG, PNG et PDF, et import de PDF et d'images comme fonds de page.
//!
//! Tout est fait en Rust, sans programme externe : la page est d'abord décrite en SVG, puis
//! rastérisée en PNG (`resvg`) ou convertie en PDF vectoriel (`svg2pdf`). L'import d'un PDF
//! rend chaque page en image (`hayro`) pour pouvoir l'annoter par-dessus.

mod import;
mod pdf;
mod raster;
mod svg;

use calame_core::Background;
use thiserror::Error;

pub use import::{import_image, render_pdf_pages, ImportedImage, RenderedPage};
pub use pdf::svgs_to_pdf;
pub use raster::svg_to_png;
pub use svg::page_svg;

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("SVG invalide : {0}")]
    Svg(String),
    #[error("échec du rendu : {0}")]
    Render(String),
    #[error("échec de la création du PDF : {0}")]
    Pdf(String),
    #[error("import impossible : {0}")]
    Import(String),
}

pub type Result<T> = std::result::Result<T, ExportError>;

/// Image de fond avec son contenu, prête à être dessinée ou exportée.
#[derive(Clone, Debug, PartialEq)]
pub struct BackgroundImage {
    /// `image/png` ou `image/jpeg`.
    pub mime: String,
    pub data: Vec<u8>,
    /// Taille en unités de page.
    pub width: f32,
    pub height: f32,
}

/// Fond d'une page à exporter.
#[derive(Clone, Debug, PartialEq)]
pub enum PageBackground {
    /// Fond uni ou motif : les images ne passent pas par ici (voir `Image`).
    Pattern(Background),
    Image(BackgroundImage),
}

/// Pixels par unité de page à l'export PNG (2 : environ 192 points par pouce).
pub const PNG_SCALE: f32 = 2.0;

#[cfg(test)]
mod tests;
