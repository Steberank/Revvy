//! Autos propios de Revvy: `car.toml` + `body.glb` (+ `collision.glb`).
//!
//! Los parámetros del vehículo van en `[vehicle]`, en unidades SI y relativos al centro
//! de masa. Nunca se completan con datos de Re-Volt (`CARINFO.TXT`).
//!
//! - `body.glb`: los nodos `WheelFL`, `WheelFR`, `WheelBL` y `WheelBR` son las ruedas, con
//!   la malla centrada en el buje; el resto es el chasis, en el espacio del modelo.
//! - `collision.glb`: cada nodo que empieza con `Sphere` es una esfera que toca el mundo
//!   (la esfera que envuelve su malla); el resto son cascos convexos contra otros autos.
//!   Sin archivo se usa `[vehicle.chassis]`, o el casco del chasis visible.

use std::path::Path;

use glam::{Mat4, Vec3};
use serde::Deserialize;

use crate::glb::{self, GlbFile, GlbNode};
use crate::vehicle::{CarSound, EngineSound, VehicleParams};
use crate::{find_file, CarDef, FormatError};

const WHEEL_NODES: [&str; 4] = ["WheelFL", "WheelFR", "WheelBL", "WheelBR"];

#[derive(Deserialize)]
struct CarToml {
    name: String,
    #[serde(default = "default_body")]
    body: String,
    #[serde(default)]
    collision: Option<String>,
    #[serde(default)]
    sound: SoundToml,
    vehicle: VehicleParams,
}

#[derive(Deserialize, Default)]
struct SoundToml {
    #[serde(default)]
    engine: EngineSound,
    #[serde(default)]
    sample: Option<String>,
}

fn default_body() -> String {
    "body.glb".to_string()
}

pub fn load(dir: &Path, toml_path: &Path) -> Result<CarDef, FormatError> {
    let text = std::fs::read_to_string(toml_path).map_err(|err| FormatError::io(toml_path, err))?;
    let manifest: CarToml = toml::from_str(&text).map_err(|err| FormatError::parse(toml_path, err.to_string()))?;
    let mut vehicle = manifest.vehicle;

    let body_path = find_file(dir, &manifest.body).ok_or_else(|| FormatError::Missing(manifest.body.clone()))?;
    let body_file = glb::read(&body_path)?;
    let is_wheel = |node: &GlbNode| WHEEL_NODES.iter().any(|name| node.is_under(name));
    let body_nodes: Vec<&GlbNode> = body_file.nodes.iter().filter(|node| !is_wheel(node)).collect();
    let mut body = glb::visual_meshes(&body_nodes, |node| node.world);
    let mut wheels: [Vec<_>; 4] = Default::default();
    for (slot, name) in WHEEL_NODES.iter().enumerate() {
        let Some(hub) = body_file.nodes.iter().find(|node| node.name == *name) else {
            continue;
        };
        let hub_inverse = hub.world.inverse();
        let nodes: Vec<&GlbNode> = body_file.nodes.iter().filter(|node| node.is_under(name)).collect();
        wheels[slot] = glb::visual_meshes(&nodes, |node| hub_inverse * node.world);
    }
    let texture = car_texture(&body_file, &mut body, &mut wheels);

    if vehicle.chassis.spheres.is_empty() && vehicle.chassis.hulls.is_empty() {
        let offset = Vec3::from(vehicle.body_offset);
        match manifest.collision.as_deref().and_then(|name| find_file(dir, name)) {
            Some(path) => {
                let file = glb::read(&path)?;
                for node in &file.nodes {
                    let points = node_points(node, node.world);
                    if points.is_empty() {
                        continue;
                    }
                    if node.name.to_ascii_lowercase().starts_with("sphere") {
                        let (min, max) = bounds(&points);
                        let centre = (min + max) * 0.5 + offset;
                        let radius = ((max - min) * 0.5).max_element();
                        vehicle.chassis.spheres.push([centre.x, centre.y, centre.z, radius]);
                    } else {
                        vehicle.chassis.hulls.push(points.iter().map(|p| (*p + offset).to_array()).collect());
                    }
                }
            }
            None => {
                tracing::warn!(auto = %manifest.name, "auto sin collision.glb: el casco sale del chasis visible");
                let points: Vec<[f32; 3]> = body_nodes
                    .iter()
                    .flat_map(|node| node_points(node, node.world))
                    .map(|p| (p + offset).to_array())
                    .collect();
                vehicle.chassis.hulls.push(points);
            }
        }
    }

    Ok(CarDef {
        id: dir.file_name().unwrap_or_default().to_string_lossy().into_owned(),
        name: manifest.name,
        dir: dir.to_path_buf(),
        body,
        wheels,
        texture,
        vehicle,
        sound: CarSound {
            engine: manifest.sound.engine,
            sample: manifest.sound.sample,
        },
        revolt: None,
    })
}

/// El render del auto usa una sola textura: la primera imagen que usen sus mallas.
fn car_texture(
    file: &GlbFile,
    body: &mut [crate::VisualMesh],
    wheels: &mut [Vec<crate::VisualMesh>; 4],
) -> Option<image::RgbaImage> {
    let first = body
        .iter()
        .chain(wheels.iter().flatten())
        .find_map(|mesh| (mesh.texture_page >= 0).then_some(mesh.texture_page))?;
    let mut others = false;
    for mesh in body.iter_mut().chain(wheels.iter_mut().flatten()) {
        if mesh.texture_page >= 0 && mesh.texture_page != first {
            others = true;
            mesh.texture_page = first;
        }
    }
    if others {
        tracing::warn!("el auto usa más de una textura: se dibuja todo con la primera");
    }
    file.images.get(first as usize).cloned()
}

fn node_points(node: &GlbNode, matrix: Mat4) -> Vec<Vec3> {
    node.primitives
        .iter()
        .flat_map(|primitive| primitive.positions.iter().map(move |&p| matrix.transform_point3(p)))
        .collect()
}

fn bounds(points: &[Vec3]) -> (Vec3, Vec3) {
    points.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(min, max), &p| {
        (min.min(p), max.max(p))
    })
}
