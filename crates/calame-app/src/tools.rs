//! Outils de dessin et réglages courants (couleur, épaisseurs, favoris).

use calame_core::{Brush, BrushKind, Color};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Pen,
    Highlighter,
    /// Gomme de trait entier.
    EraserStroke,
    /// Gomme de zone.
    EraserArea,
    Lasso,
    Text,
    /// Pipette : prend la couleur de l'élément cliqué.
    Pipette,
}

pub const PALETTE: [Color; 12] = [
    Color::rgb(0, 0, 0),
    Color::rgb(90, 90, 90),
    Color::rgb(220, 50, 47),
    Color::rgb(238, 130, 48),
    Color::rgb(244, 200, 0),
    Color::rgb(60, 160, 60),
    Color::rgb(0, 150, 150),
    Color::rgb(40, 110, 230),
    Color::rgb(110, 70, 200),
    Color::rgb(200, 70, 160),
    Color::rgb(130, 85, 50),
    Color::rgb(255, 255, 255),
];

pub const PEN_WIDTHS: [f32; 6] = [1.0, 2.0, 3.0, 5.0, 8.0, 12.0];
pub const HIGHLIGHTER_WIDTHS: [f32; 4] = [10.0, 18.0, 28.0, 40.0];
const HIGHLIGHTER_ALPHA: u8 = 110;
pub const MAX_FAVORITES: usize = 16;

pub struct ToolSettings {
    pub tool: Tool,
    /// Outil à retrouver après un prélèvement à la pipette.
    pub previous: Tool,
    /// Couleur courante, opaque : le surligneur y ajoute sa transparence.
    pub color: Color,
    pub pen_width: f32,
    pub highlighter_width: f32,
    /// Diamètre des gommes, en unités de page.
    pub eraser_size: f32,
    pub text_size: f32,
    pub favorites: Vec<Color>,
    /// Contenu du champ de saisie hexadécimale.
    pub hex: String,
}

impl Default for ToolSettings {
    fn default() -> Self {
        Self {
            tool: Tool::Pen,
            previous: Tool::Pen,
            color: Color::BLACK,
            pen_width: 3.0,
            highlighter_width: 18.0,
            eraser_size: 16.0,
            text_size: 20.0,
            favorites: Vec::new(),
            hex: Color::BLACK.to_hex(),
        }
    }
}

impl ToolSettings {
    /// Pinceau pour les outils de tracé ; `None` pour les autres outils.
    pub fn brush(&self) -> Option<Brush> {
        match self.tool {
            Tool::Pen => Some(Brush {
                kind: BrushKind::Pen,
                color: self.color,
                width: self.pen_width,
            }),
            Tool::Highlighter => Some(Brush {
                kind: BrushKind::Highlighter,
                color: Color {
                    a: HIGHLIGHTER_ALPHA,
                    ..self.color
                },
                width: self.highlighter_width,
            }),
            _ => None,
        }
    }

    /// Ajoute la couleur courante aux favoris (sans doublon, les plus récents d'abord).
    pub fn add_favorite(&mut self) {
        self.favorites.retain(|c| *c != self.color);
        self.favorites.insert(0, self.color);
        self.favorites.truncate(MAX_FAVORITES);
    }

    pub fn favorites_to_string(&self) -> String {
        self.favorites
            .iter()
            .map(|c| c.to_hex())
            .collect::<Vec<_>>()
            .join(",")
    }

    pub fn set_favorites_from_string(&mut self, text: &str) {
        self.favorites = text
            .split(',')
            .filter_map(Color::from_hex)
            .take(MAX_FAVORITES)
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlighter_is_translucent_and_pen_opaque() {
        let mut t = ToolSettings {
            color: Color::rgb(10, 20, 30),
            ..ToolSettings::default()
        };
        assert_eq!(t.brush().map(|b| b.color.a), Some(255));
        t.tool = Tool::Highlighter;
        let brush = t.brush();
        assert_eq!(brush.map(|b| b.color.a), Some(HIGHLIGHTER_ALPHA));
        assert_eq!(brush.map(|b| b.kind), Some(BrushKind::Highlighter));
        t.tool = Tool::Lasso;
        assert!(t.brush().is_none());
    }

    #[test]
    fn favorites_dedupe_cap_and_roundtrip() {
        let mut t = ToolSettings::default();
        for i in 0..20u8 {
            t.color = Color::rgb(i, 0, 0);
            t.add_favorite();
        }
        assert_eq!(t.favorites.len(), MAX_FAVORITES);
        t.color = Color::rgb(15, 0, 0);
        t.add_favorite();
        assert_eq!(t.favorites[0], Color::rgb(15, 0, 0));
        assert_eq!(
            t.favorites
                .iter()
                .filter(|c| **c == Color::rgb(15, 0, 0))
                .count(),
            1
        );

        let saved = t.favorites_to_string();
        let mut other = ToolSettings::default();
        other.set_favorites_from_string(&saved);
        assert_eq!(other.favorites, t.favorites);
        other.set_favorites_from_string("n'importe quoi,#12");
        assert!(other.favorites.is_empty());
    }
}
