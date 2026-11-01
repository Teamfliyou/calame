//! Reconnaissance de formes locale : transforme un tracé à main levée en forme parfaite.
//!
//! Aucun service externe : un rééchantillonnage, une simplification de Douglas-Peucker, des
//! ajustements géométriques (cercle de Kåsa, ellipse par axes principaux, rectangle d'aire
//! minimale) et un score d'écart moyen entre le tracé et chaque forme candidate.

mod geometry;

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use calame_core::{Brush, Point2, Stroke, StrokePoint};

use geometry::{
    bbox_diag, clean_closed, clean_open, coverage, dedup, fit_error, is_simple_polygon,
    path_length, resample, simplify_dp, smooth_path,
};

/// Longueur minimale d'un tracé reconnaissable, en unités de page.
const MIN_LENGTH: f32 = 24.0;
/// Un tracé est fermé si l'écart entre ses extrémités est inférieur à cette part de sa longueur.
const CLOSED_RATIO: f32 = 0.2;
const SAMPLES: usize = 96;
/// Écart moyen toléré (rapporté à la diagonale du tracé) pour accepter une forme.
const ROUND_TOLERANCE: f32 = 0.03;
const RECT_TOLERANCE: f32 = 0.025;
const POLYGON_TOLERANCE: f32 = 0.03;
/// Un trait est droit si son écart maximal à la corde reste sous cette part de la corde.
const LINE_TOLERANCE: f32 = 0.05;
/// Rapport d'axes en dessous duquel une forme ronde est un cercle.
const CIRCLE_ASPECT: f32 = 1.12;
/// Rapport de côtés en dessous duquel un rectangle est un carré.
const SQUARE_ASPECT: f32 = 0.1;
/// Tolérance de calage d'un trait ou d'un rectangle sur les axes (4°).
const AXIS_SNAP: f32 = 4.0 * PI / 180.0;

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Line {
        a: Point2,
        b: Point2,
    },
    Circle {
        center: Point2,
        radius: f32,
    },
    Ellipse {
        center: Point2,
        rx: f32,
        ry: f32,
        /// Angle de l'axe de longueur `rx`, en radians.
        angle: f32,
    },
    Rectangle {
        center: Point2,
        /// Longueur du côté orienté selon `angle`.
        width: f32,
        height: f32,
        angle: f32,
    },
    Square {
        center: Point2,
        side: f32,
        angle: f32,
    },
    Triangle {
        vertices: [Point2; 3],
    },
    Arrow {
        tail: Point2,
        tip: Point2,
        /// Longueur de chaque aile de la pointe.
        head_len: f32,
        /// Angle entre une aile et la tige, en radians.
        head_angle: f32,
    },
    Polygon {
        vertices: Vec<Point2>,
    },
    /// Tracé lissé, ouvert ou fermé.
    Curve {
        points: Vec<Point2>,
        closed: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Line,
    Circle,
    Ellipse,
    Rectangle,
    Square,
    Triangle,
    Arrow,
    Polygon,
    Curve,
}

impl Shape {
    pub fn kind(&self) -> ShapeKind {
        match self {
            Self::Line { .. } => ShapeKind::Line,
            Self::Circle { .. } => ShapeKind::Circle,
            Self::Ellipse { .. } => ShapeKind::Ellipse,
            Self::Rectangle { .. } => ShapeKind::Rectangle,
            Self::Square { .. } => ShapeKind::Square,
            Self::Triangle { .. } => ShapeKind::Triangle,
            Self::Arrow { .. } => ShapeKind::Arrow,
            Self::Polygon { .. } => ShapeKind::Polygon,
            Self::Curve { .. } => ShapeKind::Curve,
        }
    }

    /// Contour sous forme de ligne brisée ; un contour fermé revient à son point de départ.
    pub fn outline(&self) -> Vec<Point2> {
        match self {
            Self::Line { a, b } => vec![*a, *b],
            Self::Circle { center, radius } => ellipse_outline(*center, *radius, *radius, 0.0),
            Self::Ellipse {
                center,
                rx,
                ry,
                angle,
            } => ellipse_outline(*center, *rx, *ry, *angle),
            Self::Rectangle {
                center,
                width,
                height,
                angle,
            } => rect_outline(*center, *width, *height, *angle),
            Self::Square {
                center,
                side,
                angle,
            } => rect_outline(*center, *side, *side, *angle),
            Self::Triangle { vertices } => close(vertices.to_vec()),
            Self::Arrow {
                tail,
                tip,
                head_len,
                head_angle,
            } => {
                let (ux, uy) = unit(*tip, *tail);
                let wing = |sign: f32| {
                    let (sin, cos) = (sign * head_angle).sin_cos();
                    Point2::new(
                        tip.x + head_len * (ux * cos - uy * sin),
                        tip.y + head_len * (ux * sin + uy * cos),
                    )
                };
                vec![*tail, *tip, wing(1.0), *tip, wing(-1.0)]
            }
            Self::Polygon { vertices } => close(vertices.clone()),
            Self::Curve { points, closed } => {
                if *closed {
                    close(points.clone())
                } else {
                    points.clone()
                }
            }
        }
    }
}

fn close(mut v: Vec<Point2>) -> Vec<Point2> {
    if let Some(&first) = v.first() {
        v.push(first);
    }
    v
}

fn unit(from: Point2, to: Point2) -> (f32, f32) {
    let d = from.distance(to);
    if d <= f32::EPSILON {
        (1.0, 0.0)
    } else {
        ((to.x - from.x) / d, (to.y - from.y) / d)
    }
}

fn ellipse_outline(c: Point2, rx: f32, ry: f32, angle: f32) -> Vec<Point2> {
    let (sin, cos) = angle.sin_cos();
    let mut out: Vec<Point2> = (0..SAMPLES)
        .map(|i| {
            let t = TAU * i as f32 / SAMPLES as f32;
            let (x, y) = (rx * t.cos(), ry * t.sin());
            Point2::new(c.x + x * cos - y * sin, c.y + x * sin + y * cos)
        })
        .collect();
    out.push(out[0]);
    out
}

fn rect_outline(c: Point2, w: f32, h: f32, angle: f32) -> Vec<Point2> {
    let (sin, cos) = angle.sin_cos();
    let corner = |sx: f32, sy: f32| {
        let (x, y) = (sx * w / 2.0, sy * h / 2.0);
        Point2::new(c.x + x * cos - y * sin, c.y + x * sin + y * cos)
    };
    close(vec![
        corner(-1.0, -1.0),
        corner(1.0, -1.0),
        corner(1.0, 1.0),
        corner(-1.0, 1.0),
    ])
}

/// Reconnaît la forme la plus proche d'un tracé, ou `None` s'il est trop court pour en juger.
///
/// Un tracé ouvert qui n'est ni une droite ni une flèche devient une courbe lissée ; un tracé
/// fermé qui ne ressemble à aucune forme aussi.
pub fn recognize(points: &[Point2]) -> Option<Shape> {
    let pts = dedup(points, 0.01);
    if pts.len() < 5 {
        return None;
    }
    let length = path_length(&pts);
    let diag = bbox_diag(&pts);
    if length < MIN_LENGTH || diag < MIN_LENGTH / 2.0 {
        return None;
    }
    let rs = resample(&pts, SAMPLES);
    let gap = rs[0].distance(rs[rs.len() - 1]);
    if gap < CLOSED_RATIO * length {
        recognize_closed(&rs, diag)
    } else {
        recognize_open(&rs, length, diag)
    }
}

// ---- Tracés ouverts ---------------------------------------------------------------------

fn recognize_open(rs: &[Point2], length: f32, diag: f32) -> Option<Shape> {
    if let Some(line) = recognize_line(rs, length) {
        return Some(line);
    }
    if let Some(arrow) = recognize_arrow(rs, diag) {
        return Some(arrow);
    }
    Some(Shape::Curve {
        points: smooth_path(rs, false, 2),
        closed: false,
    })
}

fn recognize_line(rs: &[Point2], length: f32) -> Option<Shape> {
    let (a, b) = (rs[0], rs[rs.len() - 1]);
    let chord = a.distance(b);
    if chord < MIN_LENGTH || chord < 0.8 * length {
        return None;
    }
    let deviation = rs
        .iter()
        .map(|p| calame_core::geometry::dist_to_segment(*p, a, b))
        .fold(0.0, f32::max);
    if deviation > LINE_TOLERANCE * chord {
        return None;
    }
    // Un trait presque horizontal ou vertical le devient exactement, autour de son milieu.
    let angle = (b.y - a.y).atan2(b.x - a.x);
    let quarter = (angle / FRAC_PI_2).round();
    let angle = if (angle - quarter * FRAC_PI_2).abs() < AXIS_SNAP {
        quarter * FRAC_PI_2
    } else {
        angle
    };
    let (mid, half) = (a.lerp(b, 0.5), chord / 2.0);
    let (sin, cos) = angle.sin_cos();
    Some(Shape::Line {
        a: Point2::new(mid.x - half * cos, mid.y - half * sin),
        b: Point2::new(mid.x + half * cos, mid.y + half * sin),
    })
}

/// Flèche tracée d'un seul geste : tige, puis aile, retour à la pointe, seconde aile.
fn recognize_arrow(rs: &[Point2], diag: f32) -> Option<Shape> {
    let v = clean_open(simplify_dp(rs, 0.03 * diag), 20.0 * PI / 180.0);
    let [a, b, c, d, e] = v[..] else {
        return None;
    };
    let (shaft, wing1, wing2) = (a.distance(b), b.distance(c), d.distance(e));
    let (shortest, longest) = (wing1.min(wing2), wing1.max(wing2));
    if shortest <= f32::EPSILON || shaft < 2.0 * longest || longest > 1.6 * shortest {
        return None;
    }
    if b.distance(d) > 0.35 * shortest {
        return None;
    }
    let back = unit(b, a);
    let angle_of = |from: Point2, to: Point2| {
        let (wx, wy) = unit(from, to);
        (back.0 * wy - back.1 * wx).atan2(back.0 * wx + back.1 * wy)
    };
    let (a1, a2) = (angle_of(b, c), angle_of(d, e));
    // Les deux ailes sont de part et d'autre de la tige, à des angles raisonnables.
    let (lo, hi) = (10.0 * PI / 180.0, 70.0 * PI / 180.0);
    if a1 * a2 >= 0.0 || !(lo..=hi).contains(&a1.abs()) || !(lo..=hi).contains(&a2.abs()) {
        return None;
    }
    Some(Shape::Arrow {
        tail: a,
        tip: b,
        head_len: (wing1 + wing2) / 2.0,
        head_angle: (a1.abs() + a2.abs()) / 2.0,
    })
}

// ---- Tracés fermés ----------------------------------------------------------------------

fn recognize_closed(rs: &[Point2], diag: f32) -> Option<Shape> {
    // Biais de préférence : une forme simple l'emporte sur une forme plus générale à égalité.
    let mut candidates: Vec<(f32, Shape)> = Vec::new();
    let mut consider = |shape: Shape, tolerance: f32, bias: f32| -> bool {
        let outline = shape.outline();
        let error = fit_error(rs, &outline, diag);
        let accepted = error <= tolerance && coverage(&outline, rs, 0.1 * diag) >= 0.75;
        if accepted {
            candidates.push((error * bias, shape));
        }
        accepted
    };

    if let Some(round) = fit_round(rs) {
        consider(round, ROUND_TOLERANCE, 1.0);
    }

    // Sommets du contour : Douglas-Peucker sur la boucle refermée, sans sommets alignés.
    let mut looped = rs.to_vec();
    if let Some(last) = looped.last_mut() {
        *last = rs[0];
    }
    let mut vertices = simplify_dp(&looped, 0.035 * diag);
    vertices.pop();
    let vertices = clean_closed(vertices, 28.0 * PI / 180.0, 0.07 * diag);

    // Un quadrilatère qui tient la tolérance du rectangle reste un rectangle : le polygone
    // aux sommets du tracé suivrait mieux le tremblement, sans être une meilleure réponse.
    let is_rectangle = vertices.len() == 4 && consider(fit_rectangle(rs), RECT_TOLERANCE, 1.0);
    match vertices.len() {
        3 => consider(
            Shape::Triangle {
                vertices: [vertices[0], vertices[1], vertices[2]],
            },
            POLYGON_TOLERANCE,
            1.0,
        ),
        4..=8 if !is_rectangle && is_simple_polygon(&vertices) => consider(
            Shape::Polygon {
                vertices: vertices.clone(),
            },
            POLYGON_TOLERANCE,
            1.25,
        ),
        _ => false,
    };

    candidates
        .into_iter()
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, shape)| shape)
        .or_else(|| {
            Some(Shape::Curve {
                points: smooth_path(rs, true, 2),
                closed: true,
            })
        })
}

/// Cercle ou ellipse : axes principaux du nuage de points, puis cercle de Kåsa si presque rond.
fn fit_round(rs: &[Point2]) -> Option<Shape> {
    let n = rs.len() as f32;
    let (cx, cy) = (
        rs.iter().map(|p| p.x).sum::<f32>() / n,
        rs.iter().map(|p| p.y).sum::<f32>() / n,
    );
    let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
    for p in rs {
        let (dx, dy) = (p.x - cx, p.y - cy);
        sxx += dx * dx;
        sxy += dx * dy;
        syy += dy * dy;
    }
    let theta = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    let (sin, cos) = theta.sin_cos();
    let (mut umin, mut umax, mut vmin, mut vmax) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for p in rs {
        let (dx, dy) = (p.x - cx, p.y - cy);
        let (u, v) = (dx * cos + dy * sin, -dx * sin + dy * cos);
        umin = umin.min(u);
        umax = umax.max(u);
        vmin = vmin.min(v);
        vmax = vmax.max(v);
    }
    let (rx, ry) = ((umax - umin) / 2.0, (vmax - vmin) / 2.0);
    // Un nuage aplati (va-et-vient sur une droite) n'est pas une ellipse.
    if rx.min(ry) < 0.12 * rx.max(ry) {
        return None;
    }
    let (um, vm) = ((umax + umin) / 2.0, (vmax + vmin) / 2.0);
    let center = Point2::new(cx + um * cos - vm * sin, cy + um * sin + vm * cos);
    if rx.max(ry) / rx.min(ry) <= CIRCLE_ASPECT {
        return Some(kasa_circle(rs).unwrap_or(Shape::Circle {
            center,
            radius: (rx + ry) / 2.0,
        }));
    }
    Some(Shape::Ellipse {
        center,
        rx,
        ry,
        angle: theta,
    })
}

/// Ajustement algébrique d'un cercle (Kåsa), robuste aux tracés qui n'en font pas tout le tour.
fn kasa_circle(pts: &[Point2]) -> Option<Shape> {
    let n = pts.len() as f64;
    let (mx, my) = (
        pts.iter().map(|p| f64::from(p.x)).sum::<f64>() / n,
        pts.iter().map(|p| f64::from(p.y)).sum::<f64>() / n,
    );
    // Système normal de x² + y² = 2ax + 2by + c, en coordonnées centrées.
    let mut m = [[0.0_f64; 3]; 3];
    let mut rhs = [0.0_f64; 3];
    for p in pts {
        let (x, y) = (f64::from(p.x) - mx, f64::from(p.y) - my);
        let row = [x, y, 1.0];
        let target = x * x + y * y;
        for i in 0..3 {
            for j in 0..3 {
                m[i][j] += row[i] * row[j];
            }
            rhs[i] += row[i] * target;
        }
    }
    let [a2, b2, c] = solve3(m, rhs)?;
    let (a, b) = (a2 / 2.0, b2 / 2.0);
    let r2 = c + a * a + b * b;
    if r2 <= 0.0 {
        return None;
    }
    Some(Shape::Circle {
        center: Point2::new((mx + a) as f32, (my + b) as f32),
        radius: r2.sqrt() as f32,
    })
}

/// Résolution d'un système 3×3 par élimination de Gauss avec pivot partiel.
fn solve3(mut m: [[f64; 3]; 3], mut rhs: [f64; 3]) -> Option<[f64; 3]> {
    for col in 0..3 {
        let pivot = (col..3).max_by(|&i, &j| m[i][col].abs().total_cmp(&m[j][col].abs()))?;
        if m[pivot][col].abs() < 1e-12 {
            return None;
        }
        m.swap(col, pivot);
        rhs.swap(col, pivot);
        for row in col + 1..3 {
            let f = m[row][col] / m[col][col];
            let pivot_row = m[col];
            for (value, pivot_value) in m[row].iter_mut().zip(pivot_row).skip(col) {
                *value -= f * pivot_value;
            }
            rhs[row] -= f * rhs[col];
        }
    }
    let mut x = [0.0; 3];
    for row in (0..3).rev() {
        let tail: f64 = (row + 1..3).map(|k| m[row][k] * x[k]).sum();
        x[row] = (rhs[row] - tail) / m[row][row];
    }
    Some(x)
}

/// Rectangle d'aire minimale contenant le tracé ; carré si les côtés sont presque égaux.
fn fit_rectangle(rs: &[Point2]) -> Shape {
    let extents = |theta: f32| {
        let (sin, cos) = theta.sin_cos();
        let (mut umin, mut umax, mut vmin, mut vmax) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for p in rs {
            let (u, v) = (p.x * cos + p.y * sin, -p.x * sin + p.y * cos);
            umin = umin.min(u);
            umax = umax.max(u);
            vmin = vmin.min(v);
            vmax = vmax.max(v);
        }
        (
            umax - umin,
            vmax - vmin,
            (umin + umax) / 2.0,
            (vmin + vmax) / 2.0,
        )
    };
    let area = |theta: f32| {
        let (w, h, _, _) = extents(theta);
        w * h
    };
    // Recherche grossière tous les degrés sur un quart de tour, puis affinée au dixième de degré.
    let coarse = (0..90)
        .map(|k| k as f32 * PI / 180.0)
        .min_by(|a, b| area(*a).total_cmp(&area(*b)))
        .unwrap_or(0.0);
    let theta = (-10..=10)
        .map(|k| coarse + k as f32 * 0.1 * PI / 180.0)
        .min_by(|a, b| area(*a).total_cmp(&area(*b)))
        .unwrap_or(coarse);
    let (w, h, um, vm) = extents(theta);
    let (sin, cos) = theta.sin_cos();
    let center = Point2::new(um * cos - vm * sin, um * sin + vm * cos);
    // Un rectangle presque aligné sur les axes l'est exactement.
    let quarter = (theta / FRAC_PI_2).round();
    let angle = if (theta - quarter * FRAC_PI_2).abs() < AXIS_SNAP {
        quarter * FRAC_PI_2
    } else {
        theta
    };
    if (w - h).abs() / w.max(h) < SQUARE_ASPECT {
        Shape::Square {
            center,
            side: (w + h) / 2.0,
            angle,
        }
    } else {
        Shape::Rectangle {
            center,
            width: w,
            height: h,
            angle,
        }
    }
}

// ---- Application à un trait -------------------------------------------------------------

/// Trait remplacé par une forme parfaite.
#[derive(Clone, Debug)]
pub struct Recognized {
    pub stroke: Stroke,
    pub kind: ShapeKind,
}

/// Remplace un trait par la forme qu'il évoque : même pinceau, pression moyenne constante.
///
/// `allow_curve` : accepte aussi le simple lissage d'un tracé qui n'est aucune forme précise
/// (à réserver à une demande explicite, sous peine de lisser toute l'écriture).
pub fn recognize_stroke(stroke: &Stroke, allow_curve: bool) -> Option<Recognized> {
    let points: Vec<Point2> = stroke.points.iter().map(|p| p.pos).collect();
    let shape = recognize(&points)?;
    if shape.kind() == ShapeKind::Curve && !allow_curve {
        return None;
    }
    let pressure =
        stroke.points.iter().map(|p| p.pressure).sum::<f32>() / stroke.points.len() as f32;
    let mut out = Stroke::new(Brush { ..stroke.brush });
    for pos in shape.outline() {
        out.push(StrokePoint {
            pos,
            pressure,
            tilt: None,
        });
    }
    Some(Recognized {
        stroke: out,
        kind: shape.kind(),
    })
}

#[cfg(test)]
mod tests;
