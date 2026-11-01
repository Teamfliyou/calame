//! Règle, équerre, rapporteur et compas : géométrie des bords sur lesquels le tracé se cale.
//!
//! Les instruments sont des outils posés sur la page, pas des éléments de la note : ils ne sont
//! pas enregistrés avec elle.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use crate::geometry::{dist_to_segment, Point2};

/// Un centimètre en unités de page (96 points par pouce).
pub const UNITS_PER_CM: f32 = 37.795_277;
pub const UNITS_PER_MM: f32 = UNITS_PER_CM / 10.0;

const MIN_SIZE: f32 = 2.0 * UNITS_PER_CM;

/// Position et orientation d'un repère local (x vers la droite, y vers le bas) sur la page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub origin: Point2,
    /// Radians, sens horaire à l'écran.
    pub angle: f32,
}

impl Pose {
    pub fn to_page(self, local: Point2) -> Point2 {
        let (sin, cos) = self.angle.sin_cos();
        Point2::new(
            self.origin.x + local.x * cos - local.y * sin,
            self.origin.y + local.x * sin + local.y * cos,
        )
    }

    pub fn to_local(self, page: Point2) -> Point2 {
        let (sin, cos) = (-self.angle).sin_cos();
        let (dx, dy) = (page.x - self.origin.x, page.y - self.origin.y);
        Point2::new(dx * cos - dy * sin, dx * sin + dy * cos)
    }
}

/// Bord le long duquel le trait peut être guidé.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Edge {
    Segment {
        a: Point2,
        b: Point2,
    },
    /// Arc de cercle ; `sweep >= TAU` pour un cercle entier.
    Arc {
        center: Point2,
        radius: f32,
        start: f32,
        sweep: f32,
    },
}

/// Pas angulaire maximal entre deux points ajoutés le long d'un arc (5°).
const ARC_STEP: f32 = 5.0 * PI / 180.0;

impl Edge {
    /// Point du bord le plus proche de `p`.
    pub fn project(&self, p: Point2) -> Point2 {
        match *self {
            Self::Segment { a, b } => {
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let len2 = dx * dx + dy * dy;
                if len2 <= f32::EPSILON {
                    return a;
                }
                let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
                Point2::new(a.x + t * dx, a.y + t * dy)
            }
            Self::Arc {
                center,
                radius,
                start,
                sweep,
            } => {
                let angle = nearest_angle_on_arc(p, center, start, sweep);
                Point2::new(
                    center.x + radius * angle.cos(),
                    center.y + radius * angle.sin(),
                )
            }
        }
    }

    pub fn distance(&self, p: Point2) -> f32 {
        p.distance(self.project(p))
    }

    /// Points à ajouter à un tracé guidé pour aller de `prev` (déjà sur le bord) jusqu'à la
    /// projection de `p`. Sur un arc, on suit le cercle plutôt que de tracer une corde.
    pub fn extend(&self, prev: Option<Point2>, p: Point2) -> Vec<Point2> {
        let target = self.project(p);
        let (
            Self::Arc {
                center,
                radius,
                start,
                sweep,
            },
            Some(prev),
        ) = (*self, prev)
        else {
            return vec![target];
        };
        let rel = |q: Point2| (q.y - center.y).atan2(q.x - center.x) - start;
        let (r0, r1) = (rel(prev).rem_euclid(TAU), rel(target).rem_euclid(TAU));
        let mut delta = r1 - r0;
        if sweep >= TAU - 1e-4 {
            // Cercle entier : on prend le chemin le plus court, quel que soit le sens.
            if delta > PI {
                delta -= TAU;
            } else if delta <= -PI {
                delta += TAU;
            }
        }
        let steps = (delta.abs() / ARC_STEP).ceil().max(1.0) as usize;
        (1..=steps)
            .map(|k| {
                let angle = start + r0 + delta * k as f32 / steps as f32;
                Point2::new(
                    center.x + radius * angle.cos(),
                    center.y + radius * angle.sin(),
                )
            })
            .collect()
    }
}

fn nearest_angle_on_arc(p: Point2, center: Point2, start: f32, sweep: f32) -> f32 {
    let theta = (p.y - center.y).atan2(p.x - center.x);
    if sweep >= TAU - 1e-4 {
        return theta;
    }
    let rel = (theta - start).rem_euclid(TAU);
    if rel <= sweep {
        theta
    } else if rel - sweep < TAU - rel {
        start + sweep
    } else {
        start
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstrumentKind {
    Ruler,
    SetSquare45,
    SetSquare60,
    Protractor,
    Compass,
}

/// Poignée de manipulation d'un instrument.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    Rotate,
    Resize,
    /// Compas : la pointe sèche (déplace tout le compas).
    Pivot,
    /// Compas : la mine (règle l'écartement).
    Pencil,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Instrument {
    /// Règle graduée ; ses deux grands côtés guident le trait.
    Ruler {
        pose: Pose,
        length: f32,
        width: f32,
    },
    /// Équerre : angle droit à l'origine du repère, branches le long de x et de y.
    SetSquare {
        pose: Pose,
        leg_x: f32,
        leg_y: f32,
    },
    /// Rapporteur : demi-disque au-dessus de l'axe x, 0° à droite, 180° à gauche.
    Protractor {
        pose: Pose,
        radius: f32,
    },
    Compass {
        pivot: Point2,
        pencil: Point2,
    },
}

impl Instrument {
    /// Instrument de taille usuelle centré sur `center`.
    pub fn new(kind: InstrumentKind, center: Point2) -> Self {
        let cm = UNITS_PER_CM;
        let at = |dx: f32, dy: f32| Pose {
            origin: Point2::new(center.x - dx, center.y - dy),
            angle: 0.0,
        };
        match kind {
            InstrumentKind::Ruler => Self::Ruler {
                pose: at(10.0 * cm, 1.75 * cm),
                length: 20.0 * cm,
                width: 3.5 * cm,
            },
            InstrumentKind::SetSquare45 => Self::SetSquare {
                pose: at(4.0 * cm, 4.0 * cm),
                leg_x: 12.0 * cm,
                leg_y: 12.0 * cm,
            },
            InstrumentKind::SetSquare60 => Self::SetSquare {
                pose: at(3.0 * cm, 5.0 * cm),
                leg_x: 10.0 * cm,
                leg_y: 10.0 * cm * 3.0_f32.sqrt(),
            },
            InstrumentKind::Protractor => Self::Protractor {
                pose: Pose {
                    origin: Point2::new(center.x, center.y + 3.0 * cm),
                    angle: 0.0,
                },
                radius: 6.0 * cm,
            },
            InstrumentKind::Compass => Self::Compass {
                pivot: center,
                pencil: Point2::new(center.x + 4.0 * cm, center.y),
            },
        }
    }

    /// Bords de guidage, en coordonnées de page.
    pub fn edges(&self) -> Vec<Edge> {
        match self {
            Self::Ruler {
                pose,
                length,
                width,
            } => {
                let seg = |y: f32| Edge::Segment {
                    a: pose.to_page(Point2::new(0.0, y)),
                    b: pose.to_page(Point2::new(*length, y)),
                };
                vec![seg(0.0), seg(*width)]
            }
            Self::SetSquare { pose, leg_x, leg_y } => {
                let o = pose.to_page(Point2::new(0.0, 0.0));
                let x = pose.to_page(Point2::new(*leg_x, 0.0));
                let y = pose.to_page(Point2::new(0.0, *leg_y));
                vec![
                    Edge::Segment { a: o, b: x },
                    Edge::Segment { a: x, b: y },
                    Edge::Segment { a: y, b: o },
                ]
            }
            Self::Protractor { pose, radius } => vec![
                Edge::Segment {
                    a: pose.to_page(Point2::new(-radius, 0.0)),
                    b: pose.to_page(Point2::new(*radius, 0.0)),
                },
                Edge::Arc {
                    center: pose.origin,
                    radius: *radius,
                    start: pose.angle + PI,
                    sweep: PI,
                },
            ],
            Self::Compass { pivot, pencil } => vec![Edge::Arc {
                center: *pivot,
                radius: pivot.distance(*pencil),
                start: 0.0,
                sweep: TAU,
            }],
        }
    }

    /// Contour rempli de l'instrument (vide pour le compas), en coordonnées de page.
    pub fn outline(&self) -> Vec<Point2> {
        match self {
            Self::Ruler {
                pose,
                length,
                width,
            } => [(0.0, 0.0), (*length, 0.0), (*length, *width), (0.0, *width)]
                .iter()
                .map(|&(x, y)| pose.to_page(Point2::new(x, y)))
                .collect(),
            Self::SetSquare { pose, leg_x, leg_y } => [(0.0, 0.0), (*leg_x, 0.0), (0.0, *leg_y)]
                .iter()
                .map(|&(x, y)| pose.to_page(Point2::new(x, y)))
                .collect(),
            Self::Protractor { pose, radius } => {
                const N: usize = 64;
                (0..=N)
                    .map(|i| {
                        let a = PI * i as f32 / N as f32;
                        pose.to_page(Point2::new(radius * a.cos(), -radius * a.sin()))
                    })
                    .collect()
            }
            Self::Compass { .. } => Vec::new(),
        }
    }

    /// `true` si `p` est sur le corps de l'instrument (pour le saisir).
    pub fn contains(&self, p: Point2) -> bool {
        const GRAB: f32 = 12.0;
        match self {
            Self::Ruler {
                pose,
                length,
                width,
            } => {
                let l = pose.to_local(p);
                l.x >= 0.0 && l.x <= *length && l.y >= 0.0 && l.y <= *width
            }
            Self::SetSquare { pose, leg_x, leg_y } => {
                let l = pose.to_local(p);
                l.x >= 0.0 && l.y >= 0.0 && l.x / leg_x + l.y / leg_y <= 1.0
            }
            Self::Protractor { pose, radius } => {
                let l = pose.to_local(p);
                l.y <= 0.0 && l.x.hypot(l.y) <= *radius
            }
            Self::Compass { pivot, pencil } => dist_to_segment(p, *pivot, *pencil) <= GRAB,
        }
    }

    /// Poignées de manipulation, en coordonnées de page.
    pub fn handles(&self) -> Vec<(Handle, Point2)> {
        match self {
            Self::Ruler {
                pose,
                length,
                width,
            } => vec![
                (Handle::Rotate, pose.to_page(Point2::new(0.0, width / 2.0))),
                (
                    Handle::Resize,
                    pose.to_page(Point2::new(*length, width / 2.0)),
                ),
            ],
            Self::SetSquare { pose, leg_x, leg_y } => vec![
                (Handle::Rotate, pose.to_page(Point2::new(*leg_x, 0.0))),
                (Handle::Resize, pose.to_page(Point2::new(0.0, *leg_y))),
            ],
            Self::Protractor { pose, radius } => vec![
                (Handle::Rotate, pose.to_page(Point2::new(0.0, -radius))),
                (Handle::Resize, pose.to_page(Point2::new(*radius, 0.0))),
            ],
            Self::Compass { pivot, pencil } => {
                vec![(Handle::Pivot, *pivot), (Handle::Pencil, *pencil)]
            }
        }
    }

    pub fn translate(&mut self, dx: f32, dy: f32) {
        match self {
            Self::Ruler { pose, .. }
            | Self::SetSquare { pose, .. }
            | Self::Protractor { pose, .. } => {
                pose.origin = Point2::new(pose.origin.x + dx, pose.origin.y + dy);
            }
            Self::Compass { pivot, pencil } => {
                *pivot = Point2::new(pivot.x + dx, pivot.y + dy);
                *pencil = Point2::new(pencil.x + dx, pencil.y + dy);
            }
        }
    }

    /// Déplace une poignée jusqu'à `to` (pointeur, en coordonnées de page).
    pub fn drag_handle(&mut self, handle: Handle, to: Point2) {
        match (self, handle) {
            (
                Self::Ruler {
                    pose,
                    length,
                    width,
                },
                Handle::Rotate,
            ) => {
                // La poignée est au milieu du petit côté gauche : on tourne autour du centre.
                let center = pose.to_page(Point2::new(*length / 2.0, *width / 2.0));
                let angle = (to.y - center.y).atan2(to.x - center.x) - PI;
                let (sin, cos) = angle.sin_cos();
                let (hx, hy) = (*length / 2.0, *width / 2.0);
                *pose = Pose {
                    origin: Point2::new(
                        center.x - (hx * cos - hy * sin),
                        center.y - (hx * sin + hy * cos),
                    ),
                    angle,
                };
            }
            (Self::Ruler { pose, length, .. }, Handle::Resize) => {
                *length = pose.to_local(to).x.clamp(MIN_SIZE, 60.0 * UNITS_PER_CM);
            }
            (Self::SetSquare { pose, .. }, Handle::Rotate) => {
                let o = pose.origin;
                *pose = Pose {
                    origin: o,
                    angle: (to.y - o.y).atan2(to.x - o.x),
                };
            }
            (Self::SetSquare { pose, leg_x, leg_y }, Handle::Resize) => {
                let factor = pose.origin.distance(to) / *leg_y;
                let new_y = (*leg_y * factor).clamp(MIN_SIZE, 40.0 * UNITS_PER_CM);
                let applied = new_y / *leg_y;
                *leg_x *= applied;
                *leg_y = new_y;
            }
            (Self::Protractor { pose, .. }, Handle::Rotate) => {
                let c = pose.origin;
                pose.angle = (to.y - c.y).atan2(to.x - c.x) + FRAC_PI_2;
            }
            (Self::Protractor { pose, radius }, Handle::Resize) => {
                *radius = pose
                    .origin
                    .distance(to)
                    .clamp(MIN_SIZE, 30.0 * UNITS_PER_CM);
            }
            (Self::Compass { pivot, pencil }, Handle::Pivot) => {
                // Déplacer la pointe sèche déplace tout le compas, écartement conservé.
                let (dx, dy) = (to.x - pivot.x, to.y - pivot.y);
                *pivot = to;
                *pencil = Point2::new(pencil.x + dx, pencil.y + dy);
            }
            (Self::Compass { pivot, pencil }, Handle::Pencil) => {
                let d = pivot.distance(to);
                *pencil = if d < UNITS_PER_CM {
                    // Écartement minimal d'un centimètre, dans la direction visée.
                    let (ux, uy) = if d > f32::EPSILON {
                        ((to.x - pivot.x) / d, (to.y - pivot.y) / d)
                    } else {
                        (1.0, 0.0)
                    };
                    Point2::new(pivot.x + ux * UNITS_PER_CM, pivot.y + uy * UNITS_PER_CM)
                } else {
                    to
                };
            }
            _ => {}
        }
    }

    /// Écartement du compas (rayon du cercle tracé), sinon `None`.
    pub fn compass_radius(&self) -> Option<f32> {
        match self {
            Self::Compass { pivot, pencil } => Some(pivot.distance(*pencil)),
            _ => None,
        }
    }
}

/// Bord le plus proche de `p` à moins de `max_dist`, parmi tous les instruments.
/// À distance égale, l'instrument le plus récent (le dernier de la liste) l'emporte.
pub fn snap(instruments: &[Instrument], p: Point2, max_dist: f32) -> Option<Edge> {
    let mut best: Option<(f32, Edge)> = None;
    for edge in instruments.iter().flat_map(Instrument::edges) {
        let d = edge.distance(p);
        if d <= max_dist && best.as_ref().map_or(true, |(b, _)| d <= *b) {
            best = Some((d, edge));
        }
    }
    best.map(|(_, e)| e)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Point2, b: Point2) -> bool {
        a.distance(b) < 1e-3
    }

    #[test]
    fn pose_roundtrip() {
        let pose = Pose {
            origin: Point2::new(10.0, -5.0),
            angle: 0.7,
        };
        let p = Point2::new(3.0, 4.0);
        assert!(close(pose.to_local(pose.to_page(p)), p));
    }

    #[test]
    fn segment_projection_clamps_to_ends() {
        let e = Edge::Segment {
            a: Point2::new(0.0, 0.0),
            b: Point2::new(10.0, 0.0),
        };
        assert_eq!(e.project(Point2::new(4.0, 7.0)), Point2::new(4.0, 0.0));
        assert_eq!(e.project(Point2::new(-3.0, 1.0)), Point2::new(0.0, 0.0));
        assert_eq!(e.distance(Point2::new(4.0, 7.0)), 7.0);
    }

    #[test]
    fn circle_projection_is_radial() {
        let e = Edge::Arc {
            center: Point2::new(0.0, 0.0),
            radius: 10.0,
            start: 0.0,
            sweep: TAU,
        };
        assert!(close(
            e.project(Point2::new(30.0, 0.0)),
            Point2::new(10.0, 0.0)
        ));
        assert!(close(
            e.project(Point2::new(0.0, -2.0)),
            Point2::new(0.0, -10.0)
        ));
    }

    #[test]
    fn partial_arc_clamps_to_nearest_end() {
        // Demi-cercle supérieur (y vers le bas) : de 180° à 360°.
        let e = Edge::Arc {
            center: Point2::new(0.0, 0.0),
            radius: 10.0,
            start: PI,
            sweep: PI,
        };
        assert!(close(
            e.project(Point2::new(0.0, -5.0)),
            Point2::new(0.0, -10.0)
        ));
        // Sous l'axe : on retombe sur l'extrémité la plus proche.
        assert!(close(
            e.project(Point2::new(8.0, 3.0)),
            Point2::new(10.0, 0.0)
        ));
        assert!(close(
            e.project(Point2::new(-8.0, 3.0)),
            Point2::new(-10.0, 0.0)
        ));
    }

    #[test]
    fn arc_extend_follows_the_circle_instead_of_cutting_a_chord() {
        let e = Edge::Arc {
            center: Point2::new(0.0, 0.0),
            radius: 100.0,
            start: 0.0,
            sweep: TAU,
        };
        let from = Point2::new(100.0, 0.0);
        // Quart de tour d'un coup : plusieurs points intermédiaires, tous sur le cercle.
        let pts = e.extend(Some(from), Point2::new(0.0, 100.0));
        assert!(pts.len() >= 18);
        assert!(pts.iter().all(|p| (p.x.hypot(p.y) - 100.0).abs() < 1e-2));
        assert!(close(pts[pts.len() - 1], Point2::new(0.0, 100.0)));
        // Traverser l'angle 0 se fait par le chemin court, sans faire le tour.
        let wrap = e.extend(
            Some(Point2::new(100.0 * 0.1_f32.cos(), -100.0 * 0.1_f32.sin())),
            Point2::new(100.0 * 0.1_f32.cos(), 100.0 * 0.1_f32.sin()),
        );
        assert!(wrap.len() <= 4);
        assert!(Edge::Segment { a: from, b: from }.extend(None, from).len() == 1);
    }

    #[test]
    fn ruler_has_two_parallel_drawing_edges() {
        let r = Instrument::new(InstrumentKind::Ruler, Point2::new(0.0, 0.0));
        let edges = r.edges();
        assert_eq!(edges.len(), 2);
        let Edge::Segment { a, b } = edges[0] else {
            unreachable!("la règle n'a que des bords droits");
        };
        assert!((a.distance(b) - 20.0 * UNITS_PER_CM).abs() < 1e-2);
        assert!(r.contains(Point2::new(0.0, 0.0)));
        assert!(!r.contains(Point2::new(0.0, 5.0 * UNITS_PER_CM)));
    }

    #[test]
    fn set_square_is_a_right_triangle() {
        let s = Instrument::new(InstrumentKind::SetSquare45, Point2::new(0.0, 0.0));
        let Instrument::SetSquare { pose, leg_x, leg_y } = s.clone() else {
            unreachable!("c'est une équerre");
        };
        assert_eq!(leg_x, leg_y);
        // Le point juste à l'intérieur de l'hypoténuse est dans l'équerre, juste à l'extérieur non.
        assert!(s.contains(pose.to_page(Point2::new(leg_x * 0.45, leg_y * 0.45))));
        assert!(!s.contains(pose.to_page(Point2::new(leg_x * 0.55, leg_y * 0.55))));
        let t = Instrument::new(InstrumentKind::SetSquare60, Point2::new(0.0, 0.0));
        let Instrument::SetSquare { leg_x, leg_y, .. } = t else {
            unreachable!("c'est une équerre");
        };
        assert!((leg_y / leg_x - 3.0_f32.sqrt()).abs() < 1e-4);
    }

    #[test]
    fn protractor_covers_the_upper_half_only() {
        let p = Instrument::new(InstrumentKind::Protractor, Point2::new(0.0, 0.0));
        let Instrument::Protractor { pose, radius } = p.clone() else {
            unreachable!("c'est un rapporteur");
        };
        assert!(p.contains(pose.to_page(Point2::new(0.0, -radius * 0.5))));
        assert!(!p.contains(pose.to_page(Point2::new(0.0, radius * 0.5))));
        assert_eq!(p.edges().len(), 2);
    }

    #[test]
    fn compass_edge_has_the_opening_as_radius() {
        let mut c = Instrument::new(InstrumentKind::Compass, Point2::new(0.0, 0.0));
        let r0 = c.compass_radius().unwrap_or_default();
        assert!((r0 - 4.0 * UNITS_PER_CM).abs() < 1e-3);
        c.drag_handle(Handle::Pencil, Point2::new(0.0, 100.0));
        assert!((c.compass_radius().unwrap_or_default() - 100.0).abs() < 1e-3);
        // Écartement minimal d'un centimètre.
        c.drag_handle(Handle::Pencil, Point2::new(1.0, 0.0));
        assert!((c.compass_radius().unwrap_or_default() - UNITS_PER_CM).abs() < 1e-3);
        // Déplacer la pointe sèche conserve l'écartement.
        c.drag_handle(Handle::Pivot, Point2::new(50.0, 50.0));
        assert!((c.compass_radius().unwrap_or_default() - UNITS_PER_CM).abs() < 1e-3);
        assert!(close(c.handles()[0].1, Point2::new(50.0, 50.0)));
    }

    #[test]
    fn ruler_rotate_keeps_center_and_resize_changes_length() {
        let mut r = Instrument::new(InstrumentKind::Ruler, Point2::new(100.0, 100.0));
        let center_before = {
            let Instrument::Ruler {
                pose,
                length,
                width,
            } = &r
            else {
                unreachable!("c'est une règle");
            };
            pose.to_page(Point2::new(length / 2.0, width / 2.0))
        };
        // On tire la poignée gauche sous le centre : la règle devient verticale.
        r.drag_handle(Handle::Rotate, Point2::new(100.0, 400.0));
        let Instrument::Ruler {
            pose,
            length,
            width,
        } = r.clone()
        else {
            unreachable!("c'est une règle");
        };
        assert!(close(
            pose.to_page(Point2::new(length / 2.0, width / 2.0)),
            center_before
        ));
        assert!((pose.angle + FRAC_PI_2).abs() < 1e-3);
        r.drag_handle(
            Handle::Resize,
            pose.to_page(Point2::new(10.0 * UNITS_PER_CM, 0.0)),
        );
        let Instrument::Ruler { length, .. } = r else {
            unreachable!("c'est une règle");
        };
        assert!((length - 10.0 * UNITS_PER_CM).abs() < 1e-2);
    }

    #[test]
    fn snap_picks_the_nearest_edge_within_range() {
        let ruler = Instrument::new(InstrumentKind::Ruler, Point2::new(0.0, 0.0));
        let top = ruler.edges()[0];
        let Edge::Segment { a, .. } = top else {
            unreachable!("bord droit");
        };
        let near = Point2::new(a.x + 100.0, a.y - 5.0);
        assert_eq!(snap(std::slice::from_ref(&ruler), near, 10.0), Some(top));
        assert_eq!(
            snap(
                std::slice::from_ref(&ruler),
                Point2::new(a.x + 100.0, a.y - 50.0),
                10.0
            ),
            None
        );
        assert_eq!(snap(&[], near, 10.0), None);
    }
}
