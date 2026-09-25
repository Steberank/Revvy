//! Objetos de una pista propia. Todo va en la carpeta de la pista: cada objeto en
//! `objects/<nombre>/` con su `object.toml`, su `model.glb` y sus sonidos, y las apariciones
//! en `layout.ron` (`objects`). Nunca se completan con datos de Re-Volt.
//!
//! `object.toml`, en SI y relativo al origen del modelo, que es el centro de masa:
//!
//! ```toml
//! name = "Cono"                 # opcional: si falta, el nombre de la carpeta
//! model = "model.glb"           # opcional
//! mass = 1.6                    # kg
//! friction = 0.4                # contra la pista se multiplica por la rugosidad del piso
//! restitution = 0.0             # rebote, de 0 a 1
//! linear_damping = 0.96         # 1/s
//! angular_damping = 0.12        # 1/s
//! asleep = false                # quieto donde está hasta que algo lo toque
//! shape = { type = "hull" }     # o { type = "sphere", radius = 0.24 }
//! # inertia = [0.05, 0.06, 0.05]  # kg·m² en los ejes del objeto; si falta, sale de la forma
//!
//! [sound]                       # opcional, todo en la carpeta del objeto
//! impact = "golpe.wav"          # cuando choca fuerte
//! min_speed = 1.5               # m/s de cambio de velocidad para que suene el golpe
//! volume_offset = 0.0           # suma al volumen del golpe, que va de 0 a 127
//! start = "abre.wav"            # con camino: cuando sale del punto de partida
//! turn = "cierra.wav"           # con camino: cuando llega a la otra punta y vuelve
//! ```
//!
//! Con `hull`, cada nodo `Collision` del modelo es un casco convexo; sin esos nodos, el
//! casco envuelve la malla visible.
//!
//! Una carpeta con `car.toml` en lugar de `object.toml` es un auto sin conductor (§6.6 de la
//! arquitectura): nadie lo maneja, Tab no lo elige y se endereza solo si se vuelca.
//!
//! En `layout.ron`, `spawn` es `Start` (por defecto) o un `Trigger` que tira el objeto
//! cuando un auto entra en la caja, una vez por carrera o cada `rearm` segundos. `motion`
//! le da un camino: `Slide` va hasta `offset` (m, en los ejes del objeto) y vuelve en
//! `period` segundos, sin que nada lo frene, y empuja lo que encuentra. Los autos sin
//! conductor solo usan `pos` y `yaw`.
//!
//! ```ron
//! objects: [
//!     (object: "cono", pos: (x: 3.0, y: 0.35, z: -30.0)),
//!     (object: "pelota", pos: (x: 8.0, y: 1.5, z: -20.0), velocity: (x: -9.0, y: 3.0, z: 0.0),
//!      spawn: Trigger(center: (x: 0.0, y: 1.0, z: -34.0), half_extents: (x: 6.0, y: 3.0, z: 1.0), rearm: Some(6.0))),
//!     (object: "puerta", pos: (x: -1.5, y: 1.0, z: -6.0), motion: Slide(offset: (x: -2.5, y: 0.0, z: 0.0), period: 4.0)),
//!     (object: "carrito", pos: (x: 4.0, y: 0.3, z: -12.0), yaw: 1.57),
//! ],
//! ```

use std::collections::BTreeMap;
use std::path::Path;

use glam::{Quat, Vec3};
use serde::Deserialize;

use crate::glb::{self, GlbNode};
use crate::gltf_track::V3;
use crate::objects::{
    CarSpawn, ImpactSound, MotionSounds, ObjectKind, ObjectMotion, ObjectShape, ObjectSpawn,
    SpawnTrigger, SpawnWhen, TrackObjects,
};
use crate::{find_file, revvy_car, FormatError};

/// Un auto sin conductor que se vuelca se endereza cuando la Y de su eje vertical baja de
/// esto, como el chango de Re-Volt.
const CAR_MIN_UP: f32 = 0.5;

/// Una aparición de `layout.ron`.
#[derive(Deserialize)]
pub(crate) struct Placement {
    object: String,
    pos: V3,
    #[serde(default)]
    yaw: f32,
    /// m/s. Si falta, quieto.
    #[serde(default)]
    velocity: V3,
    #[serde(default)]
    spawn: SpawnFile,
    #[serde(default)]
    motion: MotionFile,
}

#[derive(Deserialize, Default)]
enum SpawnFile {
    #[default]
    Start,
    Trigger {
        center: V3,
        half_extents: V3,
        #[serde(default)]
        yaw: f32,
        #[serde(default)]
        rearm: Option<f32>,
    },
}

#[derive(Deserialize, Default)]
enum MotionFile {
    /// Lo mueve la física.
    #[default]
    Physics,
    /// Hasta `offset`, en los ejes del objeto, y de vuelta en `period` segundos.
    Slide { offset: V3, period: f32 },
}

#[derive(Deserialize)]
struct ObjectToml {
    #[serde(default)]
    name: Option<String>,
    #[serde(default = "default_model")]
    model: String,
    mass: f32,
    #[serde(default = "default_friction")]
    friction: f32,
    #[serde(default)]
    restitution: f32,
    #[serde(default = "default_damping")]
    linear_damping: f32,
    #[serde(default = "default_damping")]
    angular_damping: f32,
    #[serde(default)]
    asleep: bool,
    #[serde(default)]
    shape: ShapeToml,
    #[serde(default)]
    inertia: Option<[f32; 3]>,
    #[serde(default)]
    sound: SoundToml,
}

#[derive(Deserialize, Default)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ShapeToml {
    #[default]
    Hull,
    Sphere {
        radius: f32,
    },
}

#[derive(Deserialize, Default)]
struct SoundToml {
    #[serde(default)]
    impact: Option<String>,
    #[serde(default = "default_min_speed")]
    min_speed: f32,
    #[serde(default)]
    volume_offset: f32,
    #[serde(default)]
    start: Option<String>,
    #[serde(default)]
    turn: Option<String>,
}

fn default_model() -> String {
    "model.glb".to_string()
}

fn default_friction() -> f32 {
    0.5
}

fn default_damping() -> f32 {
    0.1
}

fn default_min_speed() -> f32 {
    1.5
}

/// Lo que hay en una carpeta de `objects/`, ya cargado.
#[derive(Clone, Copy)]
enum Loaded {
    /// Índice en `TrackObjects::kinds`.
    Object(usize),
    /// Índice en `TrackObjects::cars`.
    Car(usize),
    Broken,
}

/// Los objetos que usan las apariciones de `layout.ron`, con sus tipos de `objects/`.
pub(crate) fn load(dir: &Path, placements: &[Placement]) -> TrackObjects {
    let mut out = TrackObjects::default();
    let mut loaded: BTreeMap<&str, Loaded> = BTreeMap::new();
    for placement in placements {
        let name = placement.object.as_str();
        let slot = *loaded
            .entry(name)
            .or_insert_with(|| load_folder(&dir.join("objects").join(name), name, &mut out));
        let rot = Quat::from_rotation_y(placement.yaw);
        match slot {
            Loaded::Broken => {}
            Loaded::Car(car) => {
                if !matches!(placement.spawn, SpawnFile::Start)
                    || !matches!(placement.motion, MotionFile::Physics)
                    || placement.velocity.vec() != Vec3::ZERO
                {
                    tracing::warn!(
                        objeto = name,
                        "un auto sin conductor aparece quieto al empezar: se ignoran spawn, motion y velocity"
                    );
                }
                out.car_spawns.push(CarSpawn {
                    car,
                    pos: placement.pos.vec(),
                    yaw: placement.yaw,
                    self_righting: Some(CAR_MIN_UP),
                });
            }
            Loaded::Object(kind) => {
                let when = match &placement.spawn {
                    SpawnFile::Start => SpawnWhen::Start,
                    SpawnFile::Trigger {
                        center,
                        half_extents,
                        yaw,
                        rearm,
                    } => SpawnWhen::Trigger(SpawnTrigger {
                        center: center.vec(),
                        rotation: Quat::from_rotation_y(*yaw),
                        half_extents: half_extents.vec().abs(),
                        rearm: *rearm,
                    }),
                };
                let motion = match &placement.motion {
                    MotionFile::Physics => None,
                    MotionFile::Slide { offset, period } if *period > 0.0 => {
                        Some(ObjectMotion::Slide {
                            offset: rot * offset.vec(),
                            period: *period,
                        })
                    }
                    MotionFile::Slide { .. } => {
                        tracing::warn!(objeto = name, "camino con period ≤ 0: lo mueve la física");
                        None
                    }
                };
                out.spawns.push(ObjectSpawn {
                    kind,
                    pos: placement.pos.vec(),
                    rot,
                    velocity: placement.velocity.vec(),
                    when,
                    motion,
                });
            }
        }
    }
    out
}

/// Un objeto (`object.toml`) o un auto sin conductor (`car.toml`).
fn load_folder(dir: &Path, name: &str, out: &mut TrackObjects) -> Loaded {
    let result = match find_file(dir, "car.toml") {
        Some(toml_path) => revvy_car::load(dir, &toml_path).map(|car| {
            out.cars.push(car);
            Loaded::Car(out.cars.len() - 1)
        }),
        None => load_kind(dir).map(|kind| {
            out.kinds.push(kind);
            Loaded::Object(out.kinds.len() - 1)
        }),
    };
    result.unwrap_or_else(|err| {
        tracing::warn!(objeto = name, %err, "objeto de la pista ilegible: se saltea");
        Loaded::Broken
    })
}

fn load_kind(dir: &Path) -> Result<ObjectKind, FormatError> {
    let toml_path = find_file(dir, "object.toml")
        .ok_or_else(|| FormatError::Missing(format!("{}/object.toml", dir.display())))?;
    let text =
        std::fs::read_to_string(&toml_path).map_err(|err| FormatError::io(&toml_path, err))?;
    let manifest: ObjectToml =
        toml::from_str(&text).map_err(|err| FormatError::parse(&toml_path, err.to_string()))?;
    if manifest.mass <= 0.0 {
        return Err(FormatError::parse(
            &toml_path,
            "mass tiene que ser positiva",
        ));
    }
    let folder = dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let model_path = find_file(dir, &manifest.model)
        .ok_or_else(|| FormatError::Missing(manifest.model.clone()))?;
    let file = glb::read(&model_path)?;
    let visual: Vec<&GlbNode> = file
        .nodes
        .iter()
        .filter(|node| !node.is_under("Collision"))
        .collect();
    let meshes = glb::visual_meshes(&visual, |node| node.world);
    let textures = file
        .images
        .iter()
        .enumerate()
        .map(|(i, image)| (i as i16, image.clone()))
        .collect();

    let shape = match manifest.shape {
        ShapeToml::Sphere { radius } => ObjectShape::Sphere { radius },
        ShapeToml::Hull => {
            let collision: Vec<&GlbNode> = file
                .nodes
                .iter()
                .filter(|node| node.is_under("Collision") && !node.primitives.is_empty())
                .collect();
            let hulls: Vec<Vec<Vec3>> = if collision.is_empty() {
                vec![visual
                    .iter()
                    .flat_map(|node| glb::node_points(node, node.world))
                    .collect()]
            } else {
                collision
                    .iter()
                    .map(|node| glb::node_points(node, node.world))
                    .collect()
            };
            let hulls: Vec<Vec<Vec3>> = hulls
                .into_iter()
                .filter(|points| points.len() >= 4)
                .collect();
            if hulls.is_empty() {
                return Err(FormatError::parse(&model_path, "sin malla para el casco"));
            }
            ObjectShape::Hulls(hulls)
        }
    };

    let sound = manifest.sound;
    let file_in = |name: Option<&String>| {
        let path = dir.join(name?);
        if !path.is_file() {
            tracing::warn!(archivo = %path.display(), objeto = %folder, "falta un sonido del objeto");
            return None;
        }
        Some(path)
    };
    let impact_sound = file_in(sound.impact.as_ref()).map(|file| ImpactSound {
        file,
        min_speed: sound.min_speed,
        volume_offset: sound.volume_offset,
    });
    let motion_sounds = MotionSounds {
        start: file_in(sound.start.as_ref()),
        turn: file_in(sound.turn.as_ref()),
    };

    Ok(ObjectKind {
        name: manifest.name.unwrap_or(folder),
        meshes,
        textures,
        shape,
        mass: manifest.mass,
        inertia: manifest.inertia,
        friction: manifest.friction,
        restitution: manifest.restitution,
        linear_damping: manifest.linear_damping,
        angular_damping: manifest.angular_damping,
        starts_asleep: manifest.asleep,
        impact_sound,
        motion_sounds,
    })
}
