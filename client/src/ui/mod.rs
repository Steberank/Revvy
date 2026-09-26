//! UI inmediata: el menú, y en la carrera la ayuda de teclas, el velocímetro, el panel de
//! la carrera, el aviso de contramano y los resultados.

pub mod hud;
pub mod menu;
mod results;
pub mod room;
mod theme;

use egui::{Color32, CornerRadius, Id, LayerId, Order};

/// Lo que el frame muestra encima de la escena.
#[derive(Clone, Debug, Default)]
pub struct HudInfo {
    /// Velocidad del auto en mph (`OGU2MPH_SPEED`), como el velocímetro de Re-Volt.
    pub speed_mph: f32,
    /// El auto que se maneja.
    pub car_name: String,
    /// Autos que corren: Tab cambia entre ellos.
    pub cars: usize,
    pub free_camera: bool,
    /// `false` si no hay dispositivo de audio: el juego corre en silencio.
    pub sound: bool,
    /// La carrera del auto que se maneja. `None` en una pista sin zonas ni POS nodes.
    pub race: Option<RaceHud>,
    /// Negro encima de todo, de 0 a 1: la pantalla sale del negro después de reaparecer.
    pub fade: f32,
    /// Terminaron los jugadores: los que llegaron, en orden, con su tiempo total.
    pub results: Option<Vec<(String, f64)>>,
}

/// La carrera del auto que se maneja.
#[derive(Clone, Debug, Default)]
pub struct RaceHud {
    /// La vuelta que corre, desde 1, y las de la carrera.
    pub lap: u32,
    pub laps: u32,
    /// Su puesto, desde 1, entre los `racers` autos que corren.
    pub position: usize,
    pub racers: usize,
    /// Segundos desde la largada; si terminó, su tiempo final.
    pub time: f64,
    /// Lo que va de la vuelta. `None` antes de cruzar la línea por primera vez.
    pub lap_time: Option<f64>,
    pub last_lap: Option<f64>,
    pub best_lap: Option<f64>,
    pub wrong_way: bool,
}

pub fn show_drive(ui: &mut egui::Ui, hud: &HudInfo) {
    let ctx = ui.ctx().clone();
    let screen = ui.max_rect();
    let painter = ui.painter().clone();
    let time = ctx.input(|input| input.time);
    match &hud.results {
        Some(results) => results::show(&painter, screen, results, time),
        None => {
            keys_and_speed(&ctx, hud);
            if let Some(race) = &hud.race {
                hud::race_panel(&painter, screen, race);
                if race.wrong_way {
                    hud::wrong_way::show(&painter, screen, time);
                }
            }
        }
    }
    if hud.fade > 0.0 {
        let alpha = (hud.fade.clamp(0.0, 1.0) * 255.0) as u8;
        ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("revvy-fade")))
            .rect_filled(screen, CornerRadius::ZERO, Color32::from_black_alpha(alpha));
    }
}

fn keys_and_speed(ctx: &egui::Context, hud: &HudInfo) {
    egui::Window::new("revvy")
        .title_bar(false)
        .anchor(egui::Align2::LEFT_TOP, [12.0, 12.0])
        .show(ctx, |ui| {
            // Sin los caracteres de flecha: la fuente de egui no los trae.
            ui.label("Flechas o WASD manejar   R enderezar   Inicio reposicionar   Esc volver al menú");
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
