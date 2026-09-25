//! Capa de traducción de los objetos de Re-Volt: los del `.fob` que son cuerpos físicos, con
//! la física de su `Init*` en `obj_init.cpp`, el modelo de `models/` y el golpe de
//! `BangNoiseTable` (`ai.cpp`) → objetos de Revvy.
//!
//! - Los modelos usan las páginas de textura del nivel (`LoadOneLevelModel` con
//!   `LOADMODEL_OFFSET_TPAGE` 0): cada nivel trae en sus páginas las de sus objetos. La
//!   pelota de playa es la excepción: usa `gfx/fxpage1.bmp`.
//! - `MODELRGBPER` del `.inf` escala el color de vértice de los modelos.
//! - Un `OBJECT_THROWER` tira su objeto (`flags[1]`) a `flags[2] × 50` unidades por segundo
//!   por su eje `Look` la primera vez que un auto entra en el trigger de tipo 6 con su id
//!   (`flags[0]`).
//! - Los modelos, cascos y sonidos se buscan en la raíz de contenido: la carpeta que tiene
//!   `levels/`, `models/`, `wavs/` y `gfx/`, como la de Re-Volt.
//!
//! - El chango (`InitTrolley`) es un auto de Re-Volt, `cars/trolley`, sin conductor. A
//!   diferencia de Re-Volt, también aparece en multijugador.
//! - Las puertas corredizas de market2 (`InitSlider`) van y vuelven solas por un camino
//!   fijo (`AI_SliderHandler`) y empujan lo que encuentran.
//!
//! La lata, la colchoneta, las luces, las estrellas y los regadores como cuerpos quedan
//! afuera, porque no están sus modelos: se avisan y se saltean.

use std::collections::BTreeMap;
use std::path::Path;

use glam::Vec3;

use crate::axes;
use crate::fob::FobObject;
use crate::hul;
use crate::inf::TrackInf;
use crate::mesh::VisualMesh;
use crate::ncp;
use crate::objects::{
    CarSpawn, ImpactSound, MotionSounds, ObjectKind, ObjectMotion, ObjectShape, ObjectSpawn,
    SpawnTrigger, SpawnWhen, TrackObjects,
};
use crate::prm::Prm;
use crate::tri::{Trigger, TRIGGER_OBJECT_THROWER};
use crate::{bmp, find_file, load_car, CarDef};

/// `OBJECT_TYPE_TROLLEY`.
const OBJECT_TROLLEY: i32 = 7;
/// `OBJECT_TYPE_OBJECT_THROWER`.
const OBJECT_THROWER: i32 = 42;
/// `OBJECT_TYPE_SLIDER`.
const OBJECT_SLIDER: i32 = 56;
/// `TrolleyAIHandler` endereza el chango cuando la Y de su eje vertical baja de esto.
const TROLLEY_MIN_UP: f32 = 0.5;
/// `AI_SliderHandler`: cada hoja se corre 400 unidades y vuelve, en 3 s.
const SLIDER_TRAVEL: f32 = 400.0;
const SLIDER_PERIOD: f32 = 3.0;
/// `TriggerObjectThrower` multiplica `Speed` por esto.
const THROW_SPEED_SCALE: f32 = 50.0;
/// `FRICTION_TIME_SCALE` de PC: `Resistance` frena esta fracción por cada 1/120 s.
const FRICTION_TIME_SCALE: f32 = 120.0;

/// Cuándo arranca dormido (`NoMoveTime = 2 × MOVE_MAX_NOMOVETIME`).
#[derive(Clone, Copy)]
enum Asleep {
    Never,
    Always,
    /// Solo si `flags[0]` del `.fob` no es cero (las botellas).
    WithFlag,
}

/// Un objeto físico de Re-Volt con los números de su `Init*`.
struct RevoltKind {
    object_type: i32,
    name: &'static str,
    /// En `models/`, sin extensión.
    model: &'static str,
    /// Radio de la esfera en unidades. `None`: los cascos del `.hul` del modelo.
    sphere: Option<f32>,
    mass: f32,
    /// `BodyInertia`, diagonal, en masa × unidades².
    inertia: [f32; 3],
    hardness: f32,
    resistance: f32,
    ang_resistance: f32,
    kinetic_friction: f32,
    asleep: Asleep,
    /// `BangNoiseTable`: sonido en `wavs/`, `VolOffset` y `MinMag` (unidades/s).
    bang: Option<(&'static str, f32, f32)>,
    /// Página global en lugar de las del nivel (`TPAGE_FX1`).
    fx_page: Option<&'static str>,
}

const KINDS: [RevoltKind; 8] = [
    RevoltKind {
        object_type: 1,
        name: "beachball",
        model: "beachball",
        sphere: Some(100.0),
        mass: 0.1,
        inertia: [100.0; 3],
        hardness: 0.6,
        resistance: 0.005,
        ang_resistance: 0.005,
        kinetic_friction: 0.5,
        asleep: Asleep::Never,
        bang: Some(("beachball.wav", 0.0, 300.0)),
        fx_page: Some("fxpage1.bmp"),
    },
    RevoltKind {
        object_type: 15,
        name: "football",
        model: "football",
        sphere: Some(30.0),
        mass: 0.2,
        inertia: [100.0; 3],
        hardness: 0.6,
        resistance: 0.001,
        ang_resistance: 0.005,
        kinetic_friction: 0.8,
        asleep: Asleep::Never,
        bang: None,
        fx_page: None,
    },
    RevoltKind {
        object_type: 43,
        name: "basketball",
        model: "basketball",
        sphere: Some(48.0),
        mass: 0.3,
        inertia: [270.0; 3],
        hardness: 0.8,
        resistance: 0.001,
        ang_resistance: 0.001,
        kinetic_friction: 2.0,
        asleep: Asleep::Never,
        bang: Some(("hood/basketball.wav", 0.0, 300.0)),
        fx_page: None,
    },
    RevoltKind {
        object_type: 57,
        name: "bottle",
        model: "bottle",
        sphere: None,
        mass: 1.2,
        inertia: [1500.0, 500.0, 1500.0],
        hardness: 0.0,
        resistance: 0.008,
        ang_resistance: 0.002,
        kinetic_friction: 0.05,
        asleep: Asleep::WithFlag,
        bang: Some(("bottle.wav", 200.0, 100.0)),
        fx_page: None,
    },
    RevoltKind {
        object_type: 58,
        name: "bucket",
        model: "bucket",
        sphere: None,
        mass: 1.5,
        inertia: [2900.0, 920.0, 2900.0],
        hardness: 0.0,
        resistance: 0.008,
        ang_resistance: 0.001,
        kinetic_friction: 0.05,
        asleep: Asleep::Always,
        bang: None,
        fx_page: None,
    },
    RevoltKind {
        object_type: 59,
        name: "cone",
        model: "trafficcone",
        sphere: None,
        mass: 1.6,
        inertia: [2040.0, 2350.0, 2040.0],
        hardness: 0.0,
        resistance: 0.008,
        ang_resistance: 0.001,
        kinetic_friction: 0.4,
        asleep: Asleep::Never,
        bang: Some(("hood/roadcone.wav", -150.0, 300.0)),
        fx_page: None,
    },
    RevoltKind {
        object_type: 66,
        name: "packet",
        model: "packet",
        sphere: None,
        mass: 1.6,
        inertia: [3270.0, 1300.0, 3270.0],
        hardness: 0.0,
        resistance: 0.008,
        ang_resistance: 0.001,
        kinetic_friction: 0.4,
        asleep: Asleep::Always,
        bang: Some(("market/carton.wav", 200.0, 100.0)),
        fx_page: None,
    },
    RevoltKind {
        object_type: 67,
        name: "abc",
        model: "abcblock",
        sphere: None,
        mass: 0.8,
        inertia: [600.0; 3],
        hardness: 0.0,
        resistance: 0.008,
        ang_resistance: 0.001,
        kinetic_friction: 0.4,
        asleep: Asleep::Always,
        bang: Some(("toy/toybrick.wav", 200.0, 100.0)),
        fx_page: None,
    },
];

/// Los objetos físicos del `.fob` de `level` (la carpeta del nivel) y sus lanzadores.
pub fn translate(
    level: &Path,
    info: &TrackInf,
    objects: &[FobObject],
    triggers: &[Trigger],
) -> TrackObjects {
    let level_id = level
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    let Some(root) = level.parent().and_then(Path::parent) else {
        return TrackObjects::default();
    };
    let mut out = TrackObjects::default();
    // (tipo de Re-Volt, arranca dormido) → índice en `out.kinds`, o `None` si no cargó.
    let mut loaded: BTreeMap<(i32, bool), Option<usize>> = BTreeMap::new();
    let mut skipped: BTreeMap<i32, usize> = BTreeMap::new();
    let mut trolley: Option<Option<usize>> = None;
    let mut slider: Option<Option<usize>> = None;
    for object in objects {
        if object.id == OBJECT_TROLLEY {
            let slot = *trolley.get_or_insert_with(|| match load_trolley(root, info) {
                Ok(car) => {
                    out.cars.push(car);
                    Some(out.cars.len() - 1)
                }
                Err(err) => {
                    tracing::warn!(%err, "chango sin auto: no aparece");
                    None
                }
            });
            if let Some(car) = slot {
                let look = axes::direction(object.look);
                out.car_spawns.push(CarSpawn {
                    car,
                    pos: axes::position(object.pos),
                    yaw: look.x.atan2(look.z),
                    self_righting: Some(TROLLEY_MIN_UP),
                });
            }
            continue;
        }
        if object.id == OBJECT_SLIDER {
            let slot = *slider.get_or_insert_with(|| match load_slider(root, info) {
                Ok(kind) => {
                    out.kinds.push(kind);
                    Some(out.kinds.len() - 1)
                }
                Err(err) => {
                    tracing::warn!(%err, "puerta corrediza sin modelo: se saltea");
                    None
                }
            });
            if let Some(kind) = slot {
                // Por el eje `R` del objeto; la hoja con id 0 (`flags[0]`), para el otro lado.
                let right = Vec3::from(object.up).cross(Vec3::from(object.look));
                let right = axes::direction(right.to_array()).normalize_or_zero();
                let sign = if object.flags[0] == 0 { -1.0 } else { 1.0 };
                out.spawns.push(ObjectSpawn {
                    kind,
                    pos: axes::position(object.pos),
                    rot: rotation(object),
                    velocity: Vec3::ZERO,
                    when: SpawnWhen::Start,
                    motion: Some(ObjectMotion::Slide {
                        offset: right * sign * SLIDER_TRAVEL * axes::REVOLT_TO_METERS,
                        period: SLIDER_PERIOD,
                    }),
                });
            }
            continue;
        }
        let (object_type, velocity, when) = if object.id == OBJECT_THROWER {
            let [id, thrown, speed, _reuse] = object.flags;
            let Some(trigger) = triggers
                .iter()
                .find(|trigger| trigger.kind == TRIGGER_OBJECT_THROWER && trigger.flag == id)
            else {
                tracing::warn!(id, "lanzador sin su trigger: no tira nada");
                continue;
            };
            let look = axes::direction(object.look).normalize_or_zero();
            let velocity = look * speed as f32 * THROW_SPEED_SCALE * axes::REVOLT_TO_METERS;
            let when = SpawnWhen::Trigger(SpawnTrigger {
                center: trigger.center,
                rotation: trigger.rotation,
                half_extents: trigger.half_extents,
                rearm: None,
            });
            (thrown, velocity, when)
        } else {
            (object.id, Vec3::ZERO, SpawnWhen::Start)
        };
        let Some(spec) = KINDS.iter().find(|kind| kind.object_type == object_type) else {
            *skipped.entry(object_type).or_default() += 1;
            continue;
        };
        let asleep = match spec.asleep {
            Asleep::Never => false,
            Asleep::Always => true,
            Asleep::WithFlag => object.flags[0] != 0,
        };
        // Con `WithFlag` el mismo objeto puede arrancar dormido o no: cada caso es un tipo.
        let slot = *loaded.entry((object_type, asleep)).or_insert_with(|| {
            match load_kind(root, &level_id, info, spec, asleep) {
                Ok(kind) => {
                    out.kinds.push(kind);
                    Some(out.kinds.len() - 1)
                }
                Err(err) => {
                    tracing::warn!(objeto = spec.name, %err, "objeto sin modelo: se saltea");
                    None
                }
            }
        });
        let Some(kind) = slot else { continue };
        out.spawns.push(ObjectSpawn {
            kind,
            pos: axes::position(object.pos),
            rot: rotation(object),
            velocity,
            when,
            motion: None,
        });
    }
    let unsupported: Vec<String> = skipped
        .iter()
        .filter(|(kind, _)| !IGNORED.contains(kind))
        .map(|(kind, count)| format!("{kind}×{count}"))
        .collect();
    if !unsupported.is_empty() {
        tracing::info!(tipos = %unsupported.join(" "), "objetos de Re-Volt todavía sin traducir");
    }
    out
}

/// Los que ya se usan por otro lado: rayitos (`layout.pickups`), regadores y sonidos 3D
/// (`TrackSounds`), y el `SKYBOX`, que prende el cielo de la carpeta del nivel: ese lo carga
/// `load_sky`.
const IGNORED: [i32; 4] = [30, 40, 49, 55];

/// `R = U × L`: las filas de la matriz del objeto, pasadas a Revvy.
fn rotation(object: &FobObject) -> glam::Quat {
    let up = Vec3::from(object.up);
    let look = Vec3::from(object.look);
    let right = up.cross(look);
    axes::rotation([right.to_array(), object.up, object.look])
}

fn load_kind(
    root: &Path,
    level_id: &str,
    info: &TrackInf,
    spec: &RevoltKind,
    asleep: bool,
) -> Result<ObjectKind, String> {
    let s = axes::REVOLT_TO_METERS;
    let models = root.join("models");
    // `InitCone` y `InitPacket` eligen otro modelo en nhood2 y market2.
    let model = match (spec.model, level_id) {
        ("trafficcone", "nhood2") => "n2trafficcone",
        ("packet", "market2") => "packet1",
        (model, _) => model,
    };
    let meshes = load_model(&models, model, info)?;
    let textures = match spec.fx_page {
        Some(page) => match find_file(&root.join("gfx"), page).map(|path| bmp::load(&path)) {
            Some(Ok(image)) => vec![(0, image)],
            _ => {
                tracing::warn!(
                    pagina = page,
                    objeto = spec.name,
                    "falta la página del objeto"
                );
                Vec::new()
            }
        },
        None => Vec::new(),
    };

    let shape = match spec.sphere {
        Some(radius) => ObjectShape::Sphere { radius: radius * s },
        None => {
            let hull_path = find_file(&models, &format!("{model}.hul"))
                .ok_or_else(|| format!("falta models/{model}.hul"))?;
            let native = hul::load_native(&hull_path).map_err(|err| err.to_string())?;
            let hulls: Vec<Vec<Vec3>> = native
                .hulls
                .iter()
                .map(|points| points.iter().map(|&p| axes::position(p)).collect())
                .collect();
            if hulls.is_empty() {
                return Err(format!("models/{model}.hul sin cascos"));
            }
            ObjectShape::Hulls(hulls)
        }
    };

    let impact_sound = spec.bang.and_then(|(file, volume_offset, min_mag)| {
        let path = root.join("wavs").join(file);
        if !path.is_file() {
            tracing::warn!(archivo = %path.display(), objeto = spec.name, "falta el sonido del golpe");
            return None;
        }
        Some(ImpactSound {
            file: path,
            min_speed: min_mag * s,
            volume_offset: volume_offset / 10.0,
        })
    });

    Ok(ObjectKind {
        name: spec.name.to_string(),
        meshes,
        textures,
        shape,
        mass: spec.mass,
        inertia: Some(spec.inertia.map(|i| i * s * s)),
        friction: spec.kinetic_friction,
        restitution: spec.hardness,
        linear_damping: FRICTION_TIME_SCALE * spec.resistance,
        angular_damping: FRICTION_TIME_SCALE * spec.ang_resistance,
        starts_asleep: asleep,
        impact_sound,
        motion_sounds: MotionSounds::default(),
    })
}

/// Un modelo de `models/`, con el color de vértice escalado por `MODELRGBPER`.
fn load_model(models: &Path, model: &str, info: &TrackInf) -> Result<Vec<VisualMesh>, String> {
    let model_path = find_file(models, &format!("{model}.m"))
        .ok_or_else(|| format!("falta models/{model}.m"))?;
    let prm = Prm::parse(&model_path).map_err(|err| err.to_string())?;
    let mut meshes = prm.to_meshes(model).map_err(|err| err.to_string())?;
    scale_rgb(&mut meshes, info.model_rgb_per);
    Ok(meshes)
}

/// `MODELRGBPER` del `.inf`: `LoadModel` escala el color de vértice de los modelos del
/// nivel, y `SetupCar` el de los autos.
fn scale_rgb(meshes: &mut [VisualMesh], percent: u32) {
    if percent == 100 {
        return;
    }
    for color in meshes.iter_mut().flat_map(|mesh| mesh.colors.iter_mut()) {
        for channel in &mut color[..3] {
            *channel = (u32::from(*channel) * percent / 100).min(255) as u8;
        }
    }
}

/// `InitTrolley`: el auto `cars/trolley` de la raíz de contenido.
fn load_trolley(root: &Path, info: &TrackInf) -> Result<CarDef, String> {
    let dir = find_file(&root.join("cars"), "trolley").ok_or("falta cars/trolley")?;
    let mut car = load_car(&dir).map_err(|err| err.to_string())?;
    scale_rgb(&mut car.body, info.model_rgb_per);
    for wheel in &mut car.wheels {
        scale_rgb(wheel, info.model_rgb_per);
    }
    Ok(car)
}

/// `InitSlider`: `models/slider.m`, que choca con los polígonos de `slider.ncp`. No tiene
/// masa: lo mueve su camino. El cuerpo es el de `InitBodyDefault`, sin fricción ni rebote.
fn load_slider(root: &Path, info: &TrackInf) -> Result<ObjectKind, String> {
    let models = root.join("models");
    let meshes = load_model(&models, "slider", info)?;
    let ncp_path = find_file(&models, "slider.ncp").ok_or("falta models/slider.ncp")?;
    let hull: Vec<Vec3> = ncp::parse(&ncp_path)
        .map_err(|err| err.to_string())?
        .iter()
        .flat_map(|tri| tri.positions)
        .collect();
    if hull.len() < 4 {
        return Err("models/slider.ncp sin polígonos".into());
    }
    let sound = |file: &str| {
        let path = root.join("wavs").join(file);
        if !path.is_file() {
            tracing::warn!(archivo = %path.display(), "falta un sonido de la puerta corrediza");
            return None;
        }
        Some(path)
    };
    Ok(ObjectKind {
        name: "slider".to_string(),
        meshes,
        textures: Vec::new(),
        shape: ObjectShape::Hulls(vec![hull]),
        mass: 0.0,
        inertia: None,
        friction: 0.0,
        restitution: 0.0,
        linear_damping: 0.0,
        angular_damping: 0.0,
        starts_asleep: false,
        impact_sound: None,
        // `SFX_MARKET_DOOR_OPEN` y `SFX_MARKET_DOOR_CLOSE`.
        motion_sounds: MotionSounds {
            start: sound("market/sdrsopen.wav"),
            turn: sound("market/sdrsclos.wav"),
        },
    })
}
