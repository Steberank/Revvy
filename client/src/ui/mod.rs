//! UI inmediata: ayuda de teclas y velocímetro.

pub mod hud;
pub mod room;

/// Lo que el frame muestra encima de la escena.
#[derive(Clone, Debug, Default)]
pub struct HudInfo {
    /// Velocidad del auto en mph (`OGU2MPH_SPEED`), como el velocímetro de Re-Volt.
    pub speed_mph: f32,
    /// El auto que se maneja.
    pub car_name: String,
    /// Autos en la pista.
    pub cars: usize,
    pub free_camera: bool,
    /// `false` si no hay dispositivo de audio: el juego corre en silencio.
    pub sound: bool,
}

pub fn show_drive(ctx: &egui::Context, hud: &HudInfo) {
    egui::Window::new("revvy")
        .title_bar(false)
        .anchor(egui::Align2::LEFT_TOP, [12.0, 12.0])
        .show(ctx, |ui| {
            ui.label("↑/W acelerar   ↓/S frenar y reversa   ←/A →/D doblar   R enderezar");
            if hud.free_camera {
                ui.label("C cámara del auto   WASD mover   Q bajar   E subir   mouse mirar   Shift rápido   (flechas manejan)");
            } else {
                ui.label("C cámara libre");
            }
            if hud.cars > 1 {
                ui.label("Tab cambiar de auto");
            }
            if !hud.sound {
                ui.label("sin dispositivo de audio");
            }
        });
    egui::Window::new("revvy-speed")
        .title_bar(false)
        .anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -16.0])
        .show(ctx, |ui| {
            ui.label(&hud.car_name);
            ui.heading(format!("{:>3.0} mph", hud.speed_mph));
        });
}
