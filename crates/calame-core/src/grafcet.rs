//! Grafcet (norme CEI 60848) : étapes, transitions, actions, liaisons, divergences et renvois.
//!
//! Chaque symbole est un élément de page. Sa géométrie est décrite par des primitives
//! ([`Prim`]) : l'écran et l'export SVG dessinent exactement la même chose.
//! [`place`] et [`line_symbol`] portent l'aide à la construction (grille, alignement, liaisons
//! automatiques, numérotation).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::geometry::{dist_to_segment, Color, Point2, Rect2, Similarity};
use crate::item::Item;

/// Côté d'une étape, en unités de page.
pub const STEP_SIZE: f32 = 40.0;
/// Écart entre le carré d'une étape initiale et son cadre extérieur.
pub const INITIAL_MARGIN: f32 = 5.0;
/// Pas de la grille d'aimantation.
pub const GRID: f32 = 10.0;
pub const DEFAULT_FONT: f32 = 16.0;
pub const DEFAULT_LINE: f32 = 2.0;

const TRANSITION_WIDTH: f32 = 24.0;
const TRANSITION_THICKNESS: f32 = 4.0;
const ACTION_STUB: f32 = 16.0;
const ACTION_MIN_WIDTH: f32 = 56.0;
const ACTION_MIN_HEIGHT: f32 = 30.0;
const TEXT_GAP: f32 = 10.0;
const JUMP_LENGTH: f32 = 26.0;
const ARROW_HEAD: f32 = 7.0;
const AND_GAP: f32 = 5.0;
/// Longueur minimale d'une liaison tracée à la main.
const MIN_LINK: f32 = 8.0;
/// Distance d'accroche d'une liaison à un point d'attache.
const PORT_SNAP: f32 = 14.0;
/// Écart horizontal toléré pour aligner un nouveau symbole sous un autre.
const ALIGN_RANGE: f32 = 40.0;
/// Distance verticale au-delà de laquelle un symbole n'est plus relié à celui du dessus.
const CHAIN_REACH: f32 = 140.0;
/// Distance minimale entre deux symboles reliés.
const MIN_GAP: f32 = 20.0;
/// Distance minimale entre deux étapes : la transition s'insère entre les deux.
const STEP_TO_STEP_GAP: f32 = 40.0;
/// Écart horizontal maximal entre une étape et l'action qu'on lui accroche.
const ACTION_REACH: f32 = 160.0;

/// Le numéro de chaque variante est enregistré avec les pages : on n'ajoute qu'à la fin.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrafcetKind {
    Step,
    InitialStep,
    Transition,
    Action,
    /// Liaison simple (droite).
    Link,
    /// Liaison orientée : flèche à son extrémité.
    Arrow,
    /// Divergence ou convergence en OU : un trait horizontal.
    Or,
    /// Divergence ou convergence en ET : un double trait horizontal.
    And,
    /// Renvoi vers une étape : flèche, numéro de l'étape visée dessous.
    Jump,
}

impl GrafcetKind {
    /// Le symbole porte-t-il un texte (numéro, réceptivité, action) ?
    pub fn has_text(self) -> bool {
        matches!(
            self,
            Self::Step | Self::InitialStep | Self::Transition | Self::Action | Self::Jump
        )
    }

    pub fn is_step(self) -> bool {
        matches!(self, Self::Step | Self::InitialStep)
    }

    /// Symbole qui se place d'un clic (les autres se tracent en glissant).
    pub fn is_point(self) -> bool {
        matches!(
            self,
            Self::Step | Self::InitialStep | Self::Transition | Self::Action | Self::Jump
        )
    }
}

/// Un symbole de grafcet posé sur la page.
///
/// `pos` est le centre d'une étape ou d'une transition, le début du trait d'une action, la pointe
/// d'un renvoi ou le début d'une liaison ; `end` n'a de sens que pour une liaison ou une
/// divergence. Les dimensions du texte (`text_width`, `text_height`) sont mesurées par
/// l'interface, déjà multipliées par `scale`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GrafcetSymbol {
    pub id: Uuid,
    pub kind: GrafcetKind,
    pub pos: Point2,
    pub end: Point2,
    pub text: String,
    pub color: Color,
    pub line_width: f32,
    pub font_size: f32,
    /// Agrandissement du symbole entier (la sélection peut le redimensionner).
    pub scale: f32,
    pub text_width: f32,
    pub text_height: f32,
}

/// Où le texte s'accroche à son point d'ancrage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    Center,
    LeftMiddle,
}

/// Élément de dessin d'un symbole.
#[derive(Clone, Debug, PartialEq)]
pub enum Prim {
    Line {
        a: Point2,
        b: Point2,
        width: f32,
    },
    /// Contour d'un rectangle.
    Rect {
        min: Point2,
        max: Point2,
        width: f32,
    },
    /// Rectangle plein (barre d'une transition).
    Filled {
        min: Point2,
        max: Point2,
    },
    Text {
        at: Point2,
        anchor: Anchor,
        text: String,
        /// Corps du texte, déjà mis à l'échelle.
        size: f32,
        /// Dimensions mesurées du texte.
        width: f32,
        height: f32,
    },
}

impl GrafcetSymbol {
    pub fn new(kind: GrafcetKind, pos: Point2, text: String, color: Color) -> Self {
        let mut symbol = Self {
            id: Uuid::new_v4(),
            kind,
            pos,
            end: pos,
            text,
            color,
            line_width: DEFAULT_LINE,
            font_size: DEFAULT_FONT,
            scale: 1.0,
            text_width: 0.0,
            text_height: 0.0,
        };
        symbol.estimate_text();
        symbol
    }

    /// Liaison ou divergence de `from` à `to`.
    pub fn line(kind: GrafcetKind, from: Point2, to: Point2, color: Color) -> Self {
        let mut symbol = Self::new(kind, from, String::new(), color);
        symbol.end = to;
        symbol
    }

    /// Dimensions estimées du texte, en attendant la mesure de l'interface.
    pub fn estimate_text(&mut self) {
        let lines = self.text.split('\n');
        let count = lines.clone().count().max(1) as f32;
        let longest = lines.map(|l| l.chars().count()).max().unwrap_or(0) as f32;
        let font = self.font_size * self.scale;
        self.text_width = longest * font * 0.55;
        self.text_height = count * font * 1.25;
    }

    /// Demi-côté de l'étape, cadre extérieur compris.
    fn step_half(&self) -> f32 {
        let margin = if self.kind == GrafcetKind::InitialStep {
            INITIAL_MARGIN
        } else {
            0.0
        };
        (STEP_SIZE / 2.0 + margin) * self.scale
    }

    /// Point d'attache supérieur d'une étape ou d'une transition.
    pub fn top_port(&self) -> Option<Point2> {
        self.vertical_half()
            .map(|h| Point2::new(self.pos.x, self.pos.y - h))
    }

    /// Point d'attache inférieur d'une étape ou d'une transition.
    pub fn bottom_port(&self) -> Option<Point2> {
        self.vertical_half()
            .map(|h| Point2::new(self.pos.x, self.pos.y + h))
    }

    fn vertical_half(&self) -> Option<f32> {
        match self.kind {
            GrafcetKind::Step | GrafcetKind::InitialStep => Some(self.step_half()),
            GrafcetKind::Transition => Some(TRANSITION_THICKNESS * self.scale / 2.0),
            _ => None,
        }
    }

    /// Hauteur au-dessus de `pos` jusqu'au point d'arrivée d'une liaison venue du dessus.
    fn reach_above(&self) -> f32 {
        match self.kind {
            GrafcetKind::Jump => JUMP_LENGTH * self.scale,
            _ => self.vertical_half().unwrap_or(0.0),
        }
    }

    /// Côté droit d'une étape, où s'accroche une action.
    pub fn right_port(&self) -> Option<Point2> {
        self.kind
            .is_step()
            .then(|| Point2::new(self.pos.x + self.step_half(), self.pos.y))
    }

    /// Points où une liaison tracée à la main s'accroche.
    pub fn ports(&self) -> Vec<Point2> {
        let mut out: Vec<Point2> = [self.top_port(), self.bottom_port(), self.right_port()]
            .into_iter()
            .flatten()
            .collect();
        match self.kind {
            GrafcetKind::Action => out.push(self.pos),
            GrafcetKind::Link | GrafcetKind::Arrow | GrafcetKind::Or | GrafcetKind::And => {
                out.push(self.pos);
                out.push(self.end);
            }
            GrafcetKind::Jump => out.push(Point2::new(
                self.pos.x,
                self.pos.y - JUMP_LENGTH * self.scale,
            )),
            _ => {}
        }
        out
    }

    /// Dessin du symbole.
    pub fn prims(&self) -> Vec<Prim> {
        let s = self.scale;
        let w = self.line_width * s;
        let font = self.font_size * s;
        let p = self.pos;
        let mut out = Vec::new();
        let text = |at: Point2, anchor: Anchor| Prim::Text {
            at,
            anchor,
            text: self.text.clone(),
            size: font,
            width: self.text_width,
            height: self.text_height,
        };
        match self.kind {
            GrafcetKind::Step | GrafcetKind::InitialStep => {
                let half = STEP_SIZE * s / 2.0;
                out.push(rect_around(p, half, half, w));
                if self.kind == GrafcetKind::InitialStep {
                    let outer = self.step_half();
                    out.push(rect_around(p, outer, outer, w));
                }
                if !self.text.is_empty() {
                    out.push(text(p, Anchor::Center));
                }
            }
            GrafcetKind::Transition => {
                let (hw, hh) = (TRANSITION_WIDTH * s / 2.0, TRANSITION_THICKNESS * s / 2.0);
                out.push(Prim::Filled {
                    min: Point2::new(p.x - hw, p.y - hh),
                    max: Point2::new(p.x + hw, p.y + hh),
                });
                if !self.text.is_empty() {
                    out.push(text(
                        Point2::new(p.x + hw + TEXT_GAP * s, p.y),
                        Anchor::LeftMiddle,
                    ));
                }
            }
            GrafcetKind::Action => {
                let stub_end = Point2::new(p.x + ACTION_STUB * s, p.y);
                let width = (ACTION_MIN_WIDTH * s).max(self.text_width + 16.0 * s);
                let height = (ACTION_MIN_HEIGHT * s).max(self.text_height + 10.0 * s);
                out.push(Prim::Line {
                    a: p,
                    b: stub_end,
                    width: w,
                });
                let min = Point2::new(stub_end.x, p.y - height / 2.0);
                let max = Point2::new(stub_end.x + width, p.y + height / 2.0);
                out.push(Prim::Rect { min, max, width: w });
                if !self.text.is_empty() {
                    out.push(text(min.lerp(max, 0.5), Anchor::Center));
                }
            }
            GrafcetKind::Link => out.push(Prim::Line {
                a: p,
                b: self.end,
                width: w,
            }),
            GrafcetKind::Arrow => {
                out.push(Prim::Line {
                    a: p,
                    b: self.end,
                    width: w,
                });
                arrow_head(&mut out, p, self.end, w, s);
            }
            GrafcetKind::Or => out.push(Prim::Line {
                a: p,
                b: self.end,
                width: w,
            }),
            GrafcetKind::And => {
                let gap = AND_GAP * s / 2.0;
                for sign in [-1.0, 1.0] {
                    out.push(Prim::Line {
                        a: Point2::new(p.x, p.y + sign * gap),
                        b: Point2::new(self.end.x, self.end.y + sign * gap),
                        width: w,
                    });
                }
            }
            GrafcetKind::Jump => {
                let tail = Point2::new(p.x, p.y - JUMP_LENGTH * s);
                out.push(Prim::Line {
                    a: tail,
                    b: p,
                    width: w,
                });
                arrow_head(&mut out, tail, p, w, s);
                if !self.text.is_empty() {
                    out.push(text(
                        Point2::new(p.x, p.y + 6.0 * s + self.text_height / 2.0),
                        Anchor::Center,
                    ));
                }
            }
        }
        out
    }

    pub fn bounds(&self) -> Option<Rect2> {
        self.prims().iter().map(prim_bounds).reduce(Rect2::union)
    }

    /// `true` si `p` est à moins de `radius` du symbole.
    pub fn hit(&self, p: Point2, radius: f32) -> bool {
        self.prims().iter().any(|prim| match prim {
            Prim::Line { a, b, width } => dist_to_segment(p, *a, *b) <= width / 2.0 + radius + 2.0,
            other => prim_bounds(other).expand(radius).contains(p),
        })
    }

    pub fn transformed(&self, t: &Similarity) -> Self {
        let mut out = self.clone();
        // Les symboles restent droits : seules leur position et leur taille suivent.
        out.pos = t.apply(self.pos);
        out.end = t.apply(self.end);
        out.scale *= t.scale;
        out.text_width *= t.scale;
        out.text_height *= t.scale;
        out
    }

    /// Coin haut-gauche où placer le champ de saisie du texte.
    pub fn text_origin(&self) -> Point2 {
        let s = self.scale;
        let half_line = self.font_size * s * 0.65;
        let p = self.pos;
        match self.kind {
            GrafcetKind::Step | GrafcetKind::InitialStep => {
                Point2::new(p.x - STEP_SIZE * s * 0.35, p.y - half_line)
            }
            GrafcetKind::Transition => Point2::new(
                p.x + (TRANSITION_WIDTH / 2.0 + TEXT_GAP) * s,
                p.y - half_line,
            ),
            GrafcetKind::Action => Point2::new(p.x + (ACTION_STUB + 4.0) * s, p.y - half_line),
            _ => Point2::new(p.x - STEP_SIZE * s * 0.35, p.y + 4.0 * s),
        }
    }

    /// Remplace le texte et réestime ses dimensions.
    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.estimate_text();
    }
}

fn rect_around(center: Point2, hw: f32, hh: f32, width: f32) -> Prim {
    Prim::Rect {
        min: Point2::new(center.x - hw, center.y - hh),
        max: Point2::new(center.x + hw, center.y + hh),
        width,
    }
}

/// Pointe de flèche en `to`, pour un segment qui va de `from` à `to`.
fn arrow_head(out: &mut Vec<Prim>, from: Point2, to: Point2, width: f32, scale: f32) {
    let len = from.distance(to);
    if len <= f32::EPSILON {
        return;
    }
    let (dx, dy) = ((to.x - from.x) / len, (to.y - from.y) / len);
    let head = ARROW_HEAD * scale;
    let base = Point2::new(to.x - dx * head, to.y - dy * head);
    for sign in [-1.0, 1.0] {
        out.push(Prim::Line {
            a: to,
            b: Point2::new(
                base.x - dy * head * 0.5 * sign,
                base.y + dx * head * 0.5 * sign,
            ),
            width,
        });
    }
}

fn prim_bounds(prim: &Prim) -> Rect2 {
    let (min, max) = match prim {
        Prim::Line { a, b, width } => {
            let half = width / 2.0;
            return Rect2 {
                min: Point2::new(a.x.min(b.x), a.y.min(b.y)),
                max: Point2::new(a.x.max(b.x), a.y.max(b.y)),
            }
            .expand(half);
        }
        Prim::Rect { min, max, width } => {
            return Rect2 {
                min: *min,
                max: *max,
            }
            .expand(width / 2.0);
        }
        Prim::Filled { min, max } => (*min, *max),
        Prim::Text {
            at,
            anchor,
            width,
            height,
            ..
        } => match anchor {
            Anchor::Center => (
                Point2::new(at.x - width / 2.0, at.y - height / 2.0),
                Point2::new(at.x + width / 2.0, at.y + height / 2.0),
            ),
            Anchor::LeftMiddle => (
                Point2::new(at.x, at.y - height / 2.0),
                Point2::new(at.x + width, at.y + height / 2.0),
            ),
        },
    };
    Rect2 { min, max }
}

// ---- Construction assistée ----------------------------------------------------------------

fn snap_grid(v: f32) -> f32 {
    (v / GRID).round() * GRID
}

fn grafcet_symbols(items: &[Item]) -> impl Iterator<Item = &GrafcetSymbol> {
    items.iter().filter_map(|item| match item {
        Item::Grafcet(g) => Some(g),
        _ => None,
    })
}

/// Numéro à donner à la prochaine étape : le plus grand numéro de la page plus un (1 s'il n'y en
/// a pas). L'étape initiale prend 0 tant qu'il est libre.
pub fn next_step_number(items: &[Item], initial: bool) -> String {
    let numbers: Vec<u32> = grafcet_symbols(items)
        .filter(|g| g.kind.is_step())
        .filter_map(|g| g.text.trim().parse().ok())
        .collect();
    if initial && !numbers.contains(&0) {
        return "0".into();
    }
    (numbers.into_iter().max().map_or(1, |m| m + 1)).to_string()
}

/// Un lien part-il déjà du point `port` ?
fn has_link_at(items: &[Item], port: Point2) -> bool {
    grafcet_symbols(items).any(|g| {
        matches!(g.kind, GrafcetKind::Link | GrafcetKind::Arrow)
            && (g.pos.distance(port) < 3.0 || g.end.distance(port) < 3.0)
    })
}

/// Symboles à ajouter pour un clic en `at` avec l'outil `kind` ; le symbole cliqué est le dernier.
///
/// - tout est aimanté à la grille ;
/// - une étape, une transition ou un renvoi posé sous une étape ou une transition s'aligne
///   dessus et lui est relié ;
/// - une étape sous une étape reçoit entre les deux la transition qui manque ;
/// - une action cliquée à droite d'une étape s'accroche à son côté.
pub fn place(kind: GrafcetKind, at: Point2, items: &[Item], color: Color) -> Vec<GrafcetSymbol> {
    let mut p = Point2::new(snap_grid(at.x), snap_grid(at.y));
    let label = match kind {
        GrafcetKind::Step | GrafcetKind::InitialStep => {
            next_step_number(items, kind == GrafcetKind::InitialStep)
        }
        _ => String::new(),
    };
    let mut symbol = GrafcetSymbol::new(kind, p, label, color);
    let mut out = Vec::new();

    match kind {
        GrafcetKind::Step
        | GrafcetKind::InitialStep
        | GrafcetKind::Transition
        | GrafcetKind::Jump => {
            let above = grafcet_symbols(items)
                .filter_map(|g| g.bottom_port().map(|port| (g, port)))
                .filter(|(g, port)| {
                    (g.pos.x - p.x).abs() <= ALIGN_RANGE
                        && port.y <= p.y
                        && p.y - port.y <= CHAIN_REACH
                        && !has_link_at(items, *port)
                })
                .min_by(|(_, a), (_, b)| (p.y - a.y).total_cmp(&(p.y - b.y)));
            if let Some((neighbor, bottom)) = above {
                p.x = neighbor.pos.x;
                symbol.pos = p;
                let half = symbol.reach_above();
                let both_steps = neighbor.kind.is_step() && kind.is_step();
                let min_gap = if both_steps {
                    STEP_TO_STEP_GAP
                } else {
                    MIN_GAP
                };
                // Le haut du nouveau symbole ne peut pas être plus près que `min_gap`.
                let top = (p.y - half).max(bottom.y + min_gap);
                symbol.pos.y = if both_steps {
                    // Arrondi vers le bas de la page : l'écart minimal est respecté.
                    ((top + half) / GRID).ceil() * GRID
                } else {
                    top + half
                };
                let top = symbol.pos.y - half;
                let tail = if both_steps {
                    // Étape sous étape : la transition manquante s'insère au milieu.
                    let mid = snap_grid(bottom.y + (top - bottom.y) / 2.0);
                    let transition = GrafcetSymbol::new(
                        GrafcetKind::Transition,
                        Point2::new(p.x, mid),
                        String::new(),
                        color,
                    );
                    let (above_bar, below_bar) = (
                        transition.top_port().unwrap_or(bottom),
                        transition.bottom_port().unwrap_or(bottom),
                    );
                    out.push(transition);
                    out.push(GrafcetSymbol::line(
                        GrafcetKind::Link,
                        bottom,
                        above_bar,
                        color,
                    ));
                    below_bar
                } else {
                    bottom
                };
                out.push(GrafcetSymbol::line(
                    GrafcetKind::Link,
                    tail,
                    Point2::new(p.x, top),
                    color,
                ));
            }
        }
        GrafcetKind::Action => {
            let step = grafcet_symbols(items)
                .filter_map(|g| g.right_port().map(|port| (g, port)))
                .filter(|(g, port)| {
                    let dx = at.x - port.x;
                    (-STEP_SIZE..=ACTION_REACH).contains(&dx) && (at.y - g.pos.y).abs() <= 30.0
                })
                .min_by(|(_, a), (_, b)| (at.y - a.y).abs().total_cmp(&(at.y - b.y).abs()));
            if let Some((_, port)) = step {
                symbol.pos = port;
            }
        }
        _ => {}
    }
    out.push(symbol);
    out
}

/// Liaison ou divergence tracée en glissant de `from` à `to`. Une liaison est droite, verticale ou
/// horizontale ; une divergence est horizontale. Les extrémités s'accrochent aux points d'attache
/// proches, sinon à la grille. `None` si le tracé est trop court.
pub fn line_symbol(
    kind: GrafcetKind,
    from: Point2,
    to: Point2,
    items: &[Item],
    color: Color,
) -> Option<GrafcetSymbol> {
    let ports: Vec<Point2> = grafcet_symbols(items)
        .flat_map(GrafcetSymbol::ports)
        .collect();
    let near = |p: Point2| {
        ports
            .iter()
            .copied()
            .filter(|q| q.distance(p) <= PORT_SNAP)
            .min_by(|a, b| a.distance(p).total_cmp(&b.distance(p)))
    };
    let start = near(from).unwrap_or_else(|| Point2::new(snap_grid(from.x), snap_grid(from.y)));
    let horizontal = matches!(kind, GrafcetKind::Or | GrafcetKind::And)
        || (to.x - start.x).abs() > (to.y - start.y).abs();
    let end = if horizontal {
        let x = ports
            .iter()
            .filter(|q| (q.y - start.y).abs() < 2.0 && (q.x - to.x).abs() <= PORT_SNAP)
            .map(|q| q.x)
            .next()
            .unwrap_or_else(|| snap_grid(to.x));
        Point2::new(x, start.y)
    } else {
        let y = ports
            .iter()
            .filter(|q| (q.x - start.x).abs() < 2.0 && (q.y - to.y).abs() <= PORT_SNAP)
            .map(|q| q.y)
            .next()
            .unwrap_or_else(|| snap_grid(to.y));
        Point2::new(start.x, y)
    };
    (start.distance(end) >= MIN_LINK).then(|| GrafcetSymbol::line(kind, start, end, color))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    fn page(symbols: &[GrafcetSymbol]) -> Vec<Item> {
        symbols.iter().cloned().map(Item::Grafcet).collect()
    }

    fn kinds(symbols: &[GrafcetSymbol]) -> Vec<GrafcetKind> {
        symbols.iter().map(|s| s.kind).collect()
    }

    #[test]
    fn steps_are_numbered_and_the_initial_one_takes_zero() {
        let mut items = Vec::new();
        let first = place(
            GrafcetKind::InitialStep,
            Point2::new(100.0, 100.0),
            &items,
            Color::BLACK,
        );
        assert_eq!(first[0].text, "0");
        items.extend(page(&first));
        let second = place(
            GrafcetKind::Step,
            Point2::new(300.0, 100.0),
            &items,
            Color::BLACK,
        );
        assert_eq!(second[0].text, "1");
        items.extend(page(&second));
        let third = place(
            GrafcetKind::Step,
            Point2::new(500.0, 100.0),
            &items,
            Color::BLACK,
        );
        assert_eq!(third[0].text, "2");
        // Une seconde étape initiale ne reprend pas 0.
        items.extend(page(&third));
        let other = place(
            GrafcetKind::InitialStep,
            Point2::new(700.0, 100.0),
            &items,
            Color::BLACK,
        );
        assert_eq!(other[0].text, "3");
    }

    #[test]
    fn a_step_below_a_step_gets_the_missing_transition_and_links() {
        let first = place(
            GrafcetKind::Step,
            Point2::new(103.0, 100.0),
            &[],
            Color::BLACK,
        );
        let items = page(&first);
        let next = place(
            GrafcetKind::Step,
            Point2::new(120.0, 230.0),
            &items,
            Color::BLACK,
        );
        assert_eq!(
            kinds(&next),
            [
                GrafcetKind::Transition,
                GrafcetKind::Link,
                GrafcetKind::Link,
                GrafcetKind::Step
            ]
        );
        // Tout est aligné sur la colonne de la première étape.
        assert!(next.iter().all(|s| s.pos.x == first[0].pos.x));
        let (transition, upper, lower, step) = (&next[0], &next[1], &next[2], &next[3]);
        // La première liaison part du bas de l'étape, la dernière arrive en haut de la nouvelle.
        assert_eq!(upper.pos, first[0].bottom_port().expect("point d'attache"));
        assert_eq!(lower.end, step.top_port().expect("point d'attache"));
        assert!(transition.pos.y > upper.pos.y && transition.pos.y < step.pos.y);
        assert!(
            upper.end.y < lower.pos.y,
            "la barre sépare les deux liaisons"
        );
    }

    #[test]
    fn a_transition_below_a_step_is_linked_but_not_doubled() {
        let step = place(
            GrafcetKind::Step,
            Point2::new(100.0, 100.0),
            &[],
            Color::BLACK,
        );
        let mut items = page(&step);
        let transition = place(
            GrafcetKind::Transition,
            Point2::new(100.0, 170.0),
            &items,
            Color::BLACK,
        );
        assert_eq!(
            kinds(&transition),
            [GrafcetKind::Link, GrafcetKind::Transition]
        );
        items.extend(page(&transition));
        // Le bas de l'étape est déjà relié : une seconde transition est posée seule (branche).
        let branch = place(
            GrafcetKind::Transition,
            Point2::new(100.0, 190.0),
            &items,
            Color::BLACK,
        );
        assert_eq!(
            branch.len(),
            2,
            "relie la transition du dessus, pas l'étape"
        );
        assert_eq!(
            branch[0].pos,
            transition[1].bottom_port().expect("point d'attache")
        );
    }

    #[test]
    fn a_jump_below_a_transition_is_linked_to_it() {
        let bar = place(
            GrafcetKind::Transition,
            Point2::new(100.0, 100.0),
            &[],
            Color::BLACK,
        );
        let items = page(&bar);
        let jump = place(
            GrafcetKind::Jump,
            Point2::new(110.0, 180.0),
            &items,
            Color::BLACK,
        );
        assert_eq!(kinds(&jump), [GrafcetKind::Link, GrafcetKind::Jump]);
        assert_eq!(jump[1].pos.x, 100.0);
        assert!(jump[1].ports().contains(&jump[0].end), "arrive à la queue");
    }

    #[test]
    fn far_or_misaligned_symbols_stay_alone() {
        let step = place(
            GrafcetKind::Step,
            Point2::new(100.0, 100.0),
            &[],
            Color::BLACK,
        );
        let items = page(&step);
        let aside = place(
            GrafcetKind::Transition,
            Point2::new(300.0, 190.0),
            &items,
            Color::BLACK,
        );
        assert_eq!(aside.len(), 1);
        let far = place(
            GrafcetKind::Transition,
            Point2::new(100.0, 600.0),
            &items,
            Color::BLACK,
        );
        assert_eq!(far.len(), 1);
    }

    #[test]
    fn an_action_clicked_beside_a_step_snaps_to_its_side() {
        let step = place(
            GrafcetKind::InitialStep,
            Point2::new(100.0, 100.0),
            &[],
            Color::BLACK,
        );
        let items = page(&step);
        let action = place(
            GrafcetKind::Action,
            Point2::new(190.0, 108.0),
            &items,
            Color::BLACK,
        );
        let side = step[0].right_port().expect("point d'attache");
        assert_eq!(action[0].pos, side);
        assert_eq!(side.x, 100.0 + STEP_SIZE / 2.0 + INITIAL_MARGIN);
    }

    #[test]
    fn lines_are_orthogonal_and_snap_to_ports() {
        let step = place(
            GrafcetKind::Step,
            Point2::new(100.0, 100.0),
            &[],
            Color::BLACK,
        );
        let items = page(&step);
        let bottom = step[0].bottom_port().expect("point d'attache");
        let link = line_symbol(
            GrafcetKind::Arrow,
            Point2::new(103.0, bottom.y + 4.0),
            Point2::new(110.0, 197.0),
            &items,
            Color::BLACK,
        )
        .expect("assez long");
        assert_eq!(link.pos, bottom);
        assert_eq!(link.end, Point2::new(bottom.x, 200.0));
        let bar = line_symbol(
            GrafcetKind::And,
            Point2::new(52.0, 300.0),
            Point2::new(251.0, 340.0),
            &items,
            Color::BLACK,
        )
        .expect("assez long");
        assert_eq!(
            (bar.pos, bar.end),
            (Point2::new(50.0, 300.0), Point2::new(250.0, 300.0))
        );
        assert!(line_symbol(
            GrafcetKind::Link,
            Point2::new(0.0, 0.0),
            Point2::new(2.0, 3.0),
            &items,
            Color::BLACK
        )
        .is_none());
    }

    #[test]
    fn hit_and_bounds_follow_the_drawing() {
        let step = GrafcetSymbol::new(
            GrafcetKind::InitialStep,
            Point2::new(0.0, 0.0),
            "0".into(),
            Color::BLACK,
        );
        let b = step.bounds().expect("une étape a une étendue");
        assert!(b.width() >= STEP_SIZE + 2.0 * INITIAL_MARGIN);
        assert!(step.hit(Point2::new(0.0, 0.0), 0.0));
        assert!(!step.hit(Point2::new(60.0, 0.0), 2.0));

        let link = GrafcetSymbol::line(
            GrafcetKind::Link,
            Point2::new(0.0, 0.0),
            Point2::new(0.0, 100.0),
            Color::BLACK,
        );
        assert!(link.hit(Point2::new(1.0, 50.0), 0.0));
        assert!(!link.hit(Point2::new(20.0, 50.0), 2.0));
    }

    #[test]
    fn transform_moves_and_scales_but_keeps_symbols_upright() {
        let step = GrafcetSymbol::new(
            GrafcetKind::Step,
            Point2::new(10.0, 0.0),
            "1".into(),
            Color::BLACK,
        );
        let out = step.transformed(&Similarity {
            pivot: Point2::new(0.0, 0.0),
            scale: 2.0,
            angle: 0.7,
            translate: Point2::new(0.0, 0.0),
        });
        assert_eq!(out.scale, 2.0);
        assert!(out.bounds().expect("étendue").width() > step.bounds().expect("étendue").width());
    }
}
