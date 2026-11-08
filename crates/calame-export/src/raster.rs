//! Rastérisation d'un SVG en PNG.

use std::sync::{Arc, OnceLock};

use calame_core::Item;
use resvg::tiny_skia::{Color, Pixmap, Transform};
use resvg::usvg::{fontdb, Options, Tree};

use crate::svg::items_svg;
use crate::{ExportError, PageImages, Result, PNG_SCALE};

/// Dimension maximale d'une image exportée, en pixels (au-delà, la mémoire explose).
const MAX_SIDE: f32 = 16_384.0;
const MAX_PIXELS: f32 = 120_000_000.0;
/// Dimension maximale d'une image copiée dans le presse-papiers, en pixels.
const MAX_COPY_SIDE: f32 = 4096.0;

/// Polices du système, chargées une seule fois (c'est lent).
pub(crate) fn fonts() -> Arc<fontdb::Database> {
    static FONTS: OnceLock<Arc<fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = fontdb::Database::new();
            db.load_system_fonts();
            Arc::new(db)
        })
        .clone()
}

pub(crate) fn parse(svg: &str) -> Result<Tree> {
    let options = Options {
        fontdb: fonts(),
        ..Options::default()
    };
    Tree::from_str(svg, &options).map_err(|e| ExportError::Svg(e.to_string()))
}

/// Pixmap vide à la taille du dessin, si cette taille est raisonnable.
fn blank_pixmap(tree: &Tree, scale: f32) -> Result<Pixmap> {
    let size = tree.size();
    let (w, h) = (size.width() * scale, size.height() * scale);
    if !scale.is_finite() || scale <= 0.0 || w < 1.0 || h < 1.0 {
        return Err(ExportError::Render("dimensions nulles".into()));
    }
    if w > MAX_SIDE || h > MAX_SIDE || w * h > MAX_PIXELS {
        return Err(ExportError::Render(format!(
            "image trop grande ({}×{} pixels)",
            w.ceil(),
            h.ceil()
        )));
    }
    Pixmap::new(w.ceil() as u32, h.ceil() as u32)
        .ok_or_else(|| ExportError::Render("mémoire insuffisante".into()))
}

/// Rend le SVG en PNG avec `scale` pixels par unité.
pub fn svg_to_png(svg: &str, scale: f32) -> Result<Vec<u8>> {
    let tree = parse(svg)?;
    let mut pixmap = blank_pixmap(&tree, scale)?;
    resvg::render(
        &tree,
        Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap
        .encode_png()
        .map_err(|e| ExportError::Render(e.to_string()))
}

/// Image en mémoire, en pixels RGBA tous opaques.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Rend des éléments seuls, sur fond blanc, pour le presse-papiers du système. `None` s'ils
/// n'occupent aucune place.
pub fn items_to_bitmap(items: &[Item], images: &PageImages) -> Result<Option<Bitmap>> {
    let Some(svg) = items_svg(items, images) else {
        return Ok(None);
    };
    let tree = parse(&svg)?;
    let size = tree.size();
    let scale = PNG_SCALE.min(MAX_COPY_SIDE / size.width().max(size.height()));
    let mut pixmap = blank_pixmap(&tree, scale)?;
    pixmap.fill(Color::WHITE);
    resvg::render(
        &tree,
        Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Ok(Some(Bitmap {
        width: pixmap.width(),
        height: pixmap.height(),
        rgba: pixmap.take(),
    }))
}
