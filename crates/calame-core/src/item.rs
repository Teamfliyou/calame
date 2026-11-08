//! Éléments d'une page : traits, zones de texte et images.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::background::AssetId;
use crate::geometry::{point_in_polygon, Color, Point2, Rect2, Similarity};
use crate::stroke::Stroke;

/// Les quatre coins d'un rectangle pivoté autour de son coin haut-gauche `pos`, dans l'ordre
/// haut-gauche, haut-droit, bas-droit, bas-gauche.
fn rotated_corners(pos: Point2, angle: f32, width: f32, height: f32) -> [Point2; 4] {
    let (sin, cos) = angle.sin_cos();
    let at = |x: f32, y: f32| Point2::new(pos.x + x * cos - y * sin, pos.y + x * sin + y * cos);
    [
        at(0.0, 0.0),
        at(width, 0.0),
        at(width, height),
        at(0.0, height),
    ]
}

/// `p` est-il dans ce rectangle pivoté, à `margin` près ?
fn rotated_contains(
    pos: Point2,
    angle: f32,
    width: f32,
    height: f32,
    p: Point2,
    margin: f32,
) -> bool {
    // On ramène le point dans le repère non pivoté du rectangle.
    let (sin, cos) = (-angle).sin_cos();
    let (dx, dy) = (p.x - pos.x, p.y - pos.y);
    let (x, y) = (dx * cos - dy * sin, dx * sin + dy * cos);
    x >= -margin && x <= width + margin && y >= -margin && y <= height + margin
}

/// Zone de texte tapée au clavier. `pos` est son coin haut-gauche ; elle pivote autour de ce point.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextBox {
    pub id: Uuid,
    pub pos: Point2,
    pub text: String,
    /// Corps du texte en unités de page.
    pub font_size: f32,
    pub color: Color,
    /// Rotation en radians autour de `pos`.
    pub angle: f32,
    /// Dimensions du texte non pivoté, mesurées par l'interface lors de la mise en page.
    pub width: f32,
    pub height: f32,
}

impl TextBox {
    pub fn new(pos: Point2, text: String, font_size: f32, color: Color) -> Self {
        Self {
            id: Uuid::new_v4(),
            pos,
            text,
            font_size,
            color,
            angle: 0.0,
            width: font_size,
            height: font_size,
        }
    }

    /// Les quatre coins du rectangle pivoté, dans l'ordre haut-gauche, haut-droit, bas-droit, bas-gauche.
    pub fn corners(&self) -> [Point2; 4] {
        rotated_corners(self.pos, self.angle, self.width, self.height)
    }

    fn contains(&self, p: Point2, margin: f32) -> bool {
        rotated_contains(self.pos, self.angle, self.width, self.height, p, margin)
    }
}

/// Image posée sur la page (collée ou déposée). Son contenu est rangé à part, avec les images de la
/// bibliothèque. `pos` est son coin haut-gauche ; elle pivote autour de ce point.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageBox {
    pub id: Uuid,
    pub asset: AssetId,
    pub pos: Point2,
    /// Dimensions de l'image non pivotée, en unités de page.
    pub width: f32,
    pub height: f32,
    /// Rotation en radians autour de `pos`.
    pub angle: f32,
}

impl ImageBox {
    pub fn new(asset: AssetId, pos: Point2, width: f32, height: f32) -> Self {
        Self {
            id: Uuid::new_v4(),
            asset,
            pos,
            width,
            height,
            angle: 0.0,
        }
    }

    /// Les quatre coins du rectangle pivoté, dans l'ordre haut-gauche, haut-droit, bas-droit, bas-gauche.
    pub fn corners(&self) -> [Point2; 4] {
        rotated_corners(self.pos, self.angle, self.width, self.height)
    }
}

/// Le numéro de chaque variante est enregistré avec les pages : on n'ajoute qu'à la fin.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Item {
    Stroke(Stroke),
    Text(TextBox),
    Image(ImageBox),
}

impl Item {
    pub fn id(&self) -> Uuid {
        match self {
            Self::Stroke(s) => s.id,
            Self::Text(t) => t.id,
            Self::Image(i) => i.id,
        }
    }

    pub fn bounds(&self) -> Option<Rect2> {
        match self {
            Self::Stroke(s) => s.bounds(),
            Self::Text(t) => Rect2::from_points(t.corners()),
            Self::Image(i) => Rect2::from_points(i.corners()),
        }
    }

    /// `true` si l'élément est à moins de `radius` de `p` (gomme, sélection au clic).
    pub fn hit(&self, p: Point2, radius: f32) -> bool {
        match self {
            Self::Stroke(s) => s.hit(p, radius),
            Self::Text(t) => t.contains(p, radius),
            Self::Image(i) => rotated_contains(i.pos, i.angle, i.width, i.height, p, radius),
        }
    }

    /// Sélection au lasso : au moins la moitié d'un trait dans le polygone, le centre d'un texte, ou
    /// toute l'image (entourer une annotation posée dessus ne doit pas emporter l'image).
    pub fn inside_polygon(&self, polygon: &[Point2]) -> bool {
        match self {
            Self::Stroke(s) => {
                if s.points.is_empty() {
                    return false;
                }
                let inside = s
                    .points
                    .iter()
                    .filter(|p| point_in_polygon(p.pos, polygon))
                    .count();
                inside * 2 >= s.points.len()
            }
            Self::Text(t) => {
                let [a, _, c, _] = t.corners();
                point_in_polygon(a.lerp(c, 0.5), polygon)
            }
            Self::Image(i) => i.corners().iter().all(|c| point_in_polygon(*c, polygon)),
        }
    }

    pub fn transformed(&self, t: &Similarity) -> Self {
        match self {
            Self::Stroke(s) => Self::Stroke(s.transformed(t)),
            Self::Text(text) => {
                let mut out = text.clone();
                out.pos = t.apply(text.pos);
                out.font_size *= t.scale;
                out.width *= t.scale;
                out.height *= t.scale;
                out.angle += t.angle;
                Self::Text(out)
            }
            Self::Image(image) => {
                let mut out = image.clone();
                out.pos = t.apply(image.pos);
                out.width *= t.scale;
                out.height *= t.scale;
                out.angle += t.angle;
                Self::Image(out)
            }
        }
    }

    /// Couleur d'origine, sans transparence (pour la pipette). Une image n'en a pas.
    pub fn color(&self) -> Option<Color> {
        let c = match self {
            Self::Stroke(s) => s.brush.color,
            Self::Text(t) => t.color,
            Self::Image(_) => return None,
        };
        Some(Color::rgb(c.r, c.g, c.b))
    }

    /// Change la teinte en conservant la transparence (le surligneur reste translucide).
    /// Sans effet sur une image.
    pub fn set_color(&mut self, color: Color) {
        let target = match self {
            Self::Stroke(s) => &mut s.brush.color,
            Self::Text(t) => &mut t.color,
            Self::Image(_) => return,
        };
        *target = Color {
            a: target.a,
            ..color
        };
    }

    /// Copie avec un nouvel identifiant (collage).
    pub fn duplicated(&self) -> Self {
        let mut out = self.clone();
        match &mut out {
            Self::Stroke(s) => s.id = Uuid::new_v4(),
            Self::Text(t) => t.id = Uuid::new_v4(),
            Self::Image(i) => i.id = Uuid::new_v4(),
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stroke::{Brush, StrokePoint};

    fn text_at(x: f32, y: f32) -> TextBox {
        let mut t = TextBox::new(Point2::new(x, y), "abc".into(), 10.0, Color::BLACK);
        t.width = 30.0;
        t.height = 10.0;
        t
    }

    fn line(x0: f32, x1: f32) -> Item {
        let mut s = Stroke::new(Brush::default());
        for x in [x0, x1] {
            s.push(StrokePoint {
                pos: Point2::new(x, 0.0),
                pressure: 0.5,
                tilt: None,
            });
        }
        Item::Stroke(s)
    }

    #[test]
    fn text_hit_follows_rotation() {
        let mut t = text_at(0.0, 0.0);
        assert!(Item::Text(t.clone()).hit(Point2::new(25.0, 5.0), 0.0));
        t.angle = std::f32::consts::FRAC_PI_2;
        // Tourné d'un quart de tour (sens horaire), le texte descend le long de l'axe y.
        let item = Item::Text(t);
        assert!(item.hit(Point2::new(-5.0, 25.0), 0.0));
        assert!(!item.hit(Point2::new(25.0, 5.0), 0.0));
    }

    #[test]
    fn lasso_takes_strokes_mostly_inside_and_texts_by_center() {
        let polygon = [
            Point2::new(-5.0, -5.0),
            Point2::new(40.0, -5.0),
            Point2::new(40.0, 40.0),
            Point2::new(-5.0, 40.0),
        ];
        assert!(line(0.0, 10.0).inside_polygon(&polygon));
        assert!(!line(100.0, 110.0).inside_polygon(&polygon));
        assert!(Item::Text(text_at(0.0, 0.0)).inside_polygon(&polygon));
        assert!(!Item::Text(text_at(200.0, 0.0)).inside_polygon(&polygon));
    }

    #[test]
    fn transform_text_scales_size_and_rotates() {
        let t = Item::Text(text_at(10.0, 0.0));
        let out = t.transformed(&Similarity {
            pivot: Point2::new(0.0, 0.0),
            scale: 2.0,
            angle: 0.5,
            translate: Point2::new(0.0, 0.0),
        });
        let Item::Text(out) = out else {
            unreachable!("un texte reste un texte");
        };
        assert_eq!(out.font_size, 20.0);
        assert_eq!(out.width, 60.0);
        assert_eq!(out.angle, 0.5);
    }

    #[test]
    fn set_color_keeps_alpha_and_duplicate_changes_id() {
        let mut item = line(0.0, 1.0);
        if let Item::Stroke(s) = &mut item {
            s.brush.color.a = 110;
        }
        item.set_color(Color::rgb(1, 2, 3));
        let Item::Stroke(s) = &item else {
            unreachable!("un trait reste un trait");
        };
        assert_eq!(
            s.brush.color,
            Color {
                r: 1,
                g: 2,
                b: 3,
                a: 110
            }
        );
        assert_eq!(item.color(), Some(Color::rgb(1, 2, 3)));
        assert_ne!(item.duplicated().id(), item.id());
    }

    fn image_at(x: f32, y: f32) -> ImageBox {
        ImageBox::new(AssetId::new(), Point2::new(x, y), 40.0, 20.0)
    }

    #[test]
    fn image_follows_transforms_and_keeps_its_content() {
        let image = image_at(10.0, 0.0);
        let asset = image.asset;
        let moved = Item::Image(image).transformed(&Similarity {
            pivot: Point2::new(0.0, 0.0),
            scale: 2.0,
            angle: 0.5,
            translate: Point2::new(0.0, 0.0),
        });
        let Item::Image(out) = &moved else {
            unreachable!("une image reste une image");
        };
        assert_eq!((out.width, out.height, out.angle), (80.0, 40.0, 0.5));
        assert_eq!(out.asset, asset);
        assert_eq!(moved.color(), None);
        assert_ne!(moved.duplicated().id(), moved.id());
    }

    #[test]
    fn lasso_takes_an_image_only_when_it_surrounds_it_entirely() {
        let polygon = [
            Point2::new(-5.0, -5.0),
            Point2::new(50.0, -5.0),
            Point2::new(50.0, 30.0),
            Point2::new(-5.0, 30.0),
        ];
        assert!(Item::Image(image_at(0.0, 0.0)).inside_polygon(&polygon));
        // Le centre est dans le polygone, pas un des coins.
        assert!(!Item::Image(image_at(20.0, 0.0)).inside_polygon(&polygon));
        assert!(Item::Image(image_at(20.0, 0.0)).hit(Point2::new(30.0, 10.0), 0.0));
        assert!(!Item::Image(image_at(20.0, 0.0)).hit(Point2::new(10.0, 10.0), 0.0));
    }
}
