//! Outils géométriques internes : rééchantillonnage, simplification, distances, lissage.

use calame_core::geometry::dist_to_segment;
use calame_core::{Point2, Rect2};

pub(crate) fn path_length(pts: &[Point2]) -> f32 {
    pts.windows(2).map(|w| w[0].distance(w[1])).sum()
}

pub(crate) fn bbox_diag(pts: &[Point2]) -> f32 {
    Rect2::from_points(pts.iter().copied()).map_or(0.0, |r| r.width().hypot(r.height()))
}

/// Retire les points consécutifs distants de moins de `min_dist`.
pub(crate) fn dedup(points: &[Point2], min_dist: f32) -> Vec<Point2> {
    let mut out: Vec<Point2> = Vec::with_capacity(points.len());
    for &p in points {
        if out.last().map_or(true, |last| last.distance(p) >= min_dist) {
            out.push(p);
        }
    }
    out
}

/// `n` points régulièrement espacés le long de la ligne brisée, extrémités comprises.
pub(crate) fn resample(pts: &[Point2], n: usize) -> Vec<Point2> {
    let (Some(&first), Some(&last)) = (pts.first(), pts.last()) else {
        return Vec::new();
    };
    let total = path_length(pts);
    if n < 2 || total <= f32::EPSILON {
        return vec![first; n.max(1)];
    }
    let mut cumulative = Vec::with_capacity(pts.len());
    let mut acc = 0.0;
    cumulative.push(0.0);
    for w in pts.windows(2) {
        acc += w[0].distance(w[1]);
        cumulative.push(acc);
    }
    let step = total / (n - 1) as f32;
    let mut out = Vec::with_capacity(n);
    out.push(first);
    let mut j = 0;
    for k in 1..n - 1 {
        let target = k as f32 * step;
        while j + 2 < pts.len() && cumulative[j + 1] < target {
            j += 1;
        }
        let seg = cumulative[j + 1] - cumulative[j];
        let u = if seg > 0.0 {
            ((target - cumulative[j]) / seg).clamp(0.0, 1.0)
        } else {
            0.0
        };
        out.push(pts[j].lerp(pts[j + 1], u));
    }
    out.push(last);
    out
}

/// Simplification de Douglas-Peucker ; les extrémités sont conservées.
pub(crate) fn simplify_dp(pts: &[Point2], eps: f32) -> Vec<Point2> {
    let n = pts.len();
    if n < 3 {
        return pts.to_vec();
    }
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;
    let mut stack = vec![(0, n - 1)];
    while let Some((s, e)) = stack.pop() {
        let mut worst = (0.0, s);
        for (i, p) in pts.iter().enumerate().take(e).skip(s + 1) {
            let d = dist_to_segment(*p, pts[s], pts[e]);
            if d > worst.0 {
                worst = (d, i);
            }
        }
        if worst.0 > eps {
            keep[worst.1] = true;
            stack.push((s, worst.1));
            stack.push((worst.1, e));
        }
    }
    pts.iter()
        .zip(keep)
        .filter_map(|(p, k)| k.then_some(*p))
        .collect()
}

pub(crate) fn dist_to_polyline(p: Point2, poly: &[Point2]) -> f32 {
    match poly {
        [] => f32::INFINITY,
        [only] => p.distance(*only),
        _ => poly
            .windows(2)
            .map(|w| dist_to_segment(p, w[0], w[1]))
            .fold(f32::INFINITY, f32::min),
    }
}

pub(crate) fn mean_dist(points: &[Point2], poly: &[Point2]) -> f32 {
    if points.is_empty() {
        return f32::INFINITY;
    }
    points
        .iter()
        .map(|p| dist_to_polyline(*p, poly))
        .sum::<f32>()
        / points.len() as f32
}

/// Écart moyen du tracé à un contour, rapporté à la diagonale du tracé.
pub(crate) fn fit_error(stroke: &[Point2], outline: &[Point2], diag: f32) -> f32 {
    mean_dist(stroke, outline) / diag.max(f32::EPSILON)
}

/// Part du contour située à moins de `tol` du tracé : écarte les formes qui n'en couvrent qu'une partie.
pub(crate) fn coverage(outline: &[Point2], stroke: &[Point2], tol: f32) -> f32 {
    let samples = resample(outline, 128);
    if samples.is_empty() {
        return 0.0;
    }
    let near = samples
        .iter()
        .filter(|p| dist_to_polyline(**p, stroke) <= tol)
        .count();
    near as f32 / samples.len() as f32
}

/// Angle de virage en `b` entre les directions `a→b` et `b→c`, dans `[0, π]`.
pub(crate) fn turn_angle(a: Point2, b: Point2, c: Point2) -> f32 {
    let (v1x, v1y) = (b.x - a.x, b.y - a.y);
    let (v2x, v2y) = (c.x - b.x, c.y - b.y);
    (v1x * v2y - v1y * v2x).atan2(v1x * v2x + v1y * v2y).abs()
}

/// Retire d'un contour fermé les sommets quasi alignés ou trop proches d'un voisin.
pub(crate) fn clean_closed(mut v: Vec<Point2>, min_turn: f32, min_edge: f32) -> Vec<Point2> {
    while v.len() > 3 {
        let n = v.len();
        let turns: Vec<f32> = (0..n)
            .map(|i| turn_angle(v[(i + n - 1) % n], v[i], v[(i + 1) % n]))
            .collect();
        let (flattest, min) =
            turns
                .iter()
                .copied()
                .enumerate()
                .fold(
                    (0, f32::INFINITY),
                    |acc, (i, t)| if t < acc.1 { (i, t) } else { acc },
                );
        if min < min_turn {
            v.remove(flattest);
            continue;
        }
        // Deux sommets trop proches : on retire celui qui vire le moins.
        let close = (0..n).find(|&i| v[i].distance(v[(i + 1) % n]) < min_edge);
        match close {
            Some(i) => {
                let j = (i + 1) % n;
                v.remove(if turns[i] < turns[j] { i } else { j });
            }
            None => break,
        }
    }
    v
}

/// Retire d'une ligne brisée ouverte les sommets intérieurs quasi alignés.
pub(crate) fn clean_open(mut v: Vec<Point2>, min_turn: f32) -> Vec<Point2> {
    loop {
        if v.len() <= 2 {
            return v;
        }
        let (idx, min) = (1..v.len() - 1)
            .map(|i| (i, turn_angle(v[i - 1], v[i], v[i + 1])))
            .fold(
                (0, f32::INFINITY),
                |acc, (i, t)| if t < acc.1 { (i, t) } else { acc },
            );
        if min >= min_turn {
            return v;
        }
        v.remove(idx);
    }
}

/// Les segments `ab` et `cd` se coupent-ils (hors extrémités communes) ?
fn segments_cross(a: Point2, b: Point2, c: Point2, d: Point2) -> bool {
    let orient =
        |p: Point2, q: Point2, r: Point2| (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x);
    let (o1, o2) = (orient(a, b, c), orient(a, b, d));
    let (o3, o4) = (orient(c, d, a), orient(c, d, b));
    (o1 > 0.0) != (o2 > 0.0)
        && (o3 > 0.0) != (o4 > 0.0)
        && o1 != 0.0
        && o2 != 0.0
        && o3 != 0.0
        && o4 != 0.0
}

/// Polygone fermé sans auto-intersection.
pub(crate) fn is_simple_polygon(v: &[Point2]) -> bool {
    let n = v.len();
    for i in 0..n {
        for j in i + 1..n {
            let adjacent = j == i + 1 || (i == 0 && j == n - 1);
            if !adjacent && segments_cross(v[i], v[(i + 1) % n], v[j], v[(j + 1) % n]) {
                return false;
            }
        }
    }
    true
}

/// Lissage par un noyau binomial `[1 4 6 4 1] / 16`, répété `passes` fois.
/// Un tracé ouvert garde ses extrémités.
pub(crate) fn smooth_path(pts: &[Point2], closed: bool, passes: usize) -> Vec<Point2> {
    const KERNEL: [f32; 5] = [1.0, 4.0, 6.0, 4.0, 1.0];
    let n = pts.len();
    let mut cur = pts.to_vec();
    if n < 5 {
        return cur;
    }
    for _ in 0..passes {
        let mut next = Vec::with_capacity(n);
        for i in 0..n {
            let (mut x, mut y) = (0.0, 0.0);
            for (k, w) in KERNEL.iter().enumerate() {
                let offset = k as isize - 2;
                let idx = if closed {
                    (i as isize + offset).rem_euclid(n as isize) as usize
                } else {
                    (i as isize + offset).clamp(0, n as isize - 1) as usize
                };
                x += w * cur[idx].x;
                y += w * cur[idx].y;
            }
            next.push(Point2::new(x / 16.0, y / 16.0));
        }
        if !closed {
            next[0] = pts[0];
            next[n - 1] = pts[n - 1];
        }
        cur = next;
    }
    cur
}
