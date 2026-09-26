//! «Wrong Way !»: el aviso de contramano, arriba al centro. Parpadea como el de Re-Volt
//! (`panel.cpp`: 256 ms prendido y 256 apagado) y además late un poco.

use egui::{pos2, Align2, Color32, FontId, Painter, Rect};

use crate::ui::theme::shadow_text;

const WRONG_WAY: Color32 = Color32::from_rgb(255, 70, 52);

/// `time` en segundos: marca el parpadeo y el latido.
pub fn show(painter: &Painter, screen: Rect, time: f64) {
    if (time * 1000.0) as u64 & 256 == 0 {
        return;
    }
    let beat = 1.0 + 0.08 * (time * std::f64::consts::TAU * 2.0).sin() as f32;
    let pos = pos2(screen.center().x, screen.top() + screen.height() * 0.22);
    let font = FontId::proportional(54.0 * beat);
    shadow_text(
        painter,
        pos,
        Align2::CENTER_CENTER,
        "Wrong Way !",
        font,
        WRONG_WAY,
    );
}
