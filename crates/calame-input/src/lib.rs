//! Couche d'entrée unifiée (souris, tactile, stylet).
//!
//! Les backends natifs (Windows Ink / `WM_POINTER`, libinput, `NSEvent`) viendront
//! alimenter ces types ; l'interface ne dépend que de `PointerSample`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerKind {
    Mouse,
    Touch,
    Pen,
}

/// Un échantillon de pointeur indépendant de la plateforme.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerSample {
    pub kind: PointerKind,
    /// Position en pixels logiques de la fenêtre.
    pub x: f32,
    pub y: f32,
    /// Pression `0.0..=1.0` ; 0.5 par défaut pour la souris.
    pub pressure: f32,
    pub tilt: Option<f32>,
    pub eraser: bool,
}

impl PointerSample {
    pub fn mouse(x: f32, y: f32) -> Self {
        Self {
            kind: PointerKind::Mouse,
            x,
            y,
            pressure: 0.5,
            tilt: None,
            eraser: false,
        }
    }
}
