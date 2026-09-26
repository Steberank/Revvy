//! Resultados: cuando terminan los jugadores, la lista de los que llegaron, en orden, con
//! su tiempo a la derecha, y abajo «ESC para volver al menu» parpadeando. La pista sigue
//! dibujándose de fondo.

use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Painter, Rect, Stroke};

use super::hud::race_time;
use super::theme::{shadow_text, ACCENT, CARD, PANEL, TEXT};

const WIDTH: f32 = 560.0;
const ROW: f32 = 44.0;
const HEADER: f32 = 76.0;
const FOOTER: f32 = 64.0;
const PAD: f32 = 24.0;
/// El aviso de abajo: prendido esta parte de cada segundo.
const BLINK_ON: f64 = 0.6;

/// `results`: nombre y tiempo total de cada uno, en el orden en que llegaron. `time` en
/// segundos, para el parpadeo.
pub fn show(painter: &Painter, screen: Rect, results: &[(String, f64)], time: f64) {
    painter.rect_filled(screen, CornerRadius::ZERO, Color32::from_black_alpha(90));
    let width = WIDTH.min(screen.width() - PAD * 2.0);
    let height = HEADER + ROW * results.len().max(1) as f32 + PAD;
    let panel = Rect::from_center_size(
        screen.center() - vec2(0.0, FOOTER * 0.5),
        vec2(width, height),
    );
    painter.rect_filled(panel, CornerRadius::same(8), PANEL);
    shadow_text(
        painter,
        pos2(panel.center().x, panel.top() + HEADER * 0.5),
        Align2::CENTER_CENTER,
        "Resultados",
        FontId::proportional(36.0),
        ACCENT,
    );

    let left = panel.left() + PAD;
    let right = panel.right() - PAD;
    let mut y = panel.top() + HEADER;
    for (place, (name, total)) in results.iter().enumerate() {
        let row = Rect::from_min_size(pos2(left - 8.0, y), vec2(right - left + 16.0, ROW - 6.0));
        painter.rect_filled(row, CornerRadius::same(4), CARD);
        let middle = row.center().y;
        painter.text(
            pos2(left, middle),
            Align2::LEFT_CENTER,
            format!("{}.", place + 1),
            FontId::proportional(22.0),
            ACCENT,
        );
        painter.text(
            pos2(left + 44.0, middle),
            Align2::LEFT_CENTER,
            name,
            FontId::proportional(22.0),
            TEXT,
        );
        painter.text(
            pos2(right, middle),
            Align2::RIGHT_CENTER,
            race_time(*total),
            FontId::monospace(20.0),
            TEXT,
        );
        y += ROW;
    }
    painter.line_segment(
        [
            pos2(left, panel.top() + HEADER - 10.0),
            pos2(right, panel.top() + HEADER - 10.0),
        ],
        Stroke::new(1.0, Color32::from_white_alpha(28)),
    );

    if time.rem_euclid(1.0) < BLINK_ON {
        shadow_text(
            painter,
            pos2(screen.center().x, panel.bottom() + FOOTER * 0.75),
            Align2::CENTER_CENTER,
            "ESC para volver al menu",
            FontId::proportional(22.0),
            TEXT,
        );
    }
}
