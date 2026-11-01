//! Cache de rendu : lissage et rectangle englobant de chaque trait, calculés une seule fois.
//!
//! Sans lui, chaque image recalculerait le lissage de tous les traits de la page, même hors de
//! l'écran. Une entrée reste valable tant que l'empreinte du trait ne change pas.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use calame_core::{Item, Page, Rect2, Stroke, StrokePoint};
use uuid::Uuid;

/// Points ajoutés par segment lors du lissage affiché.
pub const SMOOTHING: usize = 6;

/// Trait prêt à dessiner.
pub struct StrokeRender {
    pub smoothed: Vec<StrokePoint>,
    pub bounds: Option<Rect2>,
}

/// Ce qui identifie la géométrie d'un trait : nombre de points, trois points repères et épaisseur.
/// Un déplacement, une rotation ou un redimensionnement (qui gardent l'identifiant) la changent.
type Fingerprint = (usize, [u32; 6], u32);

fn fingerprint(stroke: &Stroke) -> Fingerprint {
    let pts = &stroke.points;
    let bits = |i: usize| {
        pts.get(i)
            .map_or([0, 0], |p| [p.pos.x.to_bits(), p.pos.y.to_bits()])
    };
    let (a, b, c) = (
        bits(0),
        bits(pts.len() / 2),
        bits(pts.len().saturating_sub(1)),
    );
    (
        pts.len(),
        [a[0], a[1], b[0], b[1], c[0], c[1]],
        stroke.brush.width.to_bits(),
    )
}

#[derive(Default)]
pub struct RenderCache {
    entries: RefCell<HashMap<Uuid, (Fingerprint, Rc<StrokeRender>)>>,
}

impl RenderCache {
    /// Rendu d'un trait, calculé au premier appel puis réutilisé.
    pub fn get(&self, stroke: &Stroke) -> Rc<StrokeRender> {
        let print = fingerprint(stroke);
        let mut entries = self.entries.borrow_mut();
        if let Some((cached, render)) = entries.get(&stroke.id) {
            if *cached == print {
                return Rc::clone(render);
            }
        }
        let render = Rc::new(StrokeRender {
            smoothed: stroke.smoothed(SMOOTHING),
            bounds: stroke.bounds(),
        });
        entries.insert(stroke.id, (print, Rc::clone(&render)));
        render
    }

    /// Rectangle englobant d'un élément ; pour un trait, celui du cache.
    pub fn bounds(&self, item: &Item) -> Option<Rect2> {
        match item {
            Item::Stroke(s) => self.get(s).bounds,
            Item::Text(_) => item.bounds(),
        }
    }

    /// Oublie les traits qui ne sont plus sur la page (supprimés, remplacés, annulés), mais
    /// seulement quand ils deviennent nombreux : le nettoyage coûte un passage sur la page.
    pub fn prune(&self, page: &Page) {
        let mut entries = self.entries.borrow_mut();
        if entries.len() > page.items.len() * 2 + 64 {
            let alive: std::collections::HashSet<Uuid> = page.items.iter().map(Item::id).collect();
            entries.retain(|id, _| alive.contains(id));
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.borrow().len()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use calame_core::{Brush, Point2, Similarity};

    use super::*;

    fn stroke(points: &[(f32, f32)]) -> Stroke {
        let mut s = Stroke::new(Brush::default());
        for &(x, y) in points {
            s.push(StrokePoint {
                pos: Point2::new(x, y),
                pressure: 0.5,
                tilt: None,
            });
        }
        s
    }

    #[test]
    fn a_stroke_is_smoothed_once_and_reused() {
        let cache = RenderCache::default();
        let s = stroke(&[(0.0, 0.0), (10.0, 5.0), (20.0, 0.0), (30.0, 5.0)]);
        let first = cache.get(&s);
        let second = cache.get(&s);
        assert!(Rc::ptr_eq(&first, &second));
        assert_eq!(first.smoothed.len(), 3 * SMOOTHING + 1);
        assert!(first.bounds.is_some());
    }

    #[test]
    fn moving_a_stroke_with_the_same_id_invalidates_its_entry() {
        let cache = RenderCache::default();
        let s = stroke(&[(0.0, 0.0), (10.0, 5.0), (20.0, 0.0)]);
        let before = cache.get(&s);
        let moved = s.transformed(&Similarity::translation(100.0, 0.0));
        assert_eq!(moved.id, s.id);
        let after = cache.get(&moved);
        assert!(!Rc::ptr_eq(&before, &after));
        let (b, a) = (
            before.bounds.expect("bornes"),
            after.bounds.expect("bornes"),
        );
        assert!((a.min.x - b.min.x - 100.0).abs() < 1e-3);
        // Rescaling changes the width as well, even for a degenerate stroke.
        let scaled = s.transformed(&Similarity::scaling(Point2::new(0.0, 0.0), 2.0));
        assert!(!Rc::ptr_eq(&cache.get(&scaled), &after));
    }

    #[test]
    fn prune_forgets_strokes_that_left_the_page_only_when_they_pile_up() {
        let cache = RenderCache::default();
        let keep = stroke(&[(0.0, 0.0), (1.0, 1.0)]);
        let page = Page {
            items: vec![Item::Stroke(keep.clone())],
        };
        cache.get(&keep);
        for i in 0..100 {
            cache.get(&stroke(&[(0.0, 0.0), (i as f32 + 1.0, 1.0)]));
        }
        assert_eq!(cache.len(), 101);
        cache.prune(&page);
        assert_eq!(cache.len(), 1);
        // Peu d'entrées en trop : on ne s'embête pas à nettoyer.
        cache.get(&stroke(&[(5.0, 5.0), (6.0, 6.0)]));
        cache.prune(&page);
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn texts_use_their_own_bounds() {
        let cache = RenderCache::default();
        let text = Item::Text(calame_core::TextBox::new(
            Point2::new(5.0, 5.0),
            "x".into(),
            10.0,
            calame_core::Color::BLACK,
        ));
        assert_eq!(cache.bounds(&text), text.bounds());
        assert_eq!(cache.len(), 0);
    }
}

/// Mesures sur de grandes pages : `cargo test --release -p calame-app -- --ignored --nocapture`.
#[cfg(test)]
pub(crate) mod perf {
    #![allow(clippy::expect_used)]

    use std::time::Instant;

    use calame_core::{Brush, Page, Point2, Rect2, StrokePoint};

    use super::*;

    /// Page de `strokes` traits de `points` points chacun, répartis sur une grande surface.
    pub(crate) fn big_page(strokes: usize, points: usize) -> Page {
        let mut seed = 12345_u64;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((seed >> 33) as f32 / (1u64 << 31) as f32) * 2.0 - 1.0
        };
        let items = (0..strokes)
            .map(|i| {
                let mut s = Stroke::new(Brush::default());
                let (mut x, mut y) = ((i % 100) as f32 * 80.0, (i / 100) as f32 * 80.0);
                for _ in 0..points {
                    x += 2.0 + 3.0 * next();
                    y += 3.0 * next();
                    s.push(StrokePoint {
                        pos: Point2::new(x, y),
                        pressure: 0.5 + 0.4 * next(),
                        tilt: None,
                    });
                }
                Item::Stroke(s)
            })
            .collect();
        Page { items }
    }

    #[test]
    #[ignore = "mesure de performance"]
    fn large_page_cache_and_culling() {
        let page = big_page(5000, 200);
        let total_points: usize = page
            .items
            .iter()
            .map(|i| match i {
                Item::Stroke(s) => s.points.len(),
                Item::Text(_) => 0,
            })
            .sum();
        let cache = RenderCache::default();

        let start = Instant::now();
        for item in &page.items {
            cache.bounds(item);
        }
        let cold = start.elapsed();
        let start = Instant::now();
        for item in &page.items {
            cache.bounds(item);
        }
        let warm = start.elapsed();

        // Un écran typique (1600×900 unités) sur cette page de 8000×4000.
        let view = Rect2 {
            min: Point2::new(2000.0, 1000.0),
            max: Point2::new(3600.0, 1900.0),
        };
        let start = Instant::now();
        let visible = page
            .items
            .iter()
            .filter(|i| cache.bounds(i).is_some_and(|b| b.intersects(view)))
            .count();
        let culling = start.elapsed();

        eprintln!(
            "PERF page de {} traits, {total_points} points : cache à froid {cold:?}, à chaud {warm:?} ; \
             sélection des traits visibles {culling:?} ({visible} sur {})",
            page.items.len(),
            page.items.len()
        );
        assert!(
            visible < page.items.len() / 4,
            "le masquage doit écarter l'essentiel"
        );
        assert!(
            warm * 5 < cold,
            "le cache doit rendre le second passage bien plus rapide"
        );
    }
}
