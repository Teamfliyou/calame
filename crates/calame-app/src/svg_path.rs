//! Lecture des tracés SVG (`d="M4 20l1.2-4.4…"`) des maquettes, aplatis en lignes brisées.
//!
//! Seules les commandes utiles aux icônes sont reconnues : M, L, H, V, C, S, Q, T, A et Z, en
//! absolu comme en relatif. Les courbes et les arcs sont découpés en segments.

/// Une ligne brisée du tracé, en coordonnées de la `viewBox`.
#[derive(Clone, Debug, PartialEq)]
pub struct Polyline {
    pub points: Vec<(f32, f32)>,
    pub closed: bool,
}

const CURVE_STEPS: usize = 10;

struct Tokens<'a> {
    s: &'a [u8],
    i: usize,
}

impl Tokens<'_> {
    fn skip_separators(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b',' | b'\n' | b'\t' | b'\r')
        {
            self.i += 1;
        }
    }

    fn command(&mut self) -> Option<u8> {
        self.skip_separators();
        let c = *self.s.get(self.i)?;
        if c.is_ascii_alphabetic() {
            self.i += 1;
            Some(c)
        } else {
            None
        }
    }

    fn has_number(&mut self) -> bool {
        self.skip_separators();
        self.s
            .get(self.i)
            .is_some_and(|c| c.is_ascii_digit() || matches!(c, b'-' | b'+' | b'.'))
    }

    fn number(&mut self) -> Option<f32> {
        self.skip_separators();
        let start = self.i;
        if matches!(self.s.get(self.i), Some(b'-' | b'+')) {
            self.i += 1;
        }
        let mut dot = false;
        while let Some(&c) = self.s.get(self.i) {
            if c.is_ascii_digit() {
                self.i += 1;
            } else if c == b'.' && !dot {
                dot = true;
                self.i += 1;
            } else if matches!(c, b'e' | b'E') {
                self.i += 1;
                if matches!(self.s.get(self.i), Some(b'-' | b'+')) {
                    self.i += 1;
                }
            } else {
                break;
            }
        }
        std::str::from_utf8(&self.s[start..self.i])
            .ok()?
            .parse()
            .ok()
    }

    /// Drapeau d'un arc : un seul chiffre, éventuellement collé au suivant (« 0 01 2 »).
    fn flag(&mut self) -> Option<bool> {
        self.skip_separators();
        let c = *self.s.get(self.i)?;
        self.i += 1;
        match c {
            b'0' => Some(false),
            b'1' => Some(true),
            _ => None,
        }
    }
}

/// Points d'un arc elliptique SVG (paramétrage par extrémités), sans le point de départ.
#[allow(clippy::too_many_arguments)]
fn arc_points(
    from: (f32, f32),
    rx: f32,
    ry: f32,
    rotation: f32,
    large: bool,
    sweep: bool,
    to: (f32, f32),
) -> Vec<(f32, f32)> {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx < 1e-6 || ry < 1e-6 || from == to {
        return vec![to];
    }
    let phi = rotation.to_radians();
    let (sin, cos) = phi.sin_cos();
    let dx = (from.0 - to.0) / 2.0;
    let dy = (from.1 - to.1) / 2.0;
    let x1 = cos * dx + sin * dy;
    let y1 = -sin * dx + cos * dy;
    let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lambda > 1.0 {
        rx *= lambda.sqrt();
        ry *= lambda.sqrt();
    }
    let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut coef = (num / den).max(0.0).sqrt();
    if large == sweep {
        coef = -coef;
    }
    let cx1 = coef * rx * y1 / ry;
    let cy1 = -coef * ry * x1 / rx;
    let cx = cos * cx1 - sin * cy1 + (from.0 + to.0) / 2.0;
    let cy = sin * cx1 + cos * cy1 + (from.1 + to.1) / 2.0;
    let angle = |ux: f32, uy: f32, vx: f32, vy: f32| {
        let dot = ux * vx + uy * vy;
        let len = (ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt();
        let a = (dot / len).clamp(-1.0, 1.0).acos();
        if ux * vy - uy * vx < 0.0 {
            -a
        } else {
            a
        }
    };
    let ux = (x1 - cx1) / rx;
    let uy = (y1 - cy1) / ry;
    let vx = (-x1 - cx1) / rx;
    let vy = (-y1 - cy1) / ry;
    let start = angle(1.0, 0.0, ux, uy);
    let mut delta = angle(ux, uy, vx, vy);
    if !sweep && delta > 0.0 {
        delta -= std::f32::consts::TAU;
    } else if sweep && delta < 0.0 {
        delta += std::f32::consts::TAU;
    }
    let steps = ((delta.abs() / std::f32::consts::FRAC_PI_2) * 6.0)
        .ceil()
        .max(2.0) as usize;
    (1..=steps)
        .map(|i| {
            if i == steps {
                return to;
            }
            let t = start + delta * i as f32 / steps as f32;
            let (st, ct) = t.sin_cos();
            (
                cx + cos * rx * ct - sin * ry * st,
                cy + sin * rx * ct + cos * ry * st,
            )
        })
        .collect()
}

fn cubic(p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)) -> Vec<(f32, f32)> {
    (1..=CURVE_STEPS)
        .map(|i| {
            let t = i as f32 / CURVE_STEPS as f32;
            let u = 1.0 - t;
            let a = u * u * u;
            let b = 3.0 * u * u * t;
            let c = 3.0 * u * t * t;
            let d = t * t * t;
            (
                a * p0.0 + b * p1.0 + c * p2.0 + d * p3.0,
                a * p0.1 + b * p1.1 + c * p2.1 + d * p3.1,
            )
        })
        .collect()
}

/// Découpe un tracé SVG en lignes brisées. Un tracé mal formé s'arrête à la première erreur.
pub fn parse(d: &str) -> Vec<Polyline> {
    let mut tokens = Tokens {
        s: d.as_bytes(),
        i: 0,
    };
    let mut out: Vec<Polyline> = Vec::new();
    let mut current: Vec<(f32, f32)> = Vec::new();
    let mut pos = (0.0_f32, 0.0_f32);
    let mut start = pos;
    // Dernier point de contrôle, pour les commandes S et T.
    let mut last_control: Option<(f32, f32)> = None;
    let mut command = b'M';

    let flush = |current: &mut Vec<(f32, f32)>, out: &mut Vec<Polyline>, closed: bool| {
        if current.len() > 1 {
            out.push(Polyline {
                points: std::mem::take(current),
                closed,
            });
        } else {
            current.clear();
        }
    };

    loop {
        if let Some(c) = tokens.command() {
            command = c;
        } else if !tokens.has_number() {
            break;
        }
        let relative = command.is_ascii_lowercase();
        let base = if relative { pos } else { (0.0, 0.0) };
        macro_rules! num {
            () => {
                match tokens.number() {
                    Some(v) => v,
                    None => break,
                }
            };
        }
        match command.to_ascii_uppercase() {
            b'M' => {
                flush(&mut current, &mut out, false);
                let (x, y) = (num!(), num!());
                pos = (base.0 + x, base.1 + y);
                start = pos;
                current.push(pos);
                last_control = None;
                // Les paires suivantes d'un M sont des L.
                command = if relative { b'l' } else { b'L' };
            }
            b'L' => {
                let (x, y) = (num!(), num!());
                pos = (base.0 + x, base.1 + y);
                current.push(pos);
                last_control = None;
            }
            b'H' => {
                let x = num!();
                pos = (if relative { pos.0 + x } else { x }, pos.1);
                current.push(pos);
                last_control = None;
            }
            b'V' => {
                let y = num!();
                pos = (pos.0, if relative { pos.1 + y } else { y });
                current.push(pos);
                last_control = None;
            }
            b'C' => {
                let c1 = (base.0 + num!(), base.1 + num!());
                let c2 = (base.0 + num!(), base.1 + num!());
                let end = (base.0 + num!(), base.1 + num!());
                current.extend(cubic(pos, c1, c2, end));
                last_control = Some(c2);
                pos = end;
            }
            b'S' => {
                let c1 = last_control.map_or(pos, |c| (2.0 * pos.0 - c.0, 2.0 * pos.1 - c.1));
                let c2 = (base.0 + num!(), base.1 + num!());
                let end = (base.0 + num!(), base.1 + num!());
                current.extend(cubic(pos, c1, c2, end));
                last_control = Some(c2);
                pos = end;
            }
            b'Q' | b'T' => {
                let q = if command.eq_ignore_ascii_case(&b'Q') {
                    (base.0 + num!(), base.1 + num!())
                } else {
                    last_control.map_or(pos, |c| (2.0 * pos.0 - c.0, 2.0 * pos.1 - c.1))
                };
                let end = (base.0 + num!(), base.1 + num!());
                let c1 = (
                    pos.0 + 2.0 / 3.0 * (q.0 - pos.0),
                    pos.1 + 2.0 / 3.0 * (q.1 - pos.1),
                );
                let c2 = (
                    end.0 + 2.0 / 3.0 * (q.0 - end.0),
                    end.1 + 2.0 / 3.0 * (q.1 - end.1),
                );
                current.extend(cubic(pos, c1, c2, end));
                last_control = Some(q);
                pos = end;
            }
            b'A' => {
                let (rx, ry, rotation) = (num!(), num!(), num!());
                let Some(large) = tokens.flag() else { break };
                let Some(sweep) = tokens.flag() else { break };
                let end = (base.0 + num!(), base.1 + num!());
                current.extend(arc_points(pos, rx, ry, rotation, large, sweep, end));
                pos = end;
                last_control = None;
            }
            b'Z' => {
                flush(&mut current, &mut out, true);
                pos = start;
                current.push(pos);
                last_control = None;
                // Z ne prend pas d'argument : on attend une nouvelle commande.
                if tokens.command().map(|c| command = c).is_none() {
                    break;
                }
                continue;
            }
            _ => break,
        }
    }
    // Un point isolé après Z (début implicite du sous-tracé suivant) n'est pas dessiné.
    if current.len() > 1 {
        out.push(Polyline {
            points: current,
            closed: false,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: (f32, f32), b: (f32, f32)) -> bool {
        (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
    }

    #[test]
    fn lines_with_compact_numbers_and_relative_moves() {
        let paths = parse("M4 20l1.2-4.4L16 4.8l3.2 3.2L8.4 18.8z M14 7l3 3");
        assert_eq!(paths.len(), 2);
        assert!(paths[0].closed);
        assert_eq!(paths[0].points.len(), 5);
        assert!(close(paths[0].points[1], (5.2, 15.6)));
        assert!(!paths[1].closed);
        assert!(close(paths[1].points[1], (17.0, 10.0)));
    }

    #[test]
    fn horizontal_vertical_and_implicit_lines() {
        let paths = parse("M5 7h14M10 7V5h4v2M7 7l1 12.5h8L17 7");
        assert_eq!(paths.len(), 3);
        assert_eq!(
            paths[1].points,
            vec![(10.0, 7.0), (10.0, 5.0), (14.0, 5.0), (14.0, 7.0)]
        );
        // « M x y x y » : la seconde paire est une ligne.
        let implicit = parse("M1 1 2 2 3 3");
        assert_eq!(implicit[0].points.len(), 3);
    }

    #[test]
    fn arcs_end_on_their_target_and_stay_on_the_circle() {
        // Demi-cercle de rayon 5 centré en (10, 10).
        let paths = parse("M5 10a5 5 0 0 1 10 0");
        let pts = &paths[0].points;
        assert!(close(*pts.last().unwrap_or(&(0.0, 0.0)), (15.0, 10.0)));
        for p in pts {
            let r = ((p.0 - 10.0).powi(2) + (p.1 - 10.0).powi(2)).sqrt();
            assert!((r - 5.0).abs() < 1e-2);
        }
        // Le sens horaire (sweep = 1) passe par le haut de l'écran (y < 10).
        assert!(pts.iter().any(|p| p.1 < 6.0));
    }

    #[test]
    fn compact_arc_flags_are_read() {
        let paths = parse("M4 11a.5.5 0 01.5.5");
        assert!(close(
            *paths[0].points.last().unwrap_or(&(0.0, 0.0)),
            (4.5, 11.5)
        ));
    }

    #[test]
    fn curves_end_on_their_target() {
        let paths = parse("M10 40C38 150 80 190 112 190S190 150 216 40");
        let pts = &paths[0].points;
        assert!(close(pts[CURVE_STEPS], (112.0, 190.0)));
        assert!(close(*pts.last().unwrap_or(&(0.0, 0.0)), (216.0, 40.0)));
    }

    #[test]
    fn a_dot_is_kept_as_a_tiny_segment() {
        let paths = parse("M15.5 9.5h.01");
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].points.len(), 2);
    }

    #[test]
    fn malformed_paths_do_not_panic() {
        for d in ["", "M", "M1", "L2 3", "A1 1 0 2 1 3 3", "M1 1 Z Z Z", "x9"] {
            let _ = parse(d);
        }
    }
}
