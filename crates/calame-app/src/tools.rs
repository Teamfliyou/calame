//! Outils de dessin et réglages courants (couleur, épaisseurs, favoris, trousse de stylos).

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
    /// Déplacer, tourner et régler règle, équerre, rapporteur et compas.
    Instruments,
}

pub const PALETTE: [Color; 12] = [
    Color::rgb(0x15, 0x17, 0x1C),
    Color::rgb(0x2B, 0x2F, 0x38),
    Color::rgb(0x23, 0x40, 0xC4),
    Color::rgb(0xD2, 0x45, 0x2E),
    Color::rgb(0xE8, 0x7A, 0x22),
    Color::rgb(0xF4, 0xB7, 0x40),
    Color::rgb(0x2F, 0x7A, 0x5C),
    Color::rgb(0x16, 0x76, 0x76),
    Color::rgb(0x7A, 0x4F, 0xC4),
    Color::rgb(0xC8, 0x46, 0xA0),
    Color::rgb(0x82, 0x55, 0x32),
    Color::rgb(255, 255, 255),
];

/// Un stylo de la trousse : type de pinceau, couleur et épaisseur.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PenPreset {
    pub kind: BrushKind,
    pub color: Color,
    pub width: f32,
}

/// Trousse proposée au premier lancement : encre bleue fine, rouge moyen, graphite, surligneur jaune.
pub const DEFAULT_PENS: [PenPreset; 4] = [
    PenPreset {
        kind: BrushKind::Pen,
        color: Color::rgb(0x23, 0x40, 0xC4),
        width: 2.0,
    },
    PenPreset {
        kind: BrushKind::Pen,
        color: Color::rgb(0xD2, 0x45, 0x2E),
        width: 3.5,
    },
    PenPreset {
        kind: BrushKind::Pen,
        color: Color::rgb(0x2B, 0x2F, 0x38),
        width: 2.5,
    },
    PenPreset {
        kind: BrushKind::Highlighter,
        color: Color::rgb(0xF4, 0xB7, 0x40),
        width: 18.0,
    },
];
pub const MAX_PENS: usize = 8;

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
    /// Reconnaît chaque trait au relâchement (sinon : seulement après un maintien immobile).
    pub shape_recognition: bool,
    /// Contenu du champ de saisie hexadécimale.
    pub hex: String,
    /// Stylos préréglés de la trousse, enregistrés dans les réglages.
    pub pens: Vec<PenPreset>,
    /// Gomme reprise par le bouton du dock : de trait entier ou de zone.
    pub eraser: Tool,
}

impl Default for ToolSettings {
    fn default() -> Self {
        Self {
            tool: Tool::Pen,
            previous: Tool::Pen,
            color: DEFAULT_PENS[0].color,
            pen_width: DEFAULT_PENS[0].width,
            highlighter_width: 18.0,
            eraser_size: 16.0,
            text_size: 20.0,
            favorites: Vec::new(),
            shape_recognition: false,
            hex: DEFAULT_PENS[0].color.to_hex(),
            pens: DEFAULT_PENS.to_vec(),
            eraser: Tool::EraserStroke,
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

    /// Prend un stylo de la trousse : outil, couleur et épaisseur.
    pub fn apply_pen(&mut self, pen: PenPreset) {
        self.color = Color {
            a: 255,
            ..pen.color
        };
        self.hex = self.color.to_hex();
        match pen.kind {
            BrushKind::Pen => {
                self.tool = Tool::Pen;
                self.pen_width = pen.width;
            }
            BrushKind::Highlighter => {
                self.tool = Tool::Highlighter;
                self.highlighter_width = pen.width;
            }
        }
    }

    /// Réglage courant du stylo ou du surligneur, tel qu'il irait dans la trousse.
    pub fn current_pen(&self) -> Option<PenPreset> {
        let (kind, width) = match self.tool {
            Tool::Pen => (BrushKind::Pen, self.pen_width),
            Tool::Highlighter => (BrushKind::Highlighter, self.highlighter_width),
            _ => return None,
        };
        Some(PenPreset {
            kind,
            color: self.color,
            width,
        })
    }

    /// Stylo de la trousse qui correspond au réglage courant.
    pub fn active_pen(&self) -> Option<usize> {
        let current = self.current_pen()?;
        self.pens.iter().position(|p| {
            p.kind == current.kind
                && p.color == current.color
                && (p.width - current.width).abs() < 0.01
        })
    }

    /// Ajoute le réglage courant à la trousse ; `false` s'il y est déjà ou si elle est pleine.
    pub fn add_current_pen(&mut self) -> bool {
        match self.current_pen() {
            Some(pen) if self.active_pen().is_none() && self.pens.len() < MAX_PENS => {
                self.pens.push(pen);
                true
            }
            _ => false,
        }
    }

    pub fn pens_to_string(&self) -> String {
        self.pens
            .iter()
            .map(|p| {
                let kind = match p.kind {
                    BrushKind::Pen => "pen",
                    BrushKind::Highlighter => "hl",
                };
                format!("{kind}:{}:{}", p.color.to_hex(), p.width)
            })
            .collect::<Vec<_>>()
            .join(";")
    }

    /// Relit la trousse enregistrée ; les entrées illisibles sont ignorées.
    pub fn set_pens_from_string(&mut self, text: &str) {
        self.pens = text
            .split(';')
            .filter_map(|entry| {
                let mut parts = entry.split(':');
                let kind = match parts.next()? {
                    "pen" => BrushKind::Pen,
                    "hl" => BrushKind::Highlighter,
                    _ => return None,
                };
                let color = Color::from_hex(parts.next()?)?;
                let width: f32 = parts.next()?.parse().ok()?;
                (width.is_finite() && width > 0.0).then_some(PenPreset { kind, color, width })
            })
            .take(MAX_PENS)
            .collect();
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
    fn pens_apply_match_and_roundtrip() {
        let mut t = ToolSettings::default();
        assert_eq!(
            t.active_pen(),
            Some(0),
            "la trousse commence sur l'encre bleue"
        );
        t.apply_pen(DEFAULT_PENS[3]);
        assert_eq!(t.tool, Tool::Highlighter);
        assert_eq!(t.active_pen(), Some(3));
        assert!(!t.add_current_pen(), "déjà dans la trousse");
        t.color = Color::rgb(1, 2, 3);
        assert_eq!(t.active_pen(), None);
        assert!(t.add_current_pen());
        assert_eq!(t.active_pen(), Some(4));

        let saved = t.pens_to_string();
        let mut other = ToolSettings::default();
        other.set_pens_from_string(&saved);
        assert_eq!(other.pens, t.pens);
        other.set_pens_from_string("pen:#zz:2;crayon:#000000:1;hl:#ffcc00:-3;pen:#112233:1.5");
        assert_eq!(other.pens.len(), 1);
        t.tool = Tool::Lasso;
        assert!(t.current_pen().is_none());
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
