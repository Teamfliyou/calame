//! Miniatures de pages : le haut de la page, réduit, pour le sommaire, la bibliothèque et la
//! palette de recherche.
//!
//! Une miniature garde des traits allégés (un point tous les quelques millimètres), recalculés
//! seulement quand la page change ; les pages sont lues dans la base à la première demande.

use std::collections::HashMap;
use std::rc::Rc;

use calame_core::background::PAGE_WIDTH;
use calame_core::{AssetId, Background, BrushKind, Item, NodeId, Page, Point2, UNITS_PER_MM};
use calame_storage::Store;
use egui::{pos2, vec2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke};

use crate::theme;

/// Distance minimale entre deux points gardés, en unités de page.
const DECIMATION: f32 = 4.0;
/// Largeur maximale des textures d'images de fond réduites, en pixels.
const TEXTURE_WIDTH: u32 = 240;

struct ThumbStroke {
    points: Vec<Point2>,
    color: Color32,
    width: f32,
}

struct ThumbText {
    pos: Point2,
    text: String,
    size: f32,
    color: Color32,
}

pub struct Thumb {
    strokes: Vec<ThumbStroke>,
    texts: Vec<ThumbText>,
    background: Background,
    /// Taille de l'image de fond, en unités de page.
    image_size: Option<(f32, f32)>,
}

impl Thumb {
    fn new(page: &Page, background: Background, image_size: Option<(f32, f32)>) -> Self {
        let mut strokes = Vec::new();
        let mut texts = Vec::new();
        for item in &page.items {
            match item {
                Item::Stroke(s) => {
                    let mut points: Vec<Point2> = Vec::new();
                    for p in &s.points {
                        if points
                            .last()
                            .map_or(true, |last| last.distance(p.pos) >= DECIMATION)
                        {
                            points.push(p.pos);
                        }
                    }
                    if let (Some(last), Some(end)) = (points.last(), s.points.last()) {
                        if *last != end.pos {
                            points.push(end.pos);
                        }
                    }
                    let c = s.brush.color;
                    let alpha = if s.brush.kind == BrushKind::Highlighter {
                        120
                    } else {
                        c.a
                    };
                    strokes.push(ThumbStroke {
                        points,
                        color: Color32::from_rgba_unmultiplied(c.r, c.g, c.b, alpha),
                        width: s.brush.width,
                    });
                }
                Item::Text(t) => texts.push(ThumbText {
                    pos: t.pos,
                    text: t.text.clone(),
                    size: t.font_size,
                    color: Color32::from_rgba_unmultiplied(
                        t.color.r, t.color.g, t.color.b, t.color.a,
                    ),
                }),
            }
        }
        Self {
            strokes,
            texts,
            background,
            image_size,
        }
    }

    /// Largeur de page montrée : la page A4, ou l'image de fond si elle est plus large.
    fn width(&self) -> f32 {
        self.image_size.map_or(PAGE_WIDTH, |(w, _)| w.max(1.0))
    }
}

/// Miniatures déjà calculées, et textures réduites des images de fond.
#[derive(Default)]
pub struct Thumbs {
    pages: HashMap<NodeId, (i64, Rc<Thumb>)>,
    textures: HashMap<AssetId, egui::TextureHandle>,
}

impl Thumbs {
    pub fn clear(&mut self) {
        self.pages.clear();
        self.textures.clear();
    }

    fn texture(
        &mut self,
        ctx: &egui::Context,
        store: &Store,
        asset: AssetId,
    ) -> Option<egui::TextureId> {
        if let Some(handle) = self.textures.get(&asset) {
            return Some(handle.id());
        }
        let data = store.asset(asset).ok()?;
        let decoded = image::load_from_memory(&data.data).ok()?;
        let small = decoded
            .thumbnail(TEXTURE_WIDTH, TEXTURE_WIDTH * 2)
            .to_rgba8();
        let size = [small.width() as usize, small.height() as usize];
        let image = egui::ColorImage::from_rgba_unmultiplied(size, small.as_raw());
        let handle = ctx.load_texture(
            format!("miniature-{asset}"),
            image,
            egui::TextureOptions::LINEAR,
        );
        let id = handle.id();
        self.textures.insert(asset, handle);
        Some(id)
    }
}

/// Ce qu'il faut pour dessiner une miniature : la base, le cache, les dates de modification et la
/// page ouverte (dont le contenu n'est peut-être pas encore enregistré).
pub struct ThumbSource<'a> {
    pub store: &'a Store,
    pub cache: &'a mut Thumbs,
    pub stamps: &'a HashMap<NodeId, i64>,
    /// Page ouverte : identifiant, contenu, révision de l'historique et fond.
    pub open: Option<(NodeId, &'a Page, u64, Background)>,
}

impl ThumbSource<'_> {
    fn thumb(&mut self, id: NodeId) -> Rc<Thumb> {
        // La page ouverte change sans être enregistrée : sa révision sert de date (négative, pour
        // ne jamais se confondre avec une date de la base).
        let (stamp, live) = match self.open {
            Some((open, page, revision, background)) if open == id => {
                (-(revision as i64) - 1, Some((page, background)))
            }
            _ => (self.stamps.get(&id).copied().unwrap_or(0), None),
        };
        if let Some((cached, thumb)) = self.cache.pages.get(&id) {
            if *cached == stamp {
                return thumb.clone();
            }
        }
        let (page, background) = match live {
            Some((page, background)) => (page.clone(), background),
            None => (
                self.store.load_page(id).unwrap_or_default(),
                self.store.background(id).unwrap_or_default(),
            ),
        };
        let image_size = background
            .image()
            .and_then(|asset| self.store.asset(asset).ok())
            .map(|a| (a.width, a.height));
        let thumb = Rc::new(Thumb::new(&page, background, image_size));
        self.cache.pages.insert(id, (stamp, thumb.clone()));
        thumb
    }

    /// Peint le haut de la page `id` dans `rect` (papier, fond, traits et textes).
    pub fn paint(
        &mut self,
        ctx: &egui::Context,
        painter: &Painter,
        rect: Rect,
        id: NodeId,
        radius: f32,
        dark: bool,
    ) {
        let thumb = self.thumb(id);
        let texture = thumb
            .background
            .image()
            .and_then(|asset| self.cache.texture(ctx, self.store, asset));
        paint(painter, rect, &thumb, texture, radius, dark);
    }
}

fn paint(
    painter: &Painter,
    rect: Rect,
    thumb: &Thumb,
    texture: Option<egui::TextureId>,
    radius: f32,
    dark: bool,
) {
    painter.rect_filled(rect, radius, theme::paper(dark));
    let painter = painter.with_clip_rect(rect.shrink(0.5).intersect(painter.clip_rect()));
    let k = rect.width() / thumb.width();
    let to_screen = |p: Point2| rect.min + vec2(p.x, p.y) * k;
    if let (Some(texture), Some((w, h))) = (texture, thumb.image_size) {
        let image = Rect::from_min_size(rect.min, vec2(w, h) * k);
        let tint = if dark {
            Color32::from_gray(200)
        } else {
            Color32::WHITE
        };
        painter.image(
            texture,
            image,
            Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
            tint,
        );
    } else {
        paint_pattern(&painter, rect, thumb.background, k, dark);
    }
    for stroke in &thumb.strokes {
        let color = theme::display_ink(stroke.color, dark);
        let width = (stroke.width * k).max(0.7);
        match stroke.points.as_slice() {
            [] => {}
            [only] => {
                painter.circle_filled(to_screen(*only), width / 2.0, color);
            }
            points => {
                let pts: Vec<Pos2> = points.iter().map(|p| to_screen(*p)).collect();
                painter.add(Shape::line(pts, Stroke::new(width, color)));
            }
        }
    }
    for text in &thumb.texts {
        let size = text.size * k;
        if size >= 3.0 {
            painter.text(
                to_screen(text.pos),
                egui::Align2::LEFT_TOP,
                &text.text,
                FontId::proportional(size),
                theme::display_ink(text.color, dark),
            );
        }
    }
}

/// Motif du papier, à pas réel s'il reste lisible, sinon à pas décoratif.
fn paint_pattern(painter: &Painter, rect: Rect, background: Background, k: f32, dark: bool) {
    let color = theme::paper_grid(dark);
    let stroke = Stroke::new(1.0, color);
    let step = |mm: f32| (mm * UNITS_PER_MM * k).max(5.0);
    let lines = |painter: &Painter, s: f32, vertical: bool| {
        let mut y = rect.top() + s;
        while y < rect.bottom() {
            painter.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], stroke);
            y += s;
        }
        if vertical {
            let mut x = rect.left() + s;
            while x < rect.right() {
                painter.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], stroke);
                x += s;
            }
        }
    };
    match background {
        Background::GridSmall => lines(painter, step(5.0), true),
        Background::GridLarge => lines(painter, step(8.0), true),
        Background::Lines => lines(painter, step(8.0), false),
        Background::Dots | Background::Isometric => {
            let s = step(5.0);
            let mut y = rect.top() + s;
            while y < rect.bottom() {
                let mut x = rect.left() + s;
                while x < rect.right() {
                    painter.circle_filled(pos2(x, y), 0.7, color);
                    x += s;
                }
                y += s;
            }
        }
        Background::Blank | Background::Image(_) => {}
    }
}

/// Aperçu d'un papier (feuille de création d'un cahier) : le motif seul, à pas décoratif.
pub fn paint_paper(painter: &Painter, rect: Rect, background: Background, radius: f32, dark: bool) {
    painter.rect_filled(rect, radius, theme::paper(dark));
    let clipped = painter.with_clip_rect(rect.shrink(1.0).intersect(painter.clip_rect()));
    paint_pattern(
        &clipped,
        rect,
        background,
        rect.width() / PAGE_WIDTH * 1.6,
        dark,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use calame_core::{Brush, Color, Stroke as PageStroke, StrokePoint};

    #[test]
    fn strokes_are_decimated_but_keep_their_ends() {
        let points: Vec<StrokePoint> = (0..=100)
            .map(|i| StrokePoint {
                pos: Point2::new(i as f32 * 0.5, 0.0),
                pressure: 0.5,
                tilt: None,
            })
            .collect();
        let mut stroke = PageStroke::new(Brush {
            kind: BrushKind::Pen,
            color: Color::BLACK,
            width: 2.0,
        });
        stroke.points = points;
        let page = Page {
            items: vec![Item::Stroke(stroke)],
        };
        let thumb = Thumb::new(&page, Background::Blank, None);
        let kept = &thumb.strokes[0].points;
        assert!(kept.len() < 20, "{} points gardés", kept.len());
        assert_eq!(kept.first().map(|p| p.x), Some(0.0));
        assert_eq!(kept.last().map(|p| p.x), Some(50.0));
    }
}
