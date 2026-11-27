//! Description d'une page en SVG, fidèle à ce qui s'affiche à l'écran.

use std::fmt::Write as _;

use base64::Engine as _;
use calame_core::background::export_rect;
use calame_core::grafcet::{Anchor, Prim};
use calame_core::{
    BrushKind, Color, GrafcetSymbol, ImageBox, Item, Page, Rect2, Stroke, StrokePoint, TextBox,
};

use crate::{BackgroundImage, PageBackground, PageImages};

/// Nombre de points ajoutés par segment lors du lissage des traits exportés.
const SMOOTHING: usize = 4;
/// Écart minimal entre deux points exportés, en unités de page (environ 0,1 mm).
const MIN_STEP: f32 = 0.4;
/// Marge autour d'éléments rendus seuls, en unités de page.
const ITEMS_MARGIN: f32 = 8.0;

/// Retire les points à moins de `MIN_STEP` du précédent : invisibles à l'œil, mais ils multipliaient
/// la taille du fichier par dix. Le dernier point est toujours conservé.
fn thin(points: Vec<StrokePoint>) -> Vec<StrokePoint> {
    let count = points.len();
    let mut out: Vec<StrokePoint> = Vec::with_capacity(count);
    for (i, p) in points.into_iter().enumerate() {
        let last = i + 1 == count;
        if last
            || out
                .last()
                .map_or(true, |q| q.pos.distance(p.pos) >= MIN_STEP)
        {
            out.push(p);
        }
    }
    out
}

fn n(value: f32) -> String {
    let text = format!("{value:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() || text == "-" || text == "-0" {
        "0".into()
    } else {
        text.into()
    }
}

fn hex(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

fn opacity(c: Color) -> String {
    n(f32::from(c.a) / 255.0)
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Début d'un SVG cadré sur `area`.
fn svg_start(area: Rect2) -> String {
    let mut out = String::new();
    let _ = write!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" \
         width=\"{w}\" height=\"{h}\" viewBox=\"{x} {y} {w} {h}\">",
        w = n(area.width()),
        h = n(area.height()),
        x = n(area.min.x),
        y = n(area.min.y),
    );
    out
}

/// Les éléments dans leur ordre. Une image dont le contenu manque dans `images` est omise.
fn items_into(out: &mut String, items: &[Item], images: &PageImages) {
    for item in items {
        match item {
            Item::Stroke(s) => stroke_svg(out, s),
            Item::Text(t) => text_svg(out, t),
            Item::Grafcet(g) => grafcet_svg(out, g),
            Item::Image(placed) => {
                if let Some(img) = images.get(&placed.asset) {
                    placed_image_svg(out, placed, img);
                }
            }
        }
    }
}

/// SVG d'une page sans image posée dessus (voir [`page_svg_with_images`]).
pub fn page_svg(page: &Page, background: &PageBackground) -> String {
    page_svg_with_images(page, background, &PageImages::new())
}

/// SVG complet d'une page : fond blanc, fond de page, puis les éléments dans leur ordre.
/// La zone est celle de la page A4 (ou de l'image de fond), élargie au contenu qui déborde.
pub fn page_svg_with_images(
    page: &Page,
    background: &PageBackground,
    images: &PageImages,
) -> String {
    let image_size = match background {
        PageBackground::Image(img) => Some((img.width, img.height)),
        PageBackground::Pattern(_) => None,
    };
    let area = export_rect(&page.items, image_size);
    let mut out = svg_start(area);
    let _ = write!(
        out,
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#ffffff\"/>",
        n(area.min.x),
        n(area.min.y),
        n(area.width()),
        n(area.height())
    );
    match background {
        PageBackground::Pattern(bg) => pattern_svg(&mut out, *bg, area),
        PageBackground::Image(img) => image_svg(&mut out, img),
    }
    items_into(&mut out, &page.items, images);
    out.push_str("</svg>");
    out
}

/// SVG d'éléments seuls, sans fond, cadré au plus près. `None` s'ils n'occupent aucune place.
pub(crate) fn items_svg(items: &[Item], images: &PageImages) -> Option<String> {
    let area = items
        .iter()
        .filter_map(Item::bounds)
        .reduce(Rect2::union)?
        .expand(ITEMS_MARGIN);
    let mut out = svg_start(area);
    items_into(&mut out, items, images);
    out.push_str("</svg>");
    Some(out)
}

fn pattern_svg(out: &mut String, background: calame_core::Background, area: Rect2) {
    let pattern = background.pattern(area);
    let color = hex(pattern.color);
    if !pattern.lines.is_empty() {
        let mut d = String::new();
        for (a, b) in &pattern.lines {
            let _ = write!(d, "M{} {}L{} {}", n(a.x), n(a.y), n(b.x), n(b.y));
        }
        let _ = write!(
            out,
            "<path d=\"{d}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"0.75\"/>"
        );
    }
    if !pattern.dots.is_empty() {
        let _ = write!(out, "<g fill=\"{color}\">");
        for p in &pattern.dots {
            let _ = write!(out, "<circle cx=\"{}\" cy=\"{}\" r=\"1\"/>", n(p.x), n(p.y));
        }
        out.push_str("</g>");
    }
}

fn image_svg(out: &mut String, img: &BackgroundImage) {
    let data = base64::engine::general_purpose::STANDARD.encode(&img.data);
    let _ = write!(
        out,
        "<image x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" \
         xlink:href=\"data:{};base64,{data}\"/>",
        n(img.width),
        n(img.height),
        img.mime
    );
}

fn placed_image_svg(out: &mut String, placed: &ImageBox, img: &BackgroundImage) {
    let data = base64::engine::general_purpose::STANDARD.encode(&img.data);
    let _ = write!(
        out,
        "<image transform=\"translate({} {}) rotate({})\" x=\"0\" y=\"0\" width=\"{}\" \
         height=\"{}\" preserveAspectRatio=\"none\" xlink:href=\"data:{};base64,{data}\"/>",
        n(placed.pos.x),
        n(placed.pos.y),
        n(placed.angle.to_degrees()),
        n(placed.width),
        n(placed.height),
        img.mime
    );
}

fn points_attr(points: &[StrokePoint]) -> String {
    let mut s = String::with_capacity(points.len() * 12);
    for p in points {
        let _ = write!(s, "{},{} ", n(p.pos.x), n(p.pos.y));
    }
    s.trim_end().to_owned()
}

fn stroke_svg(out: &mut String, stroke: &Stroke) {
    let points = thin(stroke.smoothed(SMOOTHING));
    let c = stroke.brush.color;
    match (stroke.brush.kind, points.as_slice()) {
        (_, []) => {}
        (BrushKind::Highlighter, [only]) => {
            let _ = write!(
                out,
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" fill-opacity=\"{}\"/>",
                n(only.pos.x),
                n(only.pos.y),
                n(stroke.brush.width / 2.0),
                hex(c),
                opacity(c)
            );
        }
        (BrushKind::Highlighter, pts) => {
            let _ = write!(
                out,
                "<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-opacity=\"{}\" \
                 stroke-width=\"{}\" stroke-linecap=\"butt\" stroke-linejoin=\"round\"/>",
                points_attr(pts),
                hex(c),
                opacity(c),
                n(stroke.brush.width)
            );
        }
        (BrushKind::Pen, [only]) => {
            let _ = write!(
                out,
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" fill-opacity=\"{}\"/>",
                n(only.pos.x),
                n(only.pos.y),
                n(stroke.width_at(only) / 2.0),
                hex(c),
                opacity(c)
            );
        }
        (BrushKind::Pen, pts) => pen_svg(out, stroke, pts),
    }
}

/// Trait au stylo : contour d'épaisseur variable, comme le dessin à l'écran, avec des bouts ronds.
fn pen_svg(out: &mut String, stroke: &Stroke, pts: &[StrokePoint]) {
    let c = stroke.brush.color;
    let count = pts.len();
    let mut left = Vec::with_capacity(count);
    let mut right = Vec::with_capacity(count);
    for (i, p) in pts.iter().enumerate() {
        let prev = pts[i.saturating_sub(1)].pos;
        let next = pts[(i + 1).min(count - 1)].pos;
        let (dx, dy) = (next.x - prev.x, next.y - prev.y);
        let len = dx.hypot(dy);
        let (ux, uy) = if len > f32::EPSILON {
            (dx / len, dy / len)
        } else {
            (1.0, 0.0)
        };
        let r = stroke.width_at(p) / 2.0;
        left.push((p.pos.x - uy * r, p.pos.y + ux * r));
        right.push((p.pos.x + uy * r, p.pos.y - ux * r));
    }
    let mut d = String::new();
    for (i, (x, y)) in left.iter().chain(right.iter().rev()).enumerate() {
        let _ = write!(d, "{}{} {}", if i == 0 { "M" } else { "L" }, n(*x), n(*y));
    }
    d.push('Z');
    let _ = write!(
        out,
        "<g fill=\"{}\" fill-opacity=\"{}\"><path d=\"{d}\"/>",
        hex(c),
        opacity(c)
    );
    for p in [pts[0], pts[count - 1]] {
        let _ = write!(
            out,
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"/>",
            n(p.pos.x),
            n(p.pos.y),
            n(stroke.width_at(&p) / 2.0)
        );
    }
    out.push_str("</g>");
}

fn text_svg(out: &mut String, text: &TextBox) {
    let lines: Vec<&str> = text.text.split('\n').collect();
    // La hauteur mesurée par l'interface donne l'interligne réel ; la ligne de base est à 80 %.
    let line_height = if text.height > 0.0 {
        text.height / lines.len() as f32
    } else {
        text.font_size * 1.2
    };
    let _ = write!(
        out,
        "<text transform=\"translate({} {}) rotate({})\" font-family=\"sans-serif\" \
         font-size=\"{}\" fill=\"{}\" fill-opacity=\"{}\" xml:space=\"preserve\">",
        n(text.pos.x),
        n(text.pos.y),
        n(text.angle.to_degrees()),
        n(text.font_size),
        hex(text.color),
        opacity(text.color)
    );
    for (i, line) in lines.iter().enumerate() {
        let _ = write!(
            out,
            "<tspan x=\"0\" y=\"{}\">{}</tspan>",
            n(i as f32 * line_height + line_height * 0.8),
            escape(line)
        );
    }
    out.push_str("</text>");
}

fn grafcet_svg(out: &mut String, symbol: &GrafcetSymbol) {
    let (color, alpha) = (hex(symbol.color), opacity(symbol.color));
    let _ = write!(out, "<g opacity=\"{alpha}\">");
    for prim in symbol.prims() {
        match prim {
            Prim::Line { a, b, width } => {
                let _ = write!(
                    out,
                    "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{color}\" \
                     stroke-width=\"{}\" stroke-linecap=\"square\"/>",
                    n(a.x),
                    n(a.y),
                    n(b.x),
                    n(b.y),
                    n(width)
                );
            }
            Prim::Rect { min, max, width } => {
                let _ = write!(
                    out,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" \
                     stroke=\"{color}\" stroke-width=\"{}\"/>",
                    n(min.x),
                    n(min.y),
                    n(max.x - min.x),
                    n(max.y - min.y),
                    n(width)
                );
            }
            Prim::Filled { min, max } => {
                let _ = write!(
                    out,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{color}\"/>",
                    n(min.x),
                    n(min.y),
                    n(max.x - min.x),
                    n(max.y - min.y)
                );
            }
            Prim::Text {
                at,
                anchor,
                text,
                size,
                height,
                ..
            } => {
                let lines: Vec<&str> = text.split('\n').collect();
                let line_height = height / lines.len() as f32;
                let anchor = match anchor {
                    Anchor::Center => "middle",
                    Anchor::LeftMiddle => "start",
                };
                let _ = write!(
                    out,
                    "<text font-family=\"sans-serif\" font-size=\"{}\" fill=\"{color}\" \
                     text-anchor=\"{anchor}\" xml:space=\"preserve\">",
                    n(size)
                );
                for (i, line) in lines.iter().enumerate() {
                    // Ligne de base à 80 % de la hauteur de ligne, comme pour un texte.
                    let y = at.y - height / 2.0 + line_height * (i as f32 + 0.8);
                    let _ = write!(
                        out,
                        "<tspan x=\"{}\" y=\"{}\">{}</tspan>",
                        n(at.x),
                        n(y),
                        escape(line)
                    );
                }
                out.push_str("</text>");
            }
        }
    }
    out.push_str("</g>");
}
