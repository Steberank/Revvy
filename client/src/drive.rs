//! Vista de manejo: la pista, los autos y los objetos en el motor de Revvy (Rapier +
//! vehículo de Revvy), la cámara de persecución (o una libre) y el sonido.
//!
//! Todo en el espacio de Revvy: el contenido de Re-Volt ya llega traducido por
//! `revvy-formats`, igual que el propio (`.glb`, `car.toml`).

use std::path::{Path, PathBuf};

use anyhow::Context;
use glam::{Mat4, Quat, Vec3};
use revvy_formats::layout::StartSlot;
use revvy_formats::{
    load_car, load_track, CarDef, ObjectKind, ObjectSpawn, SpawnWhen, TrackLoad, TrackObjects,
    VisualMesh,
};
use revvy_physics::{ChaseCamera, Controls, PhysicsWorld, VehicleSound};
use winit::keyboard::KeyCode;

use crate::audio::{Audio, Listener, ObjectCue, ObjectSound};
use crate::config::ClientConfig;
use crate::input::{DriveKeys, FlyKeys, Input};
use crate::render::{CameraView, MAX_OBJECTS};
use crate::ui::HudInfo;

const MOVE_SPEED: f32 = 12.0;
const FAST_SPEED: f32 = 40.0;
/// Un frame no avanza más que esto (el tope del motor).
const MAX_FRAME: f32 = 10.0 / 72.0;
/// Con más autos que puestos, los que sobran van atrás del último, a esta distancia.
const EXTRA_SLOT_GAP: f32 = 1.5;
const MPS_TO_MPH: f32 = 2.236_94;

/// Una carrera por arrancar: la pista y los autos en el orden de la grilla. Pista y autos
/// son ids dentro de `levels/` y `cars/`, o rutas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Race {
    pub level: String,
    pub cars: Vec<String>,
}

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
    background: Option<[u8; 3]>,
    cars: Vec<CarView>,
    world: PhysicsWorld,
    chase: ChaseCamera,
    free: FreeCamera,
    free_mode: bool,
    driven: usize,
    /// Los primeros `players` autos son los de la sala; el resto no tiene conductor.
    players: usize,
    audio: Audio,
    listener_pos: Vec3,
    objects: TrackObjects,
    /// Las apariciones con trigger y desde cuándo pueden disparar (`None`: ya disparó y
    /// no se rearma).
    triggers: Vec<(usize, Option<f32>)>,
    race_time: f32,
}

struct FreeCamera {
    eye: Vec3,
    yaw: f32,
    pitch: f32,
}

impl DriveView {
    pub fn load(config: &ClientConfig, race: &Race) -> anyhow::Result<Self> {
        let content = config.content_dir();
        let level_dir = resolve_content(&content.join("levels"), &race.level);

        tracing::info!(pista = %level_dir.display(), "cargando pista");
        let mut track = load_track(&level_dir, TrackLoad::default())?;
        let collision = track
            .asset
            .collision
            .as_ref()
            .context("la pista no trae colisión")?;
        let mut world = PhysicsWorld::new(collision);
        let objects = std::mem::take(&mut track.asset.objects);
        let mut triggers = Vec::new();
        for (i, spawn) in objects.spawns.iter().enumerate() {
            match spawn.when {
                SpawnWhen::Start => add_object(&mut world, &objects, spawn),
                SpawnWhen::Trigger(_) => triggers.push((i, Some(0.0))),
            }
        }
        tracing::info!(
            objetos = world.objects().len(),
            con_trigger = triggers.len(),
            autos_sin_conductor = objects.car_spawns.len(),
            "objetos de la pista"
        );
        let visual = track.asset.visual.as_ref();

        let mut defs: Vec<(CarDef, Vec3)> = Vec::new();
        for (i, name) in race.cars.iter().enumerate() {
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
        // Los autos sin conductor van después de los de la sala: Tab no llega a ellos.
        let players = defs.len();
        for spawn in &objects.car_spawns {
            let index = world.add_vehicle(&objects.cars[spawn.car].vehicle, spawn.pos, spawn.yaw);
            world.set_self_righting(index, spawn.self_righting);
        }

        let (pos, rot) = world.vehicle(0).pose(1.0);
        let chase = ChaseCamera::new(pos, rot);
        let sound_cars: Vec<(&CarDef, Vec3)> = defs.iter().map(|(car, pos)| (car, *pos)).collect();
        let audio = Audio::new(
            &content,
            &track.asset.sounds,
            &sound_cars,
            &objects.kinds,
            config.sfx_volume,
        );
        let forward = chase.forward();
        let mut cars: Vec<CarView> = defs
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
        cars.extend(objects.car_spawns.iter().map(|spawn| {
            let car = &objects.cars[spawn.car];
            let mut parts = vec![car.body.clone()];
            parts.extend(car.wheels.iter().cloned());
            CarView {
                name: car.name.clone(),
                parts,
                texture: car.texture.clone(),
            }
        }));
        Ok(Self {
            track_meshes: visual.map(|v| v.meshes.clone()).unwrap_or_default(),
            track_textures: visual.map(|v| v.textures.clone()).unwrap_or_default(),
            color_key: visual.is_some_and(|v| v.color_key),
            sky: visual.and_then(|v| v.sky.clone()),
            background: visual.and_then(|v| v.background),
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
            players,
            audio,
            objects,
            triggers,
            race_time: 0.0,
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

    /// El fondo donde no hay cielo.
    pub fn background(&self) -> Option<[u8; 3]> {
        self.background
    }

    pub fn cars(&self) -> &[CarView] {
        &self.cars
    }

    /// Los tipos de objeto de la pista, para subir sus mallas.
    pub fn object_kinds(&self) -> &[ObjectKind] {
        &self.objects.kinds
    }

    /// Tipo y matriz de cada objeto, interpolados entre pasos.
    pub fn object_models(&self) -> Vec<(usize, Mat4)> {
        let alpha = self.world.alpha();
        self.world
            .objects()
            .iter()
            .map(|prop| {
                let (pos, rot) = prop.pose(alpha);
                (prop.kind(), Mat4::from_rotation_translation(rot, pos))
            })
            .collect()
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
        if input.take_pressed(KeyCode::Tab) && self.players > 1 {
            self.driven = (self.driven + 1) % self.players;
            let (pos, rot) = self.world.vehicle(self.driven).pose(self.world.alpha());
            self.chase = ChaseCamera::new(pos, rot);
        }
        let fly = input.fly();
        let keys = input.drive(!self.free_mode);

        let mut controls = vec![Controls::default(); self.cars.len()];
        controls[self.driven] = controls_from(keys);
        self.world.frame(dt, &controls);

        let time_step = dt.clamp(0.0, MAX_FRAME);
        self.race_time += time_step;
        self.fire_triggers();
        let (pos, rot) = self.world.vehicle(self.driven).pose(self.world.alpha());
        self.chase.update(time_step, pos, rot, &self.world);
        if self.free_mode {
            self.free.step(time_step, fly);
        }

        let listener = self.listener(time_step);
        // Los autos sin conductor no suenan, como el chango de Re-Volt.
        let sounds: Vec<VehicleSound> = self
            .world
            .vehicles()
            .iter()
            .take(self.players)
            .map(|vehicle| vehicle.sound(self.world.bodies()))
            .collect();
        let object_sounds = self.object_sounds();
        self.audio
            .update(&sounds, &object_sounds, time_step, &listener);
    }

    /// `TriggerObjectThrower`: el centro de algún auto de la sala entra en la caja y aparece
    /// el objeto. Los autos sin conductor no disparan triggers.
    fn fire_triggers(&mut self) {
        let cars: Vec<Vec3> = self
            .world
            .vehicles()
            .iter()
            .take(self.players)
            .map(|vehicle| vehicle.pose(1.0).0)
            .collect();
        for (spawn, ready_at) in &mut self.triggers {
            let Some(ready) = *ready_at else { continue };
            let spawn = &self.objects.spawns[*spawn];
            let SpawnWhen::Trigger(zone) = &spawn.when else {
                continue;
            };
            if self.race_time < ready || !cars.iter().any(|&car| zone.contains(car)) {
                continue;
            }
            add_object(&mut self.world, &self.objects, spawn);
            *ready_at = zone.rearm.map(|secs| self.race_time + secs);
        }
    }

    /// Lo que suena de los objetos en este frame: los golpes (`AI_BangNoiseHandler`) y las
    /// puntas de los caminos, a todo volumen en el punto de partida (`AI_SliderHandler`).
    fn object_sounds(&mut self) -> Vec<ObjectSound> {
        let knocks = self.world.take_knocks();
        let cues = self.world.take_motion_cues();
        let props = self.world.objects();
        let impacts = knocks.into_iter().filter_map(|(i, knock)| {
            let prop = &props[i];
            let sound = self.objects.kinds[prop.kind()].impact_sound.as_ref()?;
            Some(ObjectSound {
                kind: prop.kind(),
                cue: ObjectCue::Impact,
                pos: prop.pose(1.0).0,
                volume: sound.volume(knock)?,
            })
        });
        let ends = cues.into_iter().filter_map(|(i, cue)| {
            let prop = &props[i];
            Some(ObjectSound {
                kind: prop.kind(),
                cue: cue.into(),
                pos: prop.path_origin()?,
                volume: 127,
            })
        });
        impacts.chain(ends).collect()
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
            cars: self.players,
            free_camera: self.free_mode,
            sound: self.audio.enabled(),
        }
    }
}

/// Pone una aparición en el mundo: con su camino, o suelta con su velocidad.
fn add_object(world: &mut PhysicsWorld, objects: &TrackObjects, spawn: &ObjectSpawn) {
    if world.objects().len() >= MAX_OBJECTS {
        tracing::warn!("demasiados objetos: no aparece otro");
        return;
    }
    let kind = &objects.kinds[spawn.kind];
    match spawn.motion {
        Some(motion) => world.add_moving_object(spawn.kind, kind, spawn.pos, spawn.rot, motion),
        None => world.add_object(spawn.kind, kind, spawn.pos, spawn.rot, spawn.velocity),
    };
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

/// `cargo run -p revvy-client -- <pista> [auto] [más autos…]` arranca en la pista, sin
/// menú. Sin autos van `car` y `extra_cars` de la config.
pub fn cli_race(config: &ClientConfig) -> Option<Race> {
    let args: Vec<String> = std::env::args().skip(1).filter(|arg| !arg.starts_with('-')).collect();
    let defaults: Vec<String> = std::iter::once(config.car.clone()).chain(config.extra_cars.iter().cloned()).collect();
    race_from_args(&args, &defaults)
}

fn race_from_args(args: &[String], default_cars: &[String]) -> Option<Race> {
    let (level, cars) = args.split_first()?;
    let cars = if cars.is_empty() { default_cars } else { cars };
    Some(Race {
        level: level.clone(),
        cars: cars.to_vec(),
    })
}

/// Un id dentro de `base`, o una ruta si tiene separadores.
pub fn resolve_content(base: &Path, spec: &str) -> PathBuf {
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
    fn cli_args_skip_the_menu() {
        let defaults = ["phim_calcure".to_string(), "revvy_buggy".to_string()];
        assert_eq!(race_from_args(&[], &defaults), None);
        let race = race_from_args(&["nhood1".into()], &defaults).unwrap();
        assert_eq!(race.level, "nhood1");
        assert_eq!(race.cars, defaults);
        let race = race_from_args(&["nhood1".into(), "revvy_buggy".into()], &defaults).unwrap();
        assert_eq!(race.cars, ["revvy_buggy"]);
    }

    #[test]
    fn extra_cars_line_up_behind_the_last_slot() {
        let grid = [StartSlot { pos: Vec3::ZERO, yaw: 0.0 }];
        let (pos, _) = start_slot(&grid, 2);
        assert!((pos - Vec3::new(0.0, 0.0, -3.0)).length() < 1e-5);
    }
}
