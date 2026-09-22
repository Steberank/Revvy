//! UI inmediata. En la fase 0 la pantalla solo muestra el nombre del juego.

pub mod hud;
pub mod room;

pub fn show_boot(ui: &mut egui::Ui) {
    ui.centered_and_justified(|ui| {
        ui.heading("Revvy");
    });
}
