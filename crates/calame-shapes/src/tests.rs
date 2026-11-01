#![allow(clippy::expect_used)]

//! Jeu de tests : formes dessinées à main levée (bruitées, tournées, mal fermées) et tracés
//! qui ne doivent pas être pris pour une forme.

use std::f32::consts::{PI, TAU};

use calame_core::{Brush, BrushKind, Color, Point2, Stroke, StrokePoint};

use super::*;

/// Générateur pseudo-aléatoire déterministe, pour des tests reproductibles.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) as f32 / (1u64 << 31) as f32) * 2.0 - 1.0
    }
}

/// Ajoute un bruit de main tremblante : décalage lent (ondulation) plus petit tremblement.
fn shake(points: &[Point2], amplitude: f32, rng: &mut Rng) -> Vec<Point2> {
    let (phase_x, phase_y) = (rng.next() * PI, rng.next() * PI);
    points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let t = i as f32 * 0.21;
            Point2::new(
                p.x + amplitude * (0.6 * (t + phase_x).sin() + 0.4 * rng.next()),
                p.y + amplitude * (0.6 * (t * 1.3 + phase_y).sin() + 0.4 * rng.next()),
            )
        })
        .collect()
}

/// Points le long d'une ligne brisée, espacés d'environ `step` (échantillonnage du stylet).
fn trace(vertices: &[Point2], step: f32) -> Vec<Point2> {
    let mut out = vec![vertices[0]];
    for w in vertices.windows(2) {
        let n = (w[0].distance(w[1]) / step).ceil().max(1.0) as usize;
        for k in 1..=n {
            out.push(w[0].lerp(w[1], k as f32 / n as f32));
        }
    }
    out
}

fn circle_trace(c: Point2, rx: f32, ry: f32, rot: f32, start: f32, turns: f32) -> Vec<Point2> {
    let n = 120;
    (0..=n)
        .map(|i| {
            let t = start + TAU * turns * i as f32 / n as f32;
            let (x, y) = (rx * t.cos(), ry * t.sin());
            Point2::new(
                c.x + x * rot.cos() - y * rot.sin(),
                c.y + x * rot.sin() + y * rot.cos(),
            )
        })
        .collect()
}

fn rect_vertices(c: Point2, w: f32, h: f32, rot: f32, first_corner: usize) -> Vec<Point2> {
    let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
    let mut pts: Vec<Point2> = (0..4)
        .map(|k| {
            let (sx, sy) = corners[(k + first_corner) % 4];
            let (x, y) = (sx * w / 2.0, sy * h / 2.0);
            Point2::new(
                c.x + x * rot.cos() - y * rot.sin(),
                c.y + x * rot.sin() + y * rot.cos(),
            )
        })
        .collect();
    pts.push(pts[0]);
    pts
}

fn kind_of(points: &[Point2]) -> Option<ShapeKind> {
    recognize(points).map(|s| s.kind())
}

const ANGLES: [f32; 8] = [0.0, 0.4, 0.9, 1.4, 2.0, 2.6, 3.3, 5.1];

#[test]
fn circles_are_recognised_at_any_size_and_start() {
    let mut rng = Rng(1);
    for radius in [30.0, 80.0, 200.0] {
        for start in ANGLES {
            // Légèrement ouvert (0,96 tour) ou avec recouvrement (1,04 tour).
            for turns in [0.96, 1.0, 1.04] {
                let pts =
                    circle_trace(Point2::new(300.0, 250.0), radius, radius, 0.0, start, turns);
                let pts = shake(&pts, 0.015 * radius, &mut rng);
                let shape = recognize(&pts);
                let Some(Shape::Circle { center, radius: r }) = shape else {
                    panic!("rayon {radius}, départ {start}, tours {turns} : {shape:?}");
                };
                assert!(
                    (r - radius).abs() < 0.08 * radius,
                    "rayon {r} au lieu de {radius}"
                );
                assert!(center.distance(Point2::new(300.0, 250.0)) < 0.08 * radius);
            }
        }
    }
}

#[test]
fn ellipses_keep_their_axes_and_orientation() {
    let mut rng = Rng(2);
    for rot in [0.0, 0.5, 1.0, 2.2] {
        let pts = circle_trace(Point2::new(0.0, 0.0), 140.0, 70.0, rot, 0.7, 1.0);
        let pts = shake(&pts, 2.0, &mut rng);
        let shape = recognize(&pts);
        let Some(Shape::Ellipse { rx, ry, angle, .. }) = shape else {
            panic!("rotation {rot} : {shape:?}");
        };
        let (long, short) = (rx.max(ry), rx.min(ry));
        assert!(
            (long - 140.0).abs() < 14.0 && (short - 70.0).abs() < 8.0,
            "{rx} {ry}"
        );
        // Orientation de l'axe long, modulo un demi-tour.
        let axis = if rx >= ry { angle } else { angle + PI / 2.0 };
        let diff = (axis - rot).rem_euclid(PI);
        assert!(
            diff.min(PI - diff) < 0.12,
            "orientation {axis} au lieu de {rot}"
        );
    }
}

#[test]
fn rectangles_are_found_whatever_the_rotation_and_first_corner() {
    let mut rng = Rng(3);
    for rot in [0.0, 0.2, 0.6, 1.0, 1.5, 2.1, 2.8, 3.6] {
        for corner in 0..4 {
            let pts = trace(
                &rect_vertices(Point2::new(200.0, 200.0), 180.0, 100.0, rot, corner),
                6.0,
            );
            let pts = shake(&pts, 2.0, &mut rng);
            let kind = kind_of(&pts);
            assert_eq!(
                kind,
                Some(ShapeKind::Rectangle),
                "rotation {rot}, coin {corner}"
            );
        }
    }
}

#[test]
fn squares_are_not_taken_for_circles_or_rectangles() {
    let mut rng = Rng(4);
    for rot in [0.0, 0.3, 0.785, 1.2] {
        let pts = trace(
            &rect_vertices(Point2::new(0.0, 0.0), 120.0, 120.0, rot, 1),
            6.0,
        );
        let pts = shake(&pts, 1.5, &mut rng);
        let shape = recognize(&pts);
        let Some(Shape::Square { side, .. }) = shape else {
            panic!("rotation {rot} : {shape:?}");
        };
        assert!((side - 120.0).abs() < 10.0, "côté {side}");
    }
}

#[test]
fn nearly_axis_aligned_rectangles_snap_to_the_axes() {
    let pts = trace(
        &rect_vertices(Point2::new(0.0, 0.0), 200.0, 100.0, 0.04, 0),
        6.0,
    );
    let Some(Shape::Rectangle { angle, .. }) = recognize(&pts) else {
        panic!("rectangle attendu");
    };
    assert_eq!(angle, 0.0);
}

#[test]
fn triangles_of_various_shapes_are_found_with_their_vertices() {
    let mut rng = Rng(5);
    let triangles = [
        [(0.0, 0.0), (200.0, 0.0), (100.0, 173.0)],
        [(0.0, 0.0), (150.0, 0.0), (0.0, 110.0)],
        [(0.0, 0.0), (220.0, 40.0), (60.0, 150.0)],
    ];
    for tri in triangles {
        for rot in [0.0_f32, 0.8, 2.0, 4.0] {
            let v: Vec<Point2> = tri
                .iter()
                .map(|&(x, y): &(f32, f32)| {
                    Point2::new(
                        100.0 + x * rot.cos() - y * rot.sin(),
                        100.0 + x * rot.sin() + y * rot.cos(),
                    )
                })
                .collect();
            let mut path = v.clone();
            path.push(v[0]);
            let pts = shake(&trace(&path, 6.0), 2.0, &mut rng);
            let shape = recognize(&pts);
            let Some(Shape::Triangle { vertices }) = shape else {
                panic!("{tri:?} tourné de {rot} : {shape:?}");
            };
            for expected in &v {
                let nearest = vertices
                    .iter()
                    .map(|p| p.distance(*expected))
                    .fold(f32::MAX, f32::min);
                assert!(
                    nearest < 20.0,
                    "sommet manquant : {expected:?} (écart {nearest})"
                );
            }
        }
    }
}

#[test]
fn pentagons_become_polygons_with_five_vertices() {
    let mut rng = Rng(6);
    let v: Vec<Point2> = (0..5)
        .map(|k| {
            let a = TAU * k as f32 / 5.0 - PI / 2.0;
            Point2::new(150.0 * a.cos(), 150.0 * a.sin())
        })
        .collect();
    let mut path = v.clone();
    path.push(v[0]);
    let pts = shake(&trace(&path, 6.0), 2.0, &mut rng);
    let shape = recognize(&pts);
    let Some(Shape::Polygon { vertices }) = shape else {
        panic!("{shape:?}");
    };
    assert_eq!(vertices.len(), 5);
}

#[test]
fn lines_at_any_angle_are_straightened() {
    let mut rng = Rng(7);
    for length in [60.0, 150.0, 400.0] {
        for angle in [0.3_f32, 0.8, 1.9, 3.5, 5.0] {
            let end = Point2::new(length * angle.cos(), length * angle.sin());
            let pts = shake(
                &trace(&[Point2::new(0.0, 0.0), end], 5.0),
                0.01 * length,
                &mut rng,
            );
            let shape = recognize(&pts);
            let Some(Shape::Line { a, b }) = shape else {
                panic!("longueur {length}, angle {angle} : {shape:?}");
            };
            assert!((a.distance(b) - length).abs() < 0.06 * length);
        }
    }
}

#[test]
fn nearly_horizontal_and_vertical_lines_become_exact() {
    let pts = trace(&[Point2::new(10.0, 10.0), Point2::new(210.0, 18.0)], 5.0);
    let Some(Shape::Line { a, b }) = recognize(&pts) else {
        panic!("droite attendue");
    };
    assert!((a.y - b.y).abs() < 1e-3, "devrait être horizontale");
    let pts = trace(&[Point2::new(10.0, 10.0), Point2::new(15.0, 210.0)], 5.0);
    let Some(Shape::Line { a, b }) = recognize(&pts) else {
        panic!("droite attendue");
    };
    assert!((a.x - b.x).abs() < 1e-3, "devrait être verticale");
}

fn arrow_path(tail: Point2, tip: Point2, wing: f32, half_angle: f32) -> Vec<Point2> {
    let back = (tail.y - tip.y).atan2(tail.x - tip.x);
    let w = |sign: f32| {
        let a = back + sign * half_angle;
        Point2::new(tip.x + wing * a.cos(), tip.y + wing * a.sin())
    };
    trace(&[tail, tip, w(1.0), tip, w(-1.0)], 5.0)
}

#[test]
fn arrows_drawn_in_one_stroke_are_found() {
    let mut rng = Rng(8);
    for dir in [0.0_f32, 0.7, 1.6, 2.5, 4.0] {
        let tail = Point2::new(50.0, 50.0);
        let tip = Point2::new(50.0 + 220.0 * dir.cos(), 50.0 + 220.0 * dir.sin());
        let pts = shake(&arrow_path(tail, tip, 50.0, 0.5), 1.5, &mut rng);
        let shape = recognize(&pts);
        let Some(Shape::Arrow {
            head_angle,
            head_len,
            tip: t,
            ..
        }) = shape
        else {
            panic!("direction {dir} : {shape:?}");
        };
        assert!(t.distance(tip) < 15.0);
        assert!((head_len - 50.0).abs() < 12.0 && (head_angle - 0.5).abs() < 0.2);
    }
}

#[test]
fn a_wavy_open_stroke_becomes_a_smooth_curve_not_a_line() {
    let pts: Vec<Point2> = (0..=100)
        .map(|i| {
            let x = i as f32 * 3.0;
            Point2::new(x, 40.0 * (x / 50.0).sin())
        })
        .collect();
    let shape = recognize(&pts);
    let Some(Shape::Curve { points, closed }) = shape else {
        panic!("{shape:?}");
    };
    assert!(!closed);
    assert_eq!(points.first(), Some(&pts[0]));
    assert!(points.last().is_some_and(|p| p.distance(pts[100]) < 1e-3));
}

#[test]
fn an_unidentifiable_closed_loop_becomes_a_closed_curve() {
    // Un huit : fermé, mais ni rond, ni rectangulaire, ni polygonal simple.
    let pts: Vec<Point2> = (0..=120)
        .map(|i| {
            let t = TAU * i as f32 / 120.0;
            Point2::new(100.0 * t.sin(), 60.0 * (2.0 * t).sin())
        })
        .collect();
    let shape = recognize(&pts);
    assert!(
        matches!(shape, Some(Shape::Curve { closed: true, .. })),
        "{shape:?}"
    );
}

#[test]
fn tiny_strokes_and_dots_are_ignored() {
    assert_eq!(recognize(&[]), None);
    assert_eq!(recognize(&[Point2::new(1.0, 1.0)]), None);
    let dot: Vec<Point2> = (0..30).map(|i| Point2::new(i as f32 * 0.1, 0.0)).collect();
    assert_eq!(recognize(&dot), None);
    let small_circle = circle_trace(Point2::new(0.0, 0.0), 3.0, 3.0, 0.0, 0.0, 1.0);
    assert_eq!(recognize(&small_circle), None);
}

#[test]
fn a_retraced_line_is_not_a_shape() {
    let mut pts = trace(&[Point2::new(0.0, 0.0), Point2::new(200.0, 0.0)], 5.0);
    let back: Vec<Point2> = pts.iter().rev().copied().collect();
    pts.extend(back);
    let kind = kind_of(&pts);
    assert!(
        !matches!(
            kind,
            Some(ShapeKind::Circle | ShapeKind::Ellipse | ShapeKind::Rectangle)
        ),
        "{kind:?}"
    );
}

#[test]
fn outlines_of_closed_shapes_return_to_their_start() {
    let c = Point2::new(5.0, 5.0);
    let shapes = [
        Shape::Circle {
            center: c,
            radius: 10.0,
        },
        Shape::Ellipse {
            center: c,
            rx: 10.0,
            ry: 5.0,
            angle: 0.3,
        },
        Shape::Rectangle {
            center: c,
            width: 10.0,
            height: 4.0,
            angle: 0.0,
        },
        Shape::Square {
            center: c,
            side: 6.0,
            angle: 0.5,
        },
        Shape::Triangle {
            vertices: [c, Point2::new(9.0, 5.0), Point2::new(5.0, 9.0)],
        },
    ];
    for shape in shapes {
        let outline = shape.outline();
        assert_eq!(outline.first(), outline.last(), "{:?}", shape.kind());
    }
    let line = Shape::Line {
        a: c,
        b: Point2::new(9.0, 9.0),
    };
    assert_eq!(line.outline().len(), 2);
}

#[test]
fn recognised_strokes_keep_the_brush_and_average_pressure() {
    let brush = Brush {
        kind: BrushKind::Highlighter,
        color: Color {
            r: 255,
            g: 200,
            b: 0,
            a: 110,
        },
        width: 18.0,
    };
    let mut stroke = Stroke::new(brush);
    for (i, pos) in circle_trace(Point2::new(0.0, 0.0), 80.0, 80.0, 0.0, 0.0, 1.0)
        .into_iter()
        .enumerate()
    {
        stroke.push(StrokePoint {
            pos,
            pressure: if i % 2 == 0 { 0.2 } else { 0.6 },
            tilt: None,
        });
    }
    let recognised = recognize_stroke(&stroke, false).expect("cercle");
    assert_eq!(recognised.kind, ShapeKind::Circle);
    assert_eq!(recognised.stroke.brush, brush);
    assert_ne!(recognised.stroke.id, stroke.id);
    assert!(recognised
        .stroke
        .points
        .iter()
        .all(|p| (p.pressure - 0.4).abs() < 0.02));
    assert_eq!(
        recognised.stroke.points.first().map(|p| p.pos),
        recognised.stroke.points.last().map(|p| p.pos)
    );
}

#[test]
fn curves_need_an_explicit_request() {
    let mut stroke = Stroke::new(Brush::default());
    for i in 0..=100 {
        let x = i as f32 * 3.0;
        stroke.push(StrokePoint {
            pos: Point2::new(x, 40.0 * (x / 50.0).sin()),
            pressure: 0.5,
            tilt: None,
        });
    }
    assert!(recognize_stroke(&stroke, false).is_none());
    assert_eq!(
        recognize_stroke(&stroke, true).map(|r| r.kind),
        Some(ShapeKind::Curve)
    );
}

/// 400 tracés aléatoires (taille, rotation, point de départ, tremblement) : au moins 95 %
/// doivent être reconnus pour ce qu'ils sont, et aucun cercle ne doit devenir un rectangle.
#[test]
fn random_hand_drawn_shapes_are_recognised_most_of_the_time() {
    let mut rng = Rng(99);
    let (mut total, mut correct) = (0, 0);
    let mut failures = Vec::new();
    for case in 0..400 {
        let size = 60.0 + 140.0 * (rng.next() + 1.0) / 2.0;
        let rot = PI * rng.next();
        let noise = size * (0.008 + 0.012 * (rng.next() + 1.0) / 2.0);
        let center = Point2::new(200.0, 200.0);
        let (expected, raw) = match case % 5 {
            0 => (
                ShapeKind::Circle,
                circle_trace(
                    center,
                    size,
                    size,
                    0.0,
                    PI * rng.next(),
                    1.0 + 0.05 * rng.next(),
                ),
            ),
            1 => (
                ShapeKind::Rectangle,
                trace(
                    &rect_vertices(center, 2.0 * size, 1.1 * size, rot, case % 4),
                    6.0,
                ),
            ),
            2 => {
                let v: Vec<Point2> = (0..3)
                    .map(|k| {
                        let a = rot + TAU * k as f32 / 3.0;
                        Point2::new(center.x + size * a.cos(), center.y + size * a.sin())
                    })
                    .collect();
                let mut path = v.clone();
                path.push(v[0]);
                (ShapeKind::Triangle, trace(&path, 6.0))
            }
            3 => (
                ShapeKind::Line,
                trace(
                    &[
                        center,
                        Point2::new(
                            center.x + 2.0 * size * rot.cos(),
                            center.y + 2.0 * size * rot.sin(),
                        ),
                    ],
                    5.0,
                ),
            ),
            _ => (
                ShapeKind::Ellipse,
                circle_trace(center, 1.8 * size, size, rot, PI * rng.next(), 1.0),
            ),
        };
        let pts = shake(&raw, noise, &mut rng);
        let got = kind_of(&pts);
        total += 1;
        if got == Some(expected) {
            correct += 1;
        } else {
            failures.push((case, expected, got));
        }
        if expected == ShapeKind::Circle {
            assert!(
                !matches!(
                    got,
                    Some(ShapeKind::Rectangle | ShapeKind::Square | ShapeKind::Triangle)
                ),
                "cas {case} : un cercle est devenu {got:?}"
            );
        }
    }
    eprintln!("TAUX {correct}/{total} ; échecs : {failures:?}");
    assert!(
        correct * 100 >= total * 95,
        "{correct}/{total} reconnus ; échecs : {failures:?}"
    );
}

#[test]
fn one_undo_restores_the_original_stroke_after_a_correction() {
    use calame_core::{History, Item, Page};
    use std::collections::HashMap;

    let mut rng = Rng(11);
    let mut original = Stroke::new(Brush::default());
    for pos in shake(
        &circle_trace(Point2::new(0.0, 0.0), 90.0, 90.0, 0.0, 0.0, 1.0),
        1.5,
        &mut rng,
    ) {
        original.push(StrokePoint {
            pos,
            pressure: 0.5,
            tilt: None,
        });
    }
    let recognised = recognize_stroke(&original, false).expect("cercle");

    // Même séquence que l'application : le trait d'origine, puis sa correction.
    let (mut page, mut history) = (Page::default(), History::new());
    history.add_item(&mut page, Item::Stroke(original.clone()));
    history.replace_items(
        &mut page,
        HashMap::from([(original.id, vec![Item::Stroke(recognised.stroke.clone())])]),
    );
    assert_eq!(page.items, vec![Item::Stroke(recognised.stroke)]);

    history.undo(&mut page);
    assert_eq!(
        page.items,
        vec![Item::Stroke(original)],
        "un Annuler retrouve le trait d'origine"
    );
    history.undo(&mut page);
    assert!(page.items.is_empty());
}
