//! HUD de carrera: vuelta, puesto y tiempos del auto que se maneja, arriba a la derecha, y
//! el aviso de contramano.

pub mod wrong_way;

use egui::{pos2, vec2, Align2, CornerRadius, FontId, Painter, Rect};

use super::theme::{shadow_text, ACCENT, DIM, PANEL, TEXT};
use super::RaceHud;

const PANEL_WIDTH: f32 = 250.0;
const ROW: f32 = 26.0;
const PAD: f32 = 12.0;

/// El panel de la carrera: la vuelta y el puesto grandes, y abajo los tiempos.
pub fn race_panel(painter: &Painter, screen: Rect, race: &RaceHud) {
    let times = [
        ("Tiempo", Some(race.time)),
        ("Vuelta", race.lap_time),
        ("Última", race.last_lap),
        ("Mejor", race.best_lap),
    ];
    let height = PAD * 2.0 + ROW * 1.6 * 2.0 + ROW * times.len() as f32;
    let margin = (screen.width() * 0.02).clamp(12.0, 24.0);
    let panel = Rect::from_min_size(
        pos2(screen.right() - margin - PANEL_WIDTH, screen.top() + margin),
        vec2(PANEL_WIDTH, height),
    );
    painter.rect_filled(panel, CornerRadius::same(6), PANEL);
    let left = panel.left() + PAD;
    let right = panel.right() - PAD;
    let mut y = panel.top() + PAD;

    let big = FontId::proportional(24.0);
    for (label, value) in [
        ("VUELTA", format!("{}/{}", race.lap, race.laps)),
        ("PUESTO", format!("{}/{}", race.position, race.racers)),
    ] {
        let middle = y + ROW * 0.8;
        painter.text(
            pos2(left, middle),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(15.0),
            DIM,
        );
        shadow_text(
            painter,
            pos2(right, middle),
            Align2::RIGHT_CENTER,
            &value,
            big.clone(),
            ACCENT,
        );
        y += ROW * 1.6;
    }
    let font = FontId::monospace(17.0);
    for (label, value) in times {
        let middle = y + ROW * 0.5;
        painter.text(
            pos2(left, middle),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(15.0),
            DIM,
        );
        let text = value.map_or_else(|| "--:--:---".to_string(), race_time);
        painter.text(
            pos2(right, middle),
            Align2::RIGHT_CENTER,
            text,
            font.clone(),
            TEXT,
        );
        y += ROW;
    }
}

/// `02:13:456`: minutos, segundos y milésimas, como los tiempos de Re-Volt.
pub fn race_time(seconds: f64) -> String {
    let millis = (seconds.max(0.0) * 1000.0).round() as u64;
    format!(
        "{:02}:{:02}:{:03}",
        millis / 60_000,
        millis / 1000 % 60,
        millis % 1000
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_read_like_revolt() {
        assert_eq!(race_time(0.0), "00:00:000");
        assert_eq!(race_time(41.2345), "00:41:235");
        assert_eq!(race_time(133.456), "02:13:456");
    }
}
