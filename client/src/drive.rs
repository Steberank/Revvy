//! Vista de manejo: la pista, los autos y los objetos en el motor de Revvy (Rapier +
//! vehículo de Revvy), la carrera de `revvy-core` (vueltas, puestos, contramano y
//! reposición), la cámara de persecución (o una libre) y el sonido.
//!
//! Todo en el espacio de Revvy: el contenido de Re-Volt ya llega traducido por
//! `revvy-formats`, igual que el propio (`.glb`, `car.toml`).

use std::path::{Path, PathBuf};

use anyhow::Context;
use glam::{Mat4, Quat, Vec3};
use revvy_core::race::{CarPose, Race, RaceEvent, RaceTrack};
use revvy_core::rules::GameplayRules;
use revvy_formats::layout::StartSlot;
use revvy_formats::{
    load_car, load_track, CarDef, ObjectSpawn, SpawnWhen, TrackAnimations, TrackLoad, TrackObjects,
    VisualMesh,
};
use revvy_physics::{ChaseCamera, Controls, PhysicsWorld, Vehicle, VehicleSound};
use winit::keyboard::KeyCode;

use crate::audio::{Audio, Listener, ObjectCue, ObjectSound};
use crate::config::ClientConfig;
use crate::input::{DriveKeys, FlyKeys, Input};
use crate::render::{CameraView, ObjectMeshes, MAX_OBJECTS};
use crate::ui::{HudInfo, RaceHud};

const MOVE_SPEED: f32 = 12.0;
const FAST_SPEED: f32 = 40.0;
/// Un frame no avanza más que esto (el tope del motor).
const MAX_FRAME: f32 = 10.0 / 72.0;
/// Con más autos que puestos, los que sobran van atrás del último, a esta distancia.
const EXTRA_SLOT_GAP: f32 = 1.5;
const MPS_TO_MPH: f32 = 2.236_94;
/// Reaparecer (`CAI_ResetCar`): el piso se busca desde este alto sobre el POS node y hasta
/// esta profundidad, y el auto queda esta altura arriba del piso (Re-Volt lo sube 100
/// unidades). Sin piso, queda esa altura arriba del nodo.
const RESPAWN_PROBE: f32 = 0.5;
const RESPAWN_DEPTH: f32 = 5.0;
const RESPAWN_LIFT: f32 = 0.5;
/// Después de reaparecer, la pantalla sale del negro en este tiempo (s).
const RESPAWN_FADE: f32 = 0.5;

/// Una carrera por arrancar: la pista, quién corre y cuántas vueltas. Pista y autos son ids
/// dentro de `levels/` y `cars/`, o rutas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RaceSetup {
    pub level: String,
    /// En el orden de la grilla: primero los jugadores de la sala.
    pub entrants: Vec<Entrant>,
    /// Vueltas de la sala.
    pub laps: u32,
}

/// Un auto de la grilla.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entrant {
    pub car: String,
    /// El jugador que lo maneja. `None`: un auto extra de la config, sin jugador; en la
    /// carrera lleva el nombre del auto.
    pub player: Option<String>,
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
    /// Los primeros `players` autos corren (los de la sala y los extra); el resto no tiene
    /// conductor.
    players: usize,
    /// De esos, los primeros `humans` son de jugadores: cuando terminan todos, se ven los
    /// resultados.
    humans: usize,
    /// El nombre de cada auto que corre: su jugador, o el del auto.
    names: Vec<String>,
    race: Race,
    grid: Vec<StartSlot>,
    /// Lo que falta para que la pantalla salga del negro después de reaparecer (s).
    fade: f32,
    audio: Audio,
    listener_pos: Vec3,
    objects: TrackObjects,
    /// Las apariciones con trigger y desde cuándo pueden disparar (`None`: ya disparó y
    /// no se rearma), en segundos de carrera.
    triggers: Vec<(usize, Option<f64>)>,
    /// Los objetos animados (RVGL), con el reloj de la carrera. Sus huesos se dibujan
    /// después de los objetos de la física y ocupan lugares de `MAX_OBJECTS`.
    animations: TrackAnimations,
}

struct FreeCamera {
    eye: Vec3,
    yaw: f32,
    pitch: f32,
}

impl DriveView {
    /// `rules` da el resto de las reglas; las vueltas salen de `setup`.
    pub fn load(
        config: &ClientConfig,
        setup: &RaceSetup,
        rules: &GameplayRules,
    ) -> anyhow::Result<Self> {
        let content = config.content_dir();
        let level_dir = resolve_content(&content.join("levels"), &setup.level);

        tracing::info!(pista = %level_dir.display(), "cargando pista");
        let mut track = load_track(&level_dir, TrackLoad::default())?;
        let collision = track
            .asset
            .collision
            .as_ref()
            .context("la pista no trae colisión")?;
        let mut world = PhysicsWorld::new(collision);
        let objects = std::mem::take(&mut track.asset.objects);
        let animations = track
            .asset
            .visual
            .as_mut()
            .map(|visual| std::mem::take(&mut visual.animations))
            .unwrap_or_default();
        let room = object_room(&animations);
        let mut triggers = Vec::new();
        for (i, spawn) in objects.spawns.iter().enumerate() {
            match spawn.when {
                SpawnWhen::Start => add_object(&mut world, &objects, spawn, room),
                SpawnWhen::Trigger(_) => triggers.push((i, Some(0.0))),
            }
        }
        tracing::info!(
            objetos = world.objects().len(),
            con_trigger = triggers.len(),
            autos_sin_conductor = objects.car_spawns.len(),
            animados = animations.objects.len(),
            "objetos de la pista"
        );
        let visual = track.asset.visual.as_ref();

        let mut defs: Vec<(CarDef, Vec3)> = Vec::new();
        let mut names = Vec::new();
        for (i, entrant) in setup.entrants.iter().enumerate() {
            let car_dir = resolve_content(&content.join("cars"), &entrant.car);
            tracing::info!(auto = %car_dir.display(), "cargando auto");
            let car = load_car(&car_dir)?;
            names.push(entrant.player.clone().unwrap_or_else(|| car.name.clone()));
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
        let humans = setup
            .entrants
            .iter()
            .take_while(|entrant| entrant.player.is_some())
            .count();

        let race_rules = GameplayRules {
            laps: setup.laps,
            ..rules.clone()
        };
        let poses: Vec<CarPose> = world.vehicles()[..players].iter().map(car_pose).collect();
        let race = Race::new(
            RaceTrack::new(&track.asset.layout, Some(collision)),
            &race_rules,
            &poses,
        );
        if race.has_laps() {
            tracing::info!(vueltas = race.laps(), jugadores = humans, "carrera");
        } else {
            tracing::info!("la pista no tiene zonas ni POS nodes: se maneja sin vueltas");
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
            humans,
            names,
            race,
            grid: track.asset.layout.start_grid.clone(),
            fade: 0.0,
            audio,
            objects,
            triggers,
            animations,
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

    /// Las mallas de cada tipo de objeto, para subirlas: los de la física y después los
    /// modelos de los huesos animados, que usan las páginas de la pista.
    pub fn object_meshes(&self) -> Vec<ObjectMeshes<'_>> {
        let kinds = self
            .objects
            .kinds
            .iter()
            .map(|kind| (kind.meshes.as_slice(), kind.textures.as_slice()));
        let bones = self
            .animations
            .models
            .iter()
            .map(|meshes| -> ObjectMeshes<'_> { (meshes.as_slice(), &[]) });
        kinds.chain(bones).collect()
    }

    /// Tipo y matriz de cada objeto, interpolados entre pasos, y de cada hueso animado en
    /// este momento de la carrera. El tipo es el índice en `object_meshes`.
    pub fn object_models(&self) -> Vec<(usize, Mat4)> {
        let alpha = self.world.alpha();
        let mut models: Vec<(usize, Mat4)> = self
            .world
            .objects()
            .iter()
            .map(|prop| {
                let (pos, rot) = prop.pose(alpha);
                (prop.kind(), Mat4::from_rotation_translation(rot, pos))
            })
            .collect();
        let first_bone = self.objects.kinds.len();
        let bones = self.animations.instances(self.race.time() as f32);
        models.extend(
            bones
                .into_iter()
                .map(|(model, matrix)| (first_bone + model, matrix)),
        );
        models
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
        // Terminada la carrera, el auto sigue solo, sin mandos.
        let finished = self.race.racer(self.driven).finish_time().is_some();
        if input.take_pressed(KeyCode::Home) && !finished {
            tracing::info!(auto = self.driven, "reposición pedida");
            self.respawn(self.driven);
        }

        let mut controls = vec![Controls::default(); self.cars.len()];
        if !finished {
            controls[self.driven] = controls_from(keys);
        }
        self.world.frame(dt, &controls);

        let time_step = dt.clamp(0.0, MAX_FRAME);
        self.update_race(time_step);
        self.fire_triggers();
        self.fade = (self.fade - time_step).max(0.0);
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

    /// La carrera con las poses de este paso: vueltas, llegadas y reposiciones.
    fn update_race(&mut self, time_step: f32) {
        let poses: Vec<CarPose> = self.world.vehicles()[..self.players]
            .iter()
            .map(car_pose)
            .collect();
        for event in self.race.update(time_step, &poses) {
            match event {
                RaceEvent::Lap { car, lap, time } => {
                    tracing::info!(auto = %self.names[car], vuelta = lap, tiempo = time, "vuelta");
                }
                RaceEvent::Finished { car, time } => {
                    tracing::info!(auto = %self.names[car], tiempo = time, "llegó");
                }
                RaceEvent::Respawn { car, reason } => {
                    tracing::info!(auto = %self.names[car], motivo = ?reason, "reposición");
                    self.respawn(car);
                }
            }
        }
    }

    /// `CAI_ResetCar`: el auto vuelve a su último POS node sano, derecho, apoyado sobre el
    /// piso y mirando hacia donde sigue la carrera. Sin camino, a su puesto de largada.
    fn respawn(&mut self, car: usize) {
        let (spot, yaw) = self
            .race
            .respawn_pose(car)
            .unwrap_or_else(|| start_slot(&self.grid, car));
        let ground = self
            .world
            .ground_below(spot + Vec3::Y * RESPAWN_PROBE, RESPAWN_DEPTH);
        let pos = ground.unwrap_or(spot) + Vec3::Y * RESPAWN_LIFT;
        let rot = Quat::from_rotation_y(yaw);
        self.world.place_vehicle(car, pos, rot);
        self.race.respawned(car);
        if car == self.driven {
            self.chase = ChaseCamera::new(pos, rot);
            self.fade = RESPAWN_FADE;
        }
    }

    /// `TriggerObjectThrower`: el centro de algún auto de la sala entra en la caja y aparece
    /// el objeto. Los autos sin conductor no disparan triggers.
    fn fire_triggers(&mut self) {
        let room = object_room(&self.animations);
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
            let now = self.race.time();
            if now < ready || !cars.iter().any(|&car| zone.contains(car)) {
                continue;
            }
            add_object(&mut self.world, &self.objects, spawn, room);
            *ready_at = zone.rearm.map(|secs| now + f64::from(secs));
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
        let race = self.race.has_laps().then(|| {
            let racer = self.race.racer(self.driven);
            RaceHud {
                lap: (racer.laps() + 1).min(self.race.laps()),
                laps: self.race.laps(),
                position: self.race.position(self.driven),
                racers: self.players,
                time: racer.finish_time().unwrap_or(self.race.time()),
                lap_time: racer.lap_time(self.race.time()),
                last_lap: racer.last_lap(),
                best_lap: racer.best_lap(),
                wrong_way: racer.wrong_way(),
            }
        });
        HudInfo {
            speed_mph: self.world.vehicle(self.driven).velocity(self.world.bodies()).length() * MPS_TO_MPH,
            car_name: self.cars[self.driven].name.clone(),
            cars: self.players,
            free_camera: self.free_mode,
            sound: self.audio.enabled(),
            race,
            fade: self.fade / RESPAWN_FADE,
            results: self.results(),
        }
    }

    /// Cuando terminaron todos los jugadores: los que llegaron, en orden, con su tiempo.
    fn results(&self) -> Option<Vec<(String, f64)>> {
        let humans = self.humans.max(1).min(self.players);
        let done = (0..humans).all(|car| self.race.racer(car).finish_time().is_some());
        done.then(|| {
            self.race
                .results()
                .into_iter()
                .map(|(car, time)| (self.names[car].clone(), time))
                .collect()
        })
    }
}

/// Dónde está el auto y hacia dónde mira, al final del último paso.
fn car_pose(vehicle: &Vehicle) -> CarPose {
    let (pos, rot) = vehicle.pose(1.0);
    CarPose {
        pos,
        forward: rot * Vec3::Z,
    }
}

/// Cuántos objetos de la física se pueden dibujar: los huesos animados ocupan el resto.
fn object_room(animations: &TrackAnimations) -> usize {
    MAX_OBJECTS.saturating_sub(animations.bone_count())
}

/// Pone una aparición en el mundo, si hay `room` para dibujarla: con su camino, o suelta
/// con su velocidad.
fn add_object(world: &mut PhysicsWorld, objects: &TrackObjects, spawn: &ObjectSpawn, room: usize) {
    if world.objects().len() >= room {
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
/// menú, con las vueltas de `rules`. El primer auto es del jugador del teclado; sin autos
/// van `car` y `extra_cars` de la config.
pub fn cli_race(config: &ClientConfig, rules: &GameplayRules) -> Option<RaceSetup> {
    let args: Vec<String> = std::env::args()
        .skip(1)
        .filter(|arg| !arg.starts_with('-'))
        .collect();
    let defaults: Vec<String> = std::iter::once(config.car.clone())
        .chain(config.extra_cars.iter().cloned())
        .collect();
    race_from_args(&args, &defaults, &config.player_name, rules.laps)
}

fn race_from_args(
    args: &[String],
    default_cars: &[String],
    player: &str,
    laps: u32,
) -> Option<RaceSetup> {
    let (level, cars) = args.split_first()?;
    let cars = if cars.is_empty() { default_cars } else { cars };
    let entrants = cars
        .iter()
        .enumerate()
        .map(|(i, car)| Entrant {
            car: car.clone(),
            player: (i == 0).then(|| player.to_string()),
        })
        .collect();
    Some(RaceSetup {
        level: level.clone(),
        entrants,
        laps,
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

    /// En la arena, reaparecer lleva el auto al POS node de la meta: apoyado, derecho y
    /// mirando hacia +Z, por donde sigue el circuito.
    #[test]
    fn respawning_goes_to_the_last_good_node() {
        let config = ClientConfig::load().unwrap();
        let rules = GameplayRules::default();
        let setup = RaceSetup {
            level: "revvy_arena".into(),
            entrants: vec![Entrant {
                car: "phim_calcure".into(),
                player: Some("Jugador 1".into()),
            }],
            laps: 2,
        };
        let mut drive = DriveView::load(&config, &setup, &rules).unwrap();
        let mut input = Input::new();
        for _ in 0..30 {
            drive.step(1.0 / 60.0, &mut input);
        }
        drive.respawn(0);
        for _ in 0..60 {
            drive.step(1.0 / 60.0, &mut input);
        }
        let (pos, rot) = drive.world.vehicle(0).pose(1.0);
        assert!(
            pos.distance(Vec3::new(0.0, pos.y, -38.0)) < 0.2,
            "reapareció en {pos}"
        );
        assert!(pos.y > 0.0 && pos.y < 0.3, "apoyado en el piso: {pos}");
        assert!(
            (rot * Vec3::Z).dot(Vec3::Z) > 0.99,
            "mirando hacia el circuito"
        );
        let hud = drive.hud();
        let race = hud.race.expect("la arena tiene vueltas");
        assert_eq!((race.lap, race.laps, race.position), (1, 2, 1));
        assert!(!race.wrong_way);
        assert!(hud.results.is_none());
    }

    #[test]
    fn cli_args_skip_the_menu() {
        let defaults = ["phim_calcure".to_string(), "revvy_buggy".to_string()];
        assert_eq!(race_from_args(&[], &defaults, "Jugador 1", 3), None);
        let race = race_from_args(&["nhood1".into()], &defaults, "Jugador 1", 3).unwrap();
        assert_eq!(race.level, "nhood1");
        assert_eq!(race.laps, 3);
        let cars: Vec<&str> = race
            .entrants
            .iter()
            .map(|entrant| entrant.car.as_str())
            .collect();
        assert_eq!(cars, defaults);
        // El primer auto es del jugador del teclado; los extra no tienen jugador.
        assert_eq!(race.entrants[0].player.as_deref(), Some("Jugador 1"));
        assert_eq!(race.entrants[1].player, None);
        let race = race_from_args(
            &["nhood1".into(), "revvy_buggy".into()],
            &defaults,
            "Jugador 1",
            3,
        )
        .unwrap();
        assert_eq!(race.entrants.len(), 1);
        assert_eq!(race.entrants[0].car, "revvy_buggy");
    }

    #[test]
    fn extra_cars_line_up_behind_the_last_slot() {
        let grid = [StartSlot { pos: Vec3::ZERO, yaw: 0.0 }];
        let (pos, _) = start_slot(&grid, 2);
        assert!((pos - Vec3::new(0.0, 0.0, -3.0)).length() < 1e-5);
    }
}
