//! Fonds de page : motifs (quadrillage, lignes, points, isométrique) ou image/PDF importé.

use std::fmt;
use std::str::FromStr;

use uuid::Uuid;

use crate::geometry::{Color, Point2, Rect2};
use crate::instruments::UNITS_PER_MM;
use crate::item::Item;

/// Largeur d'une page A4 en unités de page (96 points par pouce).
pub const PAGE_WIDTH: f32 = 793.7;
/// Hauteur d'une page A4 en unités de page.
pub const PAGE_HEIGHT: f32 = 1122.5;
/// Marge ajoutée autour du contenu qui dépasse la page, à l'export.
pub const EXPORT_MARGIN: f32 = 24.0;

/// Au-delà de ce nombre de lignes par famille, le motif est omis (vue très dézoomée).
const MAX_LINES_PER_FAMILY: usize = 4000;
const MAX_DOTS: usize = 40_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AssetId(Uuid);

impl AssetId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for AssetId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for AssetId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Background {
    #[default]
    Blank,
    /// Petits carreaux de 5 mm.
    GridSmall,
    /// Grands carreaux de 8 mm.
    GridLarge,
    /// Lignes horizontales de 8 mm.
    Lines,
    /// Points tous les 5 mm.
    Dots,
    /// Réseau triangulaire (trois familles de droites à 60°).
    Isometric,
    /// Image ou page de PDF importée, posée en haut à gauche de la page.
    Image(AssetId),
}

/// Fonds proposés dans l'interface (hors image importée).
pub const PATTERN_BACKGROUNDS: [Background; 6] = [
    Background::Blank,
    Background::GridSmall,
    Background::GridLarge,
    Background::Lines,
    Background::Dots,
    Background::Isometric,
];

impl Background {
    /// Représentation texte enregistrée dans la base.
    pub fn to_db(self) -> String {
        match self {
            Self::Blank => "blank".into(),
            Self::GridSmall => "grid-small".into(),
            Self::GridLarge => "grid-large".into(),
            Self::Lines => "lines".into(),
            Self::Dots => "dots".into(),
            Self::Isometric => "isometric".into(),
            Self::Image(id) => format!("asset:{id}"),
        }
    }

    pub fn from_db(text: &str) -> Option<Self> {
        Some(match text {
            "blank" => Self::Blank,
            "grid-small" => Self::GridSmall,
            "grid-large" => Self::GridLarge,
            "lines" => Self::Lines,
            "dots" => Self::Dots,
            "isometric" => Self::Isometric,
            other => Self::Image(other.strip_prefix("asset:")?.parse().ok()?),
        })
    }

    pub fn image(self) -> Option<AssetId> {
        match self {
            Self::Image(id) => Some(id),
            _ => None,
        }
    }

    /// Lignes et points du motif dans `area`. Vide pour un fond uni ou une image.
    pub fn pattern(self, area: Rect2) -> Pattern {
        const GRID: Color = Color::rgb(196, 212, 236);
        const RULED: Color = Color::rgb(176, 190, 222);
        let mut pattern = Pattern {
            lines: Vec::new(),
            dots: Vec::new(),
            color: GRID,
        };
        let mm = UNITS_PER_MM;
        match self {
            Self::Blank | Self::Image(_) => {}
            Self::GridSmall => {
                pattern.add_family(area, 0.0, 5.0 * mm);
                pattern.add_family(area, 90.0, 5.0 * mm);
            }
            Self::GridLarge => {
                pattern.add_family(area, 0.0, 8.0 * mm);
                pattern.add_family(area, 90.0, 8.0 * mm);
            }
            Self::Lines => {
                pattern.color = RULED;
                pattern.add_family(area, 0.0, 8.0 * mm);
            }
            Self::Dots => {
                pattern.color = Color::rgb(150, 165, 195);
                pattern.add_dots(area, 5.0 * mm);
            }
            Self::Isometric => {
                // Trois familles à 90°, 30° et -30° : des triangles équilatéraux.
                let spacing = 6.0 * mm;
                pattern.add_family(area, 90.0, spacing);
                pattern.add_family(area, 30.0, spacing);
                pattern.add_family(area, -30.0, spacing);
            }
        }
        pattern
    }
}

/// Motif de fond prêt à dessiner.
#[derive(Clone, Debug, PartialEq)]
pub struct Pattern {
    pub lines: Vec<(Point2, Point2)>,
    pub dots: Vec<Point2>,
    pub color: Color,
}

impl Pattern {
    /// Droites parallèles d'angle `degrees`, espacées de `spacing`, découpées à `area`.
    fn add_family(&mut self, area: Rect2, degrees: f32, spacing: f32) {
        let (sin, cos) = degrees.to_radians().sin_cos();
        // Les droites sont les ensembles `n · p = k * spacing`, `n` étant la normale.
        let normal = (-sin, cos);
        let offsets = [
            Point2::new(area.min.x, area.min.y),
            Point2::new(area.max.x, area.min.y),
            Point2::new(area.min.x, area.max.y),
            Point2::new(area.max.x, area.max.y),
        ]
        .map(|p| p.x * normal.0 + p.y * normal.1);
        let lo = offsets.iter().copied().fold(f32::INFINITY, f32::min);
        let hi = offsets.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let (first, last) = ((lo / spacing).ceil() as i64, (hi / spacing).floor() as i64);
        if last - first > MAX_LINES_PER_FAMILY as i64 {
            return;
        }
        for k in first..=last {
            let c = k as f32 * spacing;
            // Point de la droite le plus proche de l'origine, puis direction (cos, sin).
            let base = Point2::new(normal.0 * c, normal.1 * c);
            if let Some(segment) = clip_line(area, base, (cos, sin)) {
                self.lines.push(segment);
            }
        }
    }

    fn add_dots(&mut self, area: Rect2, spacing: f32) {
        let (x0, x1) = (
            (area.min.x / spacing).ceil() as i64,
            (area.max.x / spacing).floor() as i64,
        );
        let (y0, y1) = (
            (area.min.y / spacing).ceil() as i64,
            (area.max.y / spacing).floor() as i64,
        );
        if (x1 - x0 + 1).max(0) * (y1 - y0 + 1).max(0) > MAX_DOTS as i64 {
            return;
        }
        for j in y0..=y1 {
            for i in x0..=x1 {
                self.dots
                    .push(Point2::new(i as f32 * spacing, j as f32 * spacing));
            }
        }
    }
}

/// Découpe la droite passant par `base` de direction `dir` au rectangle `area` (Liang-Barsky).
fn clip_line(area: Rect2, base: Point2, dir: (f32, f32)) -> Option<(Point2, Point2)> {
    let (mut t0, mut t1) = (f32::NEG_INFINITY, f32::INFINITY);
    for (origin, d, lo, hi) in [
        (base.x, dir.0, area.min.x, area.max.x),
        (base.y, dir.1, area.min.y, area.max.y),
    ] {
        if d.abs() < 1e-6 {
            if origin < lo || origin > hi {
                return None;
            }
        } else {
            let (a, b) = ((lo - origin) / d, (hi - origin) / d);
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
        }
    }
    (t0 < t1).then(|| {
        (
            Point2::new(base.x + dir.0 * t0, base.y + dir.1 * t0),
            Point2::new(base.x + dir.0 * t1, base.y + dir.1 * t1),
        )
    })
}

/// Zone exportée d'une page : la page A4 (ou l'image de fond) élargie à tout le contenu.
pub fn export_rect(items: &[Item], image_size: Option<(f32, f32)>) -> Rect2 {
    let (width, height) = image_size.unwrap_or((PAGE_WIDTH, PAGE_HEIGHT));
    let base = Rect2 {
        min: Point2::new(0.0, 0.0),
        max: Point2::new(width, height),
    };
    items
        .iter()
        .filter_map(Item::bounds)
        .reduce(Rect2::union)
        .map_or(base, |content| base.union(content.expand(EXPORT_MARGIN)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::TextBox;
    use crate::stroke::{Brush, Stroke, StrokePoint};

    fn area(w: f32, h: f32) -> Rect2 {
        Rect2 {
            min: Point2::new(0.0, 0.0),
            max: Point2::new(w, h),
        }
    }

    #[test]
    fn database_representation_roundtrips() {
        let image = Background::Image(AssetId::new());
        for bg in PATTERN_BACKGROUNDS.into_iter().chain([image]) {
            assert_eq!(Background::from_db(&bg.to_db()), Some(bg));
        }
        assert_eq!(Background::from_db("inconnu"), None);
        assert_eq!(Background::from_db("asset:pas-un-uuid"), None);
        assert_eq!(image.image().map(|_| ()), Some(()));
        assert_eq!(Background::GridSmall.image(), None);
    }

    #[test]
    fn grid_has_lines_in_both_directions_inside_the_area() {
        let a = area(200.0, 100.0);
        let p = Background::GridSmall.pattern(a);
        let step = 5.0 * UNITS_PER_MM;
        let vertical = p
            .lines
            .iter()
            .filter(|(s, e)| (s.x - e.x).abs() < 1e-3)
            .count();
        let horizontal = p
            .lines
            .iter()
            .filter(|(s, e)| (s.y - e.y).abs() < 1e-3)
            .count();
        assert_eq!(vertical, (200.0 / step).floor() as usize + 1);
        assert_eq!(horizontal, (100.0 / step).floor() as usize + 1);
        assert!(p
            .lines
            .iter()
            .all(|(s, e)| a.expand(1e-2).contains(*s) && a.expand(1e-2).contains(*e)));
        assert!(p.dots.is_empty());
    }

    #[test]
    fn ruled_paper_has_only_horizontal_lines() {
        let p = Background::Lines.pattern(area(300.0, 300.0));
        assert!(!p.lines.is_empty());
        assert!(p.lines.iter().all(|(s, e)| (s.y - e.y).abs() < 1e-3));
    }

    #[test]
    fn dots_sit_on_the_grid() {
        let p = Background::Dots.pattern(area(100.0, 100.0));
        assert!(p.lines.is_empty());
        let step = 5.0 * UNITS_PER_MM;
        assert!(!p.dots.is_empty());
        assert!(p.dots.iter().all(|d| {
            let (fx, fy) = (d.x / step, d.y / step);
            (fx - fx.round()).abs() < 1e-3 && (fy - fy.round()).abs() < 1e-3
        }));
    }

    #[test]
    fn isometric_has_three_directions() {
        let p = Background::Isometric.pattern(area(300.0, 300.0));
        let mut angles: Vec<i32> = p
            .lines
            .iter()
            .map(|(s, e)| {
                let a = (e.y - s.y).atan2(e.x - s.x).to_degrees();
                let a = a.rem_euclid(180.0);
                a.round() as i32 % 180
            })
            .collect();
        angles.sort_unstable();
        angles.dedup();
        assert_eq!(angles, vec![30, 90, 150]);
    }

    #[test]
    fn plain_and_image_backgrounds_have_no_pattern_and_huge_areas_are_skipped() {
        let a = area(300.0, 300.0);
        assert!(Background::Blank.pattern(a).lines.is_empty());
        assert!(Background::Image(AssetId::new())
            .pattern(a)
            .lines
            .is_empty());
        // Vue très dézoomée : trop de lignes, on n'en dessine aucune.
        let huge = area(1.0e7, 1.0e7);
        assert!(Background::GridSmall.pattern(huge).lines.is_empty());
        assert!(Background::Dots.pattern(huge).dots.is_empty());
    }

    #[test]
    fn export_rect_covers_the_page_and_all_the_content() {
        assert_eq!(export_rect(&[], None), area(PAGE_WIDTH, PAGE_HEIGHT));
        assert_eq!(export_rect(&[], Some((500.0, 700.0))), area(500.0, 700.0));

        let mut stroke = Stroke::new(Brush::default());
        for p in [Point2::new(100.0, 100.0), Point2::new(900.0, 2000.0)] {
            stroke.push(StrokePoint {
                pos: p,
                pressure: 0.5,
                tilt: None,
            });
        }
        let text = TextBox::new(Point2::new(-50.0, 10.0), "x".into(), 20.0, Color::BLACK);
        let r = export_rect(&[Item::Stroke(stroke), Item::Text(text)], None);
        assert!(r.max.y >= 2000.0 + EXPORT_MARGIN - 1e-3);
        assert!(r.max.x >= 900.0 + EXPORT_MARGIN - 1e-3);
        assert!(r.min.x <= -50.0 - EXPORT_MARGIN + 1e-3);
        assert_eq!(r.min.y.min(0.0), r.min.y);
    }
}
