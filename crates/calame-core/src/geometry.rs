use serde::{Deserialize, Serialize};

/// Point en coordonnées de page (unités logiques, y vers le bas, hauteur infinie).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point2 {
    pub x: f32,
    pub y: f32,
}

impl Point2 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn distance(self, other: Self) -> f32 {
        (self.x - other.x).hypot(self.y - other.y)
    }

    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self::new(
            self.x + (other.x - self.x) * t,
            self.y + (other.y - self.y) * t,
        )
    }
}

/// Rectangle aligné sur les axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect2 {
    pub min: Point2,
    pub max: Point2,
}

impl Rect2 {
    pub fn from_points(points: impl IntoIterator<Item = Point2>) -> Option<Self> {
        let mut iter = points.into_iter();
        let first = iter.next()?;
        let mut r = Self {
            min: first,
            max: first,
        };
        for p in iter {
            r.min = Point2::new(r.min.x.min(p.x), r.min.y.min(p.y));
            r.max = Point2::new(r.max.x.max(p.x), r.max.y.max(p.y));
        }
        Some(r)
    }

    pub fn union(self, other: Self) -> Self {
        Self {
            min: Point2::new(self.min.x.min(other.min.x), self.min.y.min(other.min.y)),
            max: Point2::new(self.max.x.max(other.max.x), self.max.y.max(other.max.y)),
        }
    }

    pub fn expand(self, margin: f32) -> Self {
        Self {
            min: Point2::new(self.min.x - margin, self.min.y - margin),
            max: Point2::new(self.max.x + margin, self.max.y + margin),
        }
    }

    pub fn center(self) -> Point2 {
        self.min.lerp(self.max, 0.5)
    }

    pub fn width(self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn height(self) -> f32 {
        self.max.y - self.min.y
    }

    pub fn contains(self, p: Point2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }
}

/// Distance du point `p` au segment `[a, b]`.
pub fn dist_to_segment(p: Point2, a: Point2, b: Point2) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    if len2 <= f32::EPSILON {
        return p.distance(a);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
    p.distance(Point2::new(a.x + t * dx, a.y + t * dy))
}

/// Test d'appartenance à un polygone (règle pair-impair), polygone fermé implicitement.
pub fn point_in_polygon(p: Point2, polygon: &[Point2]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let (a, b) = (polygon[i], polygon[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Rotation puis mise à l'échelle uniforme autour de `pivot`, suivies d'une translation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Similarity {
    pub pivot: Point2,
    pub scale: f32,
    /// Angle en radians, sens horaire à l'écran (y vers le bas).
    pub angle: f32,
    pub translate: Point2,
}

impl Similarity {
    pub fn identity() -> Self {
        Self {
            pivot: Point2::default(),
            scale: 1.0,
            angle: 0.0,
            translate: Point2::default(),
        }
    }

    pub fn translation(dx: f32, dy: f32) -> Self {
        Self {
            translate: Point2::new(dx, dy),
            ..Self::identity()
        }
    }

    pub fn scaling(pivot: Point2, scale: f32) -> Self {
        Self {
            pivot,
            scale,
            ..Self::identity()
        }
    }

    pub fn rotation(pivot: Point2, angle: f32) -> Self {
        Self {
            pivot,
            angle,
            ..Self::identity()
        }
    }

    pub fn apply(&self, p: Point2) -> Point2 {
        let (v_x, v_y) = (p.x - self.pivot.x, p.y - self.pivot.y);
        let (sin, cos) = self.angle.sin_cos();
        Point2::new(
            self.pivot.x + self.scale * (v_x * cos - v_y * sin) + self.translate.x,
            self.pivot.y + self.scale * (v_x * sin + v_y * cos) + self.translate.y,
        )
    }
}

/// Couleur RGBA non prémultipliée.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const BLACK: Self = Self::rgb(0, 0, 0);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Format `#rrggbb` (sans transparence).
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// Lit `#rrggbb` ou `rrggbb`, opaque.
    pub fn from_hex(text: &str) -> Option<Self> {
        let digits = text.trim().trim_start_matches('#');
        if digits.len() != 6 || !digits.is_ascii() {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
        Some(Self::rgb(byte(0)?, byte(2)?, byte(4)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_distance_clamps_to_endpoints() {
        let (a, b) = (Point2::new(0.0, 0.0), Point2::new(10.0, 0.0));
        assert_eq!(dist_to_segment(Point2::new(5.0, 3.0), a, b), 3.0);
        assert_eq!(dist_to_segment(Point2::new(-4.0, 3.0), a, b), 5.0);
        assert_eq!(dist_to_segment(Point2::new(2.0, 0.0), a, a), 2.0);
    }

    #[test]
    fn polygon_containment() {
        let square = [
            Point2::new(0.0, 0.0),
            Point2::new(10.0, 0.0),
            Point2::new(10.0, 10.0),
            Point2::new(0.0, 10.0),
        ];
        assert!(point_in_polygon(Point2::new(5.0, 5.0), &square));
        assert!(!point_in_polygon(Point2::new(15.0, 5.0), &square));
        assert!(!point_in_polygon(Point2::new(5.0, 5.0), &square[..2]));
    }

    #[test]
    fn similarity_rotates_scales_and_translates() {
        let pivot = Point2::new(1.0, 1.0);
        let q =
            Similarity::rotation(pivot, std::f32::consts::FRAC_PI_2).apply(Point2::new(2.0, 1.0));
        assert!(q.distance(Point2::new(1.0, 2.0)) < 1e-5);
        let s = Similarity::scaling(pivot, 2.0).apply(Point2::new(2.0, 1.0));
        assert!(s.distance(Point2::new(3.0, 1.0)) < 1e-5);
        let t = Similarity::translation(3.0, -1.0).apply(Point2::new(0.0, 0.0));
        assert_eq!(t, Point2::new(3.0, -1.0));
    }

    #[test]
    fn rect_union_and_contains() {
        let a = Rect2::from_points([Point2::new(0.0, 0.0), Point2::new(2.0, 2.0)])
            .unwrap_or_else(|| unreachable!());
        let b = Rect2::from_points([Point2::new(5.0, -1.0)]).unwrap_or_else(|| unreachable!());
        let u = a.union(b);
        assert!(u.contains(Point2::new(3.0, 0.0)));
        assert_eq!(u.width(), 5.0);
        assert_eq!(u.height(), 3.0);
        assert!(Rect2::from_points([]).is_none());
    }

    #[test]
    fn hex_roundtrip() {
        let c = Color::rgb(18, 171, 255);
        assert_eq!(Color::from_hex(&c.to_hex()), Some(c));
        assert_eq!(Color::from_hex("12ABff"), Some(c));
        assert_eq!(Color::from_hex("#12ab"), None);
        assert_eq!(Color::from_hex("zzzzzz"), None);
        assert_eq!(Color::from_hex("é1234"), None);
    }
}
