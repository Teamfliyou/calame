//! PDF vectoriel multipage à partir de pages SVG.

use std::collections::HashMap;

use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref};
use svg2pdf::ConversionOptions;

use crate::raster::parse;
use crate::{ExportError, Result};

/// Une unité de page (1/96 pouce) vaut 0,75 point PDF (1/72 pouce).
const POINTS_PER_UNIT: f32 = 0.75;

/// Assemble un PDF dont chaque page est l'un des SVG, à sa taille.
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

        let (page_id, content_id) = (alloc.bump(), alloc.bump());
        let size = tree.size();
        let (w, h) = (
            size.width() * POINTS_PER_UNIT,
            size.height() * POINTS_PER_UNIT,
        );
        let svg_name = Name(b"S0");
        let mut page = pdf.page(page_id);
        page.media_box(Rect::new(0.0, 0.0, w, h));
        page.parent(page_tree_id);
        page.contents(content_id);
        page.resources().x_objects().pair(svg_name, svg_id);
        page.finish();

        // Le graphique mesure un point sur un point : on l'étire à la taille de la page.
        let mut content = Content::new();
        content.transform([w, 0.0, 0.0, h, 0.0, 0.0]);
        content.x_object(svg_name);
        pdf.stream(content_id, &content.finish());
        pdf.extend(&chunk);
        page_ids.push(page_id);
    }

    pdf.catalog(catalog_id).pages(page_tree_id);
    pdf.pages(page_tree_id)
        .kids(page_ids.iter().copied())
        .count(page_ids.len() as i32);
    Ok(pdf.finish())
}
