//! La paleta de la UI y el texto con sombra: los comparten el menú, el HUD de carrera y
//! los resultados.

use egui::{vec2, Align2, Color32, FontId, Painter, Pos2};

pub const ACCENT: Color32 = Color32::from_rgb(255, 184, 28);
pub const ACCENT_DARK: Color32 = Color32::from_rgb(214, 150, 18);
pub const TEXT: Color32 = Color32::from_rgb(240, 241, 245);
pub const DIM: Color32 = Color32::from_rgb(150, 155, 168);
pub const DARK: Color32 = Color32::from_rgb(22, 22, 26);
pub const READY: Color32 = Color32::from_rgb(92, 206, 124);
pub const PANEL: Color32 = Color32::from_rgba_unmultiplied_const(10, 12, 18, 210);
pub const TAB_IDLE: Color32 = Color32::from_rgba_unmultiplied_const(10, 12, 18, 165);
pub const CARD: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 18);
pub const CARD_EMPTY: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 8);
pub const CARD_BORDER: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 34);
pub const BUTTON: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 22);
pub const HIGHLIGHT: Color32 = Color32::from_rgba_unmultiplied_const(255, 184, 28, 40);
pub const SHADOW: Color32 = Color32::from_black_alpha(160);

/// Texto con sombra, para leerlo sobre la pista.
pub fn shadow_text(
    painter: &Painter,
    pos: Pos2,
    anchor: Align2,
    text: &str,
    font: FontId,
    color: Color32,
) {
    painter.text(pos + vec2(2.0, 2.0), anchor, text, font.clone(), SHADOW);
    painter.text(pos, anchor, text, font, color);
}
