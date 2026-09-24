//! Vista de manejo: la pista, los autos en el motor de Revvy (Rapier + vehículo de
//! Revvy), la cámara de persecución (o una libre) y el sonido.
//!
//! Todo en el espacio de Revvy: el contenido de Re-Volt ya llega traducido por
//! `revvy-formats`, igual que el propio (`.glb`, `car.toml`).

use std::path::{Path, PathBuf};

use anyhow::Context;
use glam::{Mat4, Quat, Vec3};
use revvy_formats::layout::StartSlot;
use revvy_formats::{load_car, load_track, CarDef, TrackLoad, VisualMesh};
use revvy_physics::{ChaseCamera, Controls, PhysicsWorld, VehicleSound};
use winit::keyboard::KeyCode;

use crate::audio::{Audio, Listener};
use crate::config::ClientConfig;
use crate::input::{DriveKeys, FlyKeys, Input};
use crate::render::CameraView;
use crate::ui::HudInfo;

const MOVE_SPEED: f32 = 12.0;
const FAST_SPEED: f32 = 40.0;
/// Un frame no avanza más que esto (el tope del motor).
const MAX_FRAME: f32 = 10.0 / 72.0;
/// Con más autos que puestos, los que sobran van atrás del último, a esta distancia.
const EXTRA_SLOT_GAP: f32 = 1.5;
const MPS_TO_MPH: f32 = 2.236_94;

/// Lo que el render necesita de un auto.
pub struct CarView {
    pub name: String,
    /// Chasis y las cuatro ruedas (FL, FR, BL, BR).
    pub parts: Vec<Vec<VisualMesh>>,
    pub texture: Option<image::RgbaImage>,
}

pub struct DriveView {
    track_meshes: Vec<VisualMesh>,
    track_textures: Vec<(i16, image::RgbaImage)>,
    color_key: bool,
    sky: Option<[image::RgbaImage; 6]>,
    cars: Vec<CarView>,
    world: PhysicsWorld,
    chase: ChaseCamera,
    free: FreeCamera,
    free_mode: bool,
    driven: usize,
    audio: Audio,
    listener_pos: Vec3,
}

struct FreeCamera {
    eye: Vec3,
    yaw: f32,
    pitch: f32,
}

impl DriveView {
    pub fn load(config: &ClientConfig) -> anyhow::Result<Self> {
        let content = config.content_dir();
        let (level, car_names) = cli_content(&config.level, &config.car, &config.extra_cars);
        let level_dir = resolve_content(&content.join("levels"), &level);

        tracing::info!(pista = %level_dir.display(), "cargando pista");
        let track = load_track(&level_dir, TrackLoad::default())?;
        let collision = track.asset.collision.as_ref().context("la pista no trae colisión")?;
        let mut world = PhysicsWorld::new(collision);
        let visual = track.asset.visual.as_ref();

        let mut defs: Vec<(CarDef, Vec3)> = Vec::new();
        for (i, name) in car_names.iter().enumerate() {
            let car_dir = resolve_content(&content.join("cars"), name);
            tracing::info!(auto = %car_dir.display(), "cargando auto");
            let car = load_car(&car_dir)?;
            let (pos, yaw) = start_slot(&track.asset.layout.start_grid, i);
            world.add_vehicle(&car.vehicle, pos, yaw);
            tracing::info!(
                nombre = %car.name,
                origen = if car.revolt.is_some() { "Re-Volt" } else { "propio" },
                tope_mph = car.vehicle.top_speed * MPS_TO_MPH,
                "auto listo"
            );
            defs.push((car, pos));
        }

        let (pos, rot) = world.vehicle(0).pose(1.0);
        let chase = ChaseCamera::new(pos, rot);
        let sound_cars: Vec<(&CarDef, Vec3)> = defs.iter().map(|(car, pos)| (car, *pos)).collect();
        let audio = Audio::new(&content, &track.asset.sounds, &sound_cars, config.sfx_volume);
        let forward = chase.forward();
        let cars = defs
            .into_iter()
            .map(|(car, _)| {
                let mut parts = vec![car.body];
                parts.extend(car.wheels);
                CarView {
                    name: car.name,
                    parts,
                    texture: car.texture,
                }
            })
            .collect();
        Ok(Self {
            track_meshes: visual.map(|v| v.meshes.clone()).unwrap_or_default(),
            track_textures: visual.map(|v| v.textures.clone()).unwrap_or_default(),
            color_key: visual.is_some_and(|v| v.color_key),
            sky: visual.and_then(|v| v.sky.clone()),
            cars,
            listener_pos: chase.eye,
            free: FreeCamera {
                eye: chase.eye,
                yaw: forward.x.atan2(forward.z),
                pitch: forward.y.clamp(-1.0, 1.0).asin(),
            },
            chase,
            world,
            free_mode: false,
            driven: 0,
            audio,
        })
    }

    pub fn track_meshes(&self) -> &[VisualMesh] {
        &self.track_meshes
    }

    pub fn track_textures(&self) -> &[(i16, image::RgbaImage)] {
        &self.track_textures
    }

    /// El negro puro de las texturas de la pista no se dibuja (pistas de Re-Volt).
    pub fn color_key(&self) -> bool {
        self.color_key
    }

    pub fn sky(&self) -> Option<&[image::RgbaImage; 6]> {
        self.sky.as_ref()
    }

    pub fn cars(&self) -> &[CarView] {
        &self.cars
    }

    /// Un frame: mandos, física, cámara y sonido.
    pub fn step(&mut self, dt: f32, input: &mut Input) {
        if input.take_pressed(KeyCode::KeyC) {
            self.free_mode = !self.free_mode;
            if self.free_mode {
                let forward = self.chase.forward();
                self.free = FreeCamera {
                    eye: self.chase.eye,
                    yaw: forward.x.atan2(forward.z),
                    pitch: forward.y.clamp(-1.0, 1.0).asin(),
                };
            }
        }
        if input.take_pressed(KeyCode::Tab) && self.cars.len() > 1 {
            self.driven = (self.driven + 1) % self.cars.len();
            let (pos, rot) = self.world.vehicle(self.driven).pose(self.world.alpha());
            self.chase = ChaseCamera::new(pos, rot);
        }
        let fly = input.fly();
        let keys = input.drive(!self.free_mode);

        let mut controls = vec![Controls::default(); self.cars.len()];
        controls[self.driven] = controls_from(keys);
        self.world.frame(dt, &controls);

        let time_step = dt.clamp(0.0, MAX_FRAME);
        let (pos, rot) = self.world.vehicle(self.driven).pose(self.world.alpha());
        self.chase.update(time_step, pos, rot, &self.world);
        if self.free_mode {
            self.free.step(time_step, fly);
        }

        let listener = self.listener(time_step);
        let sounds: Vec<VehicleSound> = self
            .world
            .vehicles()
            .iter()
            .map(|vehicle| vehicle.sound(self.world.bodies()))
            .collect();
        self.audio.update(&sounds, time_step, &listener);
    }

    /// La cámara que escucha.
    fn listener(&mut self, time_step: f32) -> Listener {
        let camera = self.camera();
        let forward = (camera.target - camera.eye).normalize_or_zero();
        let vel = if self.free_mode {
            if time_step > 1e-5 {
                (camera.eye - self.listener_pos) / time_step
            } else {
                Vec3::ZERO
            }
        } else {
            self.chase.vel
        };
        self.listener_pos = camera.eye;
        Listener {
            pos: camera.eye,
            forward,
            up: Vec3::Y,
            vel,
        }
    }

    pub fn camera(&self) -> CameraView {
        if self.free_mode {
            self.free.view()
        } else {
            CameraView {
                eye: self.chase.eye,
                target: self.chase.target,
            }
        }
    }

    /// Chasis y ruedas de cada auto, cinco matrices por auto, interpoladas entre pasos.
    pub fn car_models(&self) -> Vec<Mat4> {
        let alpha = self.world.alpha();
        self.world.vehicles().iter().flat_map(|vehicle| vehicle.models(alpha)).collect()
    }

    pub fn hud(&self) -> HudInfo {
        HudInfo {
            speed_mph: self.world.vehicle(self.driven).velocity(self.world.bodies()).length() * MPS_TO_MPH,
            car_name: self.cars[self.driven].name.clone(),
            cars: self.cars.len(),
            free_camera: self.free_mode,
            sound: self.audio.enabled(),
        }
    }
}

/// Teclas del auto: las opuestas se anulan, como `s_RationaliseControl`.
fn controls_from(keys: DriveKeys) -> Controls {
    let steer = match (keys.left, keys.right) {
        (true, false) => -1.0,
        (false, true) => 1.0,
        _ => 0.0,
    };
    let throttle = match (keys.accelerate, keys.brake) {
        (true, false) => 1.0,
        (false, true) => -1.0,
        _ => 0.0,
    };
    Controls {
        steer,
        throttle,
        reset: keys.reset,
    }
}

/// El puesto `i` de la grilla. Si no alcanza, detrás del último.
fn start_slot(grid: &[StartSlot], i: usize) -> (Vec3, f32) {
    if let Some(slot) = grid.get(i) {
        return (slot.pos, slot.yaw);
    }
    let Some(last) = grid.last() else {
        return (Vec3::new(0.0, 0.5, 0.0), 0.0);
    };
    let back = Quat::from_rotation_y(last.yaw) * Vec3::NEG_Z;
    let extra = (i + 1 - grid.len()) as f32 * EXTRA_SLOT_GAP;
    (last.pos + back * extra, last.yaw)
}

impl FreeCamera {
    fn step(&mut self, dt: f32, keys: FlyKeys) {
        self.yaw -= keys.look_dx * 0.003;
        self.pitch = (self.pitch - keys.look_dy * 0.003).clamp(-1.4, 1.4);
        let look = look_dir(self.yaw, self.pitch);
        let right = look.cross(Vec3::Y).normalize_or_zero();
        let mut wish = Vec3::ZERO;
        if keys.forward {
            wish += look;
        }
        if keys.back {
            wish -= look;
        }
        if keys.left {
            wish -= right;
        }
        if keys.right {
            wish += right;
        }
        if keys.up {
            wish += Vec3::Y;
        }
        if keys.down {
            wish -= Vec3::Y;
        }
        if wish.length_squared() > 0.0 {
            let speed = if keys.fast { FAST_SPEED } else { MOVE_SPEED };
            self.eye += wish.normalize() * speed * dt.clamp(0.0, 0.1);
        }
    }

    fn view(&self) -> CameraView {
        CameraView {
            eye: self.eye,
            target: self.eye + look_dir(self.yaw, self.pitch),
        }
    }
}

fn look_dir(yaw: f32, pitch: f32) -> Vec3 {
    Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), yaw.cos() * pitch.cos())
}

/// `cargo run -p revvy-client -- <pista> [auto] [más autos…]`.
fn cli_content(level: &str, car: &str, extra: &[String]) -> (String, Vec<String>) {
    let mut cars: Vec<String> = std::iter::once(car.to_string()).chain(extra.iter().cloned()).collect();
    if cfg!(test) {
        return (level.to_string(), cars);
    }
    let args: Vec<String> = std::env::args().skip(1).filter(|arg| !arg.starts_with('-')).collect();
    let level = args.first().cloned().unwrap_or_else(|| level.to_string());
    if args.len() > 1 {
        cars = args[1..].to_vec();
    }
    (level, cars)
}

fn resolve_content(base: &Path, spec: &str) -> PathBuf {
    let path = PathBuf::from(spec);
    if path.is_absolute() || spec.contains('/') || spec.contains('\\') {
        path
    } else {
        base.join(spec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opposite_keys_cancel_like_rationalise_control() {
        let c = controls_from(DriveKeys {
            accelerate: true,
            brake: true,
            left: true,
            right: false,
            reset: false,
        });
        assert_eq!(c.throttle, 0.0);
        assert_eq!(c.steer, -1.0);
    }

    #[test]
    fn extra_cars_line_up_behind_the_last_slot() {
        let grid = [StartSlot { pos: Vec3::ZERO, yaw: 0.0 }];
        let (pos, _) = start_slot(&grid, 2);
        assert!((pos - Vec3::new(0.0, 0.0, -3.0)).length() < 1e-5);
    }
}
