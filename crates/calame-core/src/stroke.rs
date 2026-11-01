use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::geometry::{Color, Point2};

/// Échantillon d'un tracé : position, pression et inclinaison.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct StrokePoint {
    pub pos: Point2,
    /// Pression normalisée dans `0.0..=1.0`.
    pub pressure: f32,
    /// Inclinaison (altitude) normalisée dans `0.0..=1.0`, si disponible.
    pub tilt: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrushKind {
    Pen,
    Highlighter,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Brush {
    pub kind: BrushKind,
    pub color: Color,
    /// Épaisseur maximale en unités de page.
    pub width: f32,
}

impl Default for Brush {
    fn default() -> Self {
        Self {
            kind: BrushKind::Pen,
            color: Color::BLACK,
            width: 3.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub id: Uuid,
    pub brush: Brush,
    pub points: Vec<StrokePoint>,
}

impl Stroke {
    pub fn new(brush: Brush) -> Self {
        Self {
            id: Uuid::new_v4(),
            brush,
            points: Vec::new(),
        }
    }

    pub fn push(&mut self, point: StrokePoint) {
        self.points.push(point);
    }

    /// Épaisseur effective en un point (la pression module l'épaisseur du stylo).
    pub fn width_at(&self, p: &StrokePoint) -> f32 {
        match self.brush.kind {
            BrushKind::Pen => self.brush.width * (0.25 + 0.75 * p.pressure.clamp(0.0, 1.0)),
            BrushKind::Highlighter => self.brush.width,
        }
    }

    /// Lisse le tracé par une spline de Catmull-Rom (`subdivisions` points par segment).
    pub fn smoothed(&self, subdivisions: usize) -> Vec<StrokePoint> {
        let n = self.points.len();
        if n < 3 || subdivisions < 2 {
            return self.points.clone();
        }
        let at = |i: isize| self.points[i.clamp(0, n as isize - 1) as usize];
        let mut out = Vec::with_capacity((n - 1) * subdivisions + 1);
        for i in 0..n as isize - 1 {
            let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
            for s in 0..subdivisions {
                let t = s as f32 / subdivisions as f32;
                out.push(StrokePoint {
                    pos: Point2::new(
                        catmull_rom(p0.pos.x, p1.pos.x, p2.pos.x, p3.pos.x, t),
                        catmull_rom(p0.pos.y, p1.pos.y, p2.pos.y, p3.pos.y, t),
                    ),
                    pressure: p1.pressure + (p2.pressure - p1.pressure) * t,
                    tilt: p1.tilt,
                });
            }
        }
        out.push(at(n as isize - 1));
        out
    }
}

fn catmull_rom(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f32, y: f32) -> StrokePoint {
        StrokePoint {
            pos: Point2::new(x, y),
            pressure: 0.5,
            tilt: None,
        }
    }

    #[test]
    fn smoothing_keeps_endpoints_and_passes_through_control_points() {
        let mut s = Stroke::new(Brush::default());
        for (x, y) in [(0.0, 0.0), (10.0, 5.0), (20.0, 0.0), (30.0, 5.0)] {
            s.push(pt(x, y));
        }
        let out = s.smoothed(4);
        assert_eq!(out.first().map(|p| p.pos), Some(Point2::new(0.0, 0.0)));
        assert_eq!(out.last().map(|p| p.pos), Some(Point2::new(30.0, 5.0)));
        assert_eq!(out.len(), 3 * 4 + 1);
        assert_eq!(out[4].pos, Point2::new(10.0, 5.0));
    }

    #[test]
    fn width_follows_pressure_for_pen_only() {
        let mut s = Stroke::new(Brush::default());
        let light = StrokePoint {
            pressure: 0.0,
            ..pt(0.0, 0.0)
        };
        let hard = StrokePoint {
            pressure: 1.0,
            ..pt(0.0, 0.0)
        };
        assert!(s.width_at(&hard) > s.width_at(&light));
        s.brush.kind = BrushKind::Highlighter;
        assert_eq!(s.width_at(&hard), s.width_at(&light));
    }
}
