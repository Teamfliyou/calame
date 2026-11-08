//! Import de PDF (rendu page par page) et d'images, pour servir de fond de page ou être posées
//! sur la page.

use std::io::Cursor;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

use calame_core::background::PAGE_WIDTH;
use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::{Pdf, PdfData};
use hayro::{render, RenderCache, RenderSettings};
use image::{imageops, ImageFormat, ImageReader, RgbaImage};

use crate::{ExportError, Result};

/// Nombre maximal de pages importées d'un PDF.
const MAX_PAGES: usize = 500;
/// Côté maximal d'une page rendue, en pixels.
const MAX_RENDER_SIDE: f32 = 6000.0;
/// Taille maximale d'une image importée : 40 Mo.
const MAX_IMAGE_BYTES: usize = 40 * 1024 * 1024;
/// Côté maximal d'une image collée, en pixels : au-delà, elle est réduite (elle doit aussi tenir
/// dans une texture de la carte graphique).
const MAX_PASTED_SIDE: u32 = 4096;
/// Une unité de page (1/96 pouce) fait 0,75 point (1/72 pouce).
const POINTS_PER_UNIT: f32 = 0.75;

/// Page de PDF rendue en image PNG.
#[derive(Clone, Debug)]
pub struct RenderedPage {
    pub png: Vec<u8>,
    /// Taille de la page en unités de page.
    pub width: f32,
    pub height: f32,
}

/// Image importée, avec son format d'origine conservé (PNG ou JPEG).
#[derive(Clone, Debug)]
pub struct ImportedImage {
    pub mime: String,
    pub data: Vec<u8>,
    /// Taille en unités de page.
    pub width: f32,
    pub height: f32,
}

/// Rend chaque page d'un PDF en PNG avec `pixels_per_unit` pixels par unité de page.
///
/// Un PDF protégé par mot de passe ou illisible donne une erreur ; une page qui fait échouer le
/// rendu est remplacée par une page blanche plutôt que d'interrompre tout l'import.
pub fn render_pdf_pages(bytes: Vec<u8>, pixels_per_unit: f32) -> Result<Vec<RenderedPage>> {
    let data: PdfData = Arc::new(bytes).into();
    let pdf = Pdf::new(data).map_err(|e| ExportError::Import(format!("PDF illisible : {e:?}")))?;
    let settings = InterpreterSettings::default();
    let cache = RenderCache::new();
    let mut out = Vec::new();
    for page in pdf.pages().iter().take(MAX_PAGES) {
        let (w_pt, h_pt) = page.render_dimensions();
        if !(w_pt > 0.0 && h_pt > 0.0) {
            continue;
        }
        let units = (w_pt / POINTS_PER_UNIT, h_pt / POINTS_PER_UNIT);
        // Pixels par point, limités pour qu'une page géante ne sature pas la mémoire.
        let scale = (pixels_per_unit / POINTS_PER_UNIT).min(MAX_RENDER_SIDE / w_pt.max(h_pt));
        let rendered = catch_unwind(AssertUnwindSafe(|| {
            render(
                page,
                &cache,
                &settings,
                &RenderSettings {
                    x_scale: scale,
                    y_scale: scale,
                    ..RenderSettings::default()
                },
            )
        }));
        let png = match rendered {
            Ok(pixmap) => {
                // Le rendu est transparent par endroits : on le pose sur du blanc.
                let rgba: Vec<u8> = pixmap
                    .data_as_u8_slice()
                    .chunks_exact(4)
                    .flat_map(|p| {
                        let missing = 255 - p[3];
                        [
                            p[0].saturating_add(missing),
                            p[1].saturating_add(missing),
                            p[2].saturating_add(missing),
                            255,
                        ]
                    })
                    .collect();
                encode_png(u32::from(pixmap.width()), u32::from(pixmap.height()), rgba)?
            }
            Err(_) => {
                let (w, h) = ((w_pt * scale).ceil() as u32, (h_pt * scale).ceil() as u32);
                encode_png(w, h, vec![255; (w * h * 4) as usize])?
            }
        };
        out.push(RenderedPage {
            png,
            width: units.0,
            height: units.1,
        });
    }
    if out.is_empty() {
        return Err(ExportError::Import("le PDF ne contient aucune page".into()));
    }
    Ok(out)
}

fn encode_png(width: u32, height: u32, rgba: Vec<u8>) -> Result<Vec<u8>> {
    let img = RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| ExportError::Import("dimensions d'image incohérentes".into()))?;
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
        .map_err(|e| ExportError::Import(e.to_string()))?;
    Ok(out)
}

/// Encode en PNG une image du presse-papiers (pixels RGBA) ; un pixel y vaut une unité de page.
pub fn pasted_image(width: u32, height: u32, rgba: Vec<u8>) -> Result<ImportedImage> {
    if width == 0 || height == 0 {
        return Err(ExportError::Import("image vide".into()));
    }
    let side = width.max(height);
    let data = if side > MAX_PASTED_SIDE {
        let image = RgbaImage::from_raw(width, height, rgba)
            .ok_or_else(|| ExportError::Import("dimensions d'image incohérentes".into()))?;
        let fit = |v: u32| (u64::from(v) * u64::from(MAX_PASTED_SIDE) / u64::from(side)).max(1);
        let small = imageops::thumbnail(&image, fit(width) as u32, fit(height) as u32);
        encode_png(small.width(), small.height(), small.into_raw())?
    } else {
        encode_png(width, height, rgba)?
    };
    Ok(ImportedImage {
        mime: "image/png".into(),
        data,
        width: width as f32,
        height: height as f32,
    })
}

/// Vérifie qu'une image PNG ou JPEG se lit et calcule sa taille en unités de page : la largeur
/// d'une page A4, à la même proportion que l'image.
pub fn import_image(bytes: Vec<u8>) -> Result<ImportedImage> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(ExportError::Import(
            "image trop volumineuse (40 Mo maximum)".into(),
        ));
    }
    let reader = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|e| ExportError::Import(e.to_string()))?;
    let mime = match reader.format() {
        Some(ImageFormat::Png) => "image/png",
        Some(ImageFormat::Jpeg) => "image/jpeg",
        _ => {
            return Err(ExportError::Import(
                "format non pris en charge (PNG ou JPEG)".into(),
            ))
        }
    };
    let (w, h) = reader
        .into_dimensions()
        .map_err(|e| ExportError::Import(e.to_string()))?;
    if w == 0 || h == 0 {
        return Err(ExportError::Import("image vide".into()));
    }
    Ok(ImportedImage {
        mime: mime.into(),
        data: bytes,
        width: PAGE_WIDTH,
        height: PAGE_WIDTH * h as f32 / w as f32,
    })
}
