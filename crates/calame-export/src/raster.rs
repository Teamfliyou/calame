//! Rastérisation d'un SVG en PNG.

use std::sync::{Arc, OnceLock};

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{fontdb, Options, Tree};

use crate::{ExportError, Result};

/// Dimension maximale d'une image exportée, en pixels (au-delà, la mémoire explose).
const MAX_SIDE: f32 = 16_384.0;
const MAX_PIXELS: f32 = 120_000_000.0;

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

/// Rend le SVG en PNG avec `scale` pixels par unité.
pub fn svg_to_png(svg: &str, scale: f32) -> Result<Vec<u8>> {
    let tree = parse(svg)?;
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
    let mut pixmap = Pixmap::new(w.ceil() as u32, h.ceil() as u32)
        .ok_or_else(|| ExportError::Render("mémoire insuffisante".into()))?;
    resvg::render(
        &tree,
        Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap
        .encode_png()
        .map_err(|e| ExportError::Render(e.to_string()))
}
