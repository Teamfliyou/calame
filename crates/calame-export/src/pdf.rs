//! PDF vectoriel multipage à partir de pages SVG, découpées en feuilles A4.

use std::collections::HashMap;

use calame_core::background::{PAGE_HEIGHT, PAGE_WIDTH};
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref};
use svg2pdf::ConversionOptions;

use crate::raster::parse;
use crate::{ExportError, Result};

/// Une unité de page (1/96 pouce) vaut 0,75 point PDF (1/72 pouce).
const POINTS_PER_UNIT: f32 = 0.75;
/// Tolérance sur la taille d'une page, en unités (les arrondis du SVG).
const SIZE_TOLERANCE: f32 = 1.0;
/// Réduction maximale pour faire tenir une page en largeur : au-delà, l'écriture deviendrait
/// illisible, et la page est découpée en colonnes à sa taille réelle.
const MIN_FIT_SCALE: f32 = 0.5;

/// Découpage d'une page en feuilles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Sheets {
    /// Taille d'une feuille, en unités de page.
    pub width: f32,
    pub height: f32,
    /// Réduction appliquée à la page pour tenir sur les feuilles.
    pub scale: f32,
    pub columns: usize,
    pub rows: usize,
}

/// Feuilles nécessaires pour une page de `width` × `height` unités. Une page qui tient sur une
/// feuille A4 (dans un sens ou dans l'autre) garde sa taille ; les autres sont réduites à la largeur
/// de la feuille si possible, puis coupées en autant de feuilles A4 qu'il en faut.
pub(crate) fn sheets_for(width: f32, height: f32) -> Sheets {
    let fits = |w: f32, h: f32| width <= w + SIZE_TOLERANCE && height <= h + SIZE_TOLERANCE;
    if fits(PAGE_WIDTH, PAGE_HEIGHT) || fits(PAGE_HEIGHT, PAGE_WIDTH) {
        return Sheets {
            width,
            height,
            scale: 1.0,
            columns: 1,
            rows: 1,
        };
    }
    let (sheet_w, sheet_h) = if width > height {
        (PAGE_HEIGHT, PAGE_WIDTH)
    } else {
        (PAGE_WIDTH, PAGE_HEIGHT)
    };
    let fit = sheet_w / width;
    let scale = if fit >= MIN_FIT_SCALE {
        fit.min(1.0)
    } else {
        1.0
    };
    let count = |length: f32, sheet: f32| (length * scale / sheet - 1e-3).ceil().max(1.0) as usize;
    Sheets {
        width: sheet_w,
        height: sheet_h,
        scale,
        columns: count(width, sheet_w),
        rows: count(height, sheet_h),
    }
}

/// Assemble un PDF à partir des SVG : une feuille pour chacun, ou plusieurs s'il dépasse le format
/// A4 (voir [`sheets_for`]). Le dessin n'est écrit qu'une fois, partagé par ses feuilles.
pub fn svgs_to_pdf(svgs: &[String]) -> Result<Vec<u8>> {
    if svgs.is_empty() {
        return Err(ExportError::Pdf("aucune page à exporter".into()));
    }
    let mut alloc = Ref::new(1);
    let catalog_id = alloc.bump();
    let page_tree_id = alloc.bump();
    let mut pdf = Pdf::new();
    let mut page_ids = Vec::with_capacity(svgs.len());

    for svg in svgs {
        let tree = parse(svg)?;
        let (chunk, svg_ref) = svg2pdf::to_chunk(&tree, ConversionOptions::default())
            .map_err(|e| ExportError::Pdf(e.to_string()))?;
        // Les numéros d'objets du morceau sont renumérotés pour ne pas entrer en conflit.
        let mut renumbered = HashMap::new();
        let chunk = chunk.renumber(|old| *renumbered.entry(old).or_insert_with(|| alloc.bump()));
        let svg_id = *renumbered
            .get(&svg_ref)
            .ok_or_else(|| ExportError::Pdf("objet SVG manquant".into()))?;

        let size = tree.size();
        let sheets = sheets_for(size.width(), size.height());
        // Taille du dessin entier et d'une feuille, en points.
        let (w, h) = (
            size.width() * sheets.scale * POINTS_PER_UNIT,
            size.height() * sheets.scale * POINTS_PER_UNIT,
        );
        let (sheet_w, sheet_h) = (
            sheets.width * POINTS_PER_UNIT,
            sheets.height * POINTS_PER_UNIT,
        );
        let svg_name = Name(b"S0");
        for row in 0..sheets.rows {
            for column in 0..sheets.columns {
                let (page_id, content_id) = (alloc.bump(), alloc.bump());
                let mut page = pdf.page(page_id);
                page.media_box(Rect::new(0.0, 0.0, sheet_w, sheet_h));
                page.parent(page_tree_id);
                page.contents(content_id);
                page.resources().x_objects().pair(svg_name, svg_id);
                page.finish();

                // Le graphique mesure un point sur un point : on l'étire à la taille du dessin, puis
                // on le décale pour amener le morceau de cette feuille dans le cadre de la page.
                let mut content = Content::new();
                content.transform([
                    w,
                    0.0,
                    0.0,
                    h,
                    -(column as f32) * sheet_w,
                    (row + 1) as f32 * sheet_h - h,
                ]);
                content.x_object(svg_name);
                pdf.stream(content_id, &content.finish());
                page_ids.push(page_id);
            }
        }
        pdf.extend(&chunk);
    }

    pdf.catalog(catalog_id).pages(page_tree_id);
    pdf.pages(page_tree_id)
        .kids(page_ids.iter().copied())
        .count(page_ids.len() as i32);
    Ok(pdf.finish())
}
