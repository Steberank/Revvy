//! UI inmediata. En la fase 0 la pantalla solo muestra el nombre del juego.

pub mod hud;
pub mod room;

pub fn show_drive(ctx: &egui::Context) {
    egui::Window::new("revvy")
        .title_bar(false)
        .anchor(egui::Align2::LEFT_TOP, [12.0, 12.0])
        .show(ctx, |ui| {
            ui.label("WASD mover   Q bajar   E subir   mouse mirar   Shift más rápido");
        });
}
