//! Vista de manejo: la pista legacy, el auto con la física portada de Re-Volt, la
//! cámara de persecución de `camera.cpp` (o una libre) y el sonido.
//!
//! La física y el sonido trabajan en el espacio de Re-Volt (5 mm, Y abajo). Solo la
//! cámara y las matrices de dibujo pasan a Revvy (m, Y arriba).

use std::path::{Path, PathBuf};

use anyhow::Context;
use glam::{Mat4, Vec3};
use revvy_formats::{load_car, load_track, TrackLoad, VisualMesh};
use revvy_physics::revolt::math::{build_look_matrix_forward, vec_mul_mat};
use revvy_physics::revolt::units::OGU2MPH_SPEED;
use revvy_physics::revolt::{Car, CollWorld, Controls, FollowCamera, Simulation};
use revvy_physics::vehicle_controller::{model_matrix, to_revolt_dir, to_revolt_point, to_revvy_dir, to_revvy_point};
use winit::keyboard::KeyCode;

use crate::audio::{Audio, Listener};
use crate::config::ClientConfig;
use crate::input::{DriveKeys, FlyKeys, Input};
use crate::render::CameraView;
use crate::ui::HudInfo;

const MOVE_SPEED: f32 = 12.0;
const FAST_SPEED: f32 = 40.0;
/// `CTRL_RANGE_MAX`: una tecla vale el recorrido entero del stick.
const CTRL_RANGE_MAX: f32 = 127.0;

pub struct DriveView {
    track_meshes: Vec<VisualMesh>,
    track_textures: Vec<(i16, image::RgbaImage)>,
    sky: Option<[image::RgbaImage; 6]>,
    /// Chasis y las cuatro ruedas (FL, FR, BL, BR).
    car_parts: Vec<Vec<VisualMesh>>,
    car_texture: Option<image::RgbaImage>,
    sim: Simulation,
    follow: FollowCamera,
    free: FreeCamera,
    free_mode: bool,
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
        let (level, car) = cli_content(&config.level, &config.car);
        let level_dir = resolve_content(&content.join("levels"), &level);
        let car_dir = resolve_content(&content.join("cars"), &car);

        tracing::info!(level = %level_dir.display(), "cargando pista legacy");
        let track = load_track(&level_dir, TrackLoad::default())?;
        let legacy = track
            .asset
            .legacy
            .clone()
            .context("la pista no trae datos de Re-Volt para la física")?;
        let visual = track.asset.visual.as_ref();
        let track_meshes = visual.map(|v| v.meshes.clone()).unwrap_or_default();
        let track_textures = visual.map(|v| v.textures.clone()).unwrap_or_default();
        tracing::info!(meshes = track_meshes.len(), "pista lista");

        tracing::info!(car = %car_dir.display(), "cargando auto");
        let car_def = load_car(&car_dir)?;
        tracing::info!(
            name = %car_def.info.name,
            top_speed_mph = car_def.info.top_speed_mph,
            spheres = car_def.hull_spheres.len(),
            "auto listo"
        );
        let mut car_parts = vec![car_def.body.clone()];
        car_parts.extend(car_def.wheels.iter().cloned());

        let start = Car::start_grid(legacy.start_pos, legacy.start_rot, legacy.start_grid_type);
        let world = CollWorld::new(&legacy);
        tracing::info!(
            polys = world.polys.len(),
            mundo = world.n_world_polys,
            celdas = world.cells.len(),
            "colisión de Re-Volt"
        );
        let car = Car::new(&car_def.info, &car_def.hull_spheres);
        let sim = Simulation::new(world, car, start);
        let follow = FollowCamera::new(sim.car.body.centre.pos, &sim.car.body.centre.wmatrix);
        let audio = Audio::new(&content, &legacy, &car_def, start.0, config.sfx_volume);

        let eye = to_revvy_point(follow.wpos);
        let forward = to_revvy_dir(follow.wmatrix.l);
        Ok(Self {
            track_meshes,
            track_textures,
            sky: load_sky(&level_dir),
            car_parts,
            car_texture: car_def.texture.clone(),
            listener_pos: follow.wpos,
            sim,
            follow,
            free: FreeCamera {
                eye,
                yaw: forward.x.atan2(forward.z),
                pitch: forward.y.clamp(-1.0, 1.0).asin(),
            },
            free_mode: false,
            audio,
        })
    }

    pub fn track_meshes(&self) -> &[VisualMesh] {
        &self.track_meshes
    }

    pub fn track_textures(&self) -> &[(i16, image::RgbaImage)] {
        &self.track_textures
    }

    pub fn sky(&self) -> Option<&[image::RgbaImage; 6]> {
        self.sky.as_ref()
    }

    pub fn car_parts(&self) -> &[Vec<VisualMesh>] {
        &self.car_parts
    }

    pub fn car_texture(&self) -> Option<&image::RgbaImage> {
        self.car_texture.as_ref()
    }

    /// Un frame: mandos, física, cámara y sonido.
    pub fn step(&mut self, dt: f32, input: &mut Input) {
        if input.take_pressed(KeyCode::KeyC) {
            self.free_mode = !self.free_mode;
            if self.free_mode {
                let camera = self.chase_camera();
                let forward = (camera.target - camera.eye).normalize_or_zero();
                self.free = FreeCamera {
                    eye: camera.eye,
                    yaw: forward.x.atan2(forward.z),
                    pitch: forward.y.clamp(-1.0, 1.0).asin(),
                };
            }
        }
        let fly = input.fly();
        let keys = input.drive(!self.free_mode);

        let report = self.sim.frame(dt, controls(keys));
        // El mismo `TimeStep` que usó la física.
        let time_step = dt.clamp(0.0, 10.0 / 72.0);
        let body = &self.sim.car.body.centre;
        self.follow.update(time_step, body.pos, &body.wmatrix, &self.sim.level);
        if self.free_mode {
            self.free.step(time_step, fly);
        }

        let listener = self.listener(time_step);
        self.audio.update(&report.sfx, time_step, &listener);
    }

    /// Cámara que escucha, en el espacio de Re-Volt.
    fn listener(&mut self, time_step: f32) -> Listener {
        if !self.free_mode {
            self.listener_pos = self.follow.wpos;
            return Listener {
                pos: self.follow.wpos,
                mat: self.follow.wmatrix,
                vel: self.follow.vel,
            };
        }
        let camera = self.free.view();
        let pos = to_revolt_point(camera.eye);
        let look = to_revolt_dir(camera.target - camera.eye);
        let vel = if time_step > 1e-5 {
            (pos - self.listener_pos) / time_step
        } else {
            Vec3::ZERO
        };
        self.listener_pos = pos;
        Listener {
            pos,
            mat: build_look_matrix_forward(pos, pos + look),
            vel,
        }
    }

    fn chase_camera(&self) -> CameraView {
        let eye = to_revvy_point(self.follow.wpos);
        CameraView {
            eye,
            target: eye + to_revvy_dir(self.follow.wmatrix.l),
        }
    }

    pub fn camera(&self) -> CameraView {
        if self.free_mode {
            self.free.view()
        } else {
            self.chase_camera()
        }
    }

    /// `DrawCar`: el chasis en `Pos + BodyOffset` y cada rueda en `WPos` con su `WMatrix`.
    pub fn car_models(&self) -> [Mat4; 5] {
        let car = &self.sim.car;
        let body = &car.body.centre;
        let body_pos = body.pos + vec_mul_mat(car.body_offset, &body.wmatrix);
        let mut models = [model_matrix(&body.wmatrix, body_pos); 5];
        for (i, wheel) in car.wheels.iter().enumerate() {
            models[i + 1] = model_matrix(&wheel.wmatrix, wheel.wpos);
        }
        models
    }

    pub fn hud(&self) -> HudInfo {
        HudInfo {
            speed_mph: self.sim.car.body.centre.vel.length() * OGU2MPH_SPEED,
            free_camera: self.free_mode,
            sound: self.audio.enabled(),
        }
    }
}

/// `CRD_KeyboardInput` + `s_RationaliseControl`: teclas opuestas se anulan.
fn controls(keys: DriveKeys) -> Controls {
    let dx = match (keys.left, keys.right) {
        (true, false) => -CTRL_RANGE_MAX,
        (false, true) => CTRL_RANGE_MAX,
        _ => 0.0,
    };
    let dy = match (keys.accelerate, keys.brake) {
        (true, false) => -CTRL_RANGE_MAX,
        (false, true) => CTRL_RANGE_MAX,
        _ => 0.0,
    };
    Controls {
        dx,
        dy,
        reset: keys.reset,
    }
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
    Vec3::new(
        yaw.sin() * pitch.cos(),
        pitch.sin(),
        yaw.cos() * pitch.cos(),
    )
}

/// `cargo run -p revvy-client -- <nivel> [auto]`.
fn cli_content(level: &str, car: &str) -> (String, String) {
    if cfg!(test) {
        return (level.to_string(), car.to_string());
    }
    let mut args = std::env::args().skip(1).filter(|arg| !arg.starts_with('-'));
    let level = args.next().unwrap_or_else(|| level.to_string());
    let car = args.next().unwrap_or_else(|| car.to_string());
    (level, car)
}

fn resolve_content(base: &Path, spec: &str) -> PathBuf {
    let path = PathBuf::from(spec);
    if path.is_absolute() || spec.contains('/') || spec.contains('\\') {
        path
    } else {
        base.join(spec)
    }
}

fn load_sky(level: &Path) -> Option<[image::RgbaImage; 6]> {
    // `RenderSkybox`: ft, rt, bk, lt, tp, bt sobre +Z, -X, -Z, +X, arriba, abajo
    // en el archivo. Con el cambio de ejes eso es +Z, +X, -Z, -X, +Y, -Y.
    let names = ["sky_rt", "sky_lt", "sky_tp", "sky_bt", "sky_ft", "sky_bk"];
    let mut faces = Vec::with_capacity(6);
    for name in names {
        let image = revvy_formats::load_bmp(&level.join(format!("{name}.bmp"))).ok()?;
        faces.push(image);
    }
    let faces: [image::RgbaImage; 6] = faces.try_into().ok()?;
    let (width, height) = (faces[0].width(), faces[0].height());
    if width != height
        || faces
            .iter()
            .any(|face| face.width() != width || face.height() != height)
    {
        return None;
    }
    Some(faces)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opposite_keys_cancel_like_rationalise_control() {
        let c = controls(DriveKeys {
            accelerate: true,
            brake: true,
            left: true,
            right: false,
            reset: false,
        });
        assert_eq!(c.dy, 0.0);
        assert_eq!(c.dx, -CTRL_RANGE_MAX);
    }
}
