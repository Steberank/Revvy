//! Instancias `.fin` (el nombre del archivo no es la grilla: la grilla sale del `.inf`).

use std::path::Path;

use glam::Vec3;

use crate::axes;
use crate::binutil::Reader;
use crate::mesh::VisualMesh;
use crate::ncp::{self, CollisionTri};
use crate::prm::Prm;
use crate::FormatError;

#[derive(Clone, Debug)]
pub struct Instance {
    pub name: String,
    /// `FILE_INSTANCE.Priority`. Con prioridad 0 la instancia es opcional (detalle).
    pub priority: u8,
    /// `INSTANCE_NO_OBJECT_COLLISION` (32), `INSTANCE_NO_CAMERA_COLLISION` (64), …
    pub flag: u8,
    pub position: [f32; 3],
    pub matrix: [[f32; 3]; 3],
}

pub fn parse(path: &Path) -> Result<Vec<Instance>, FormatError> {
    let file = std::fs::File::open(path).map_err(|err| FormatError::io(path, err))?;
    let mut reader = Reader::new(std::io::BufReader::new(file));
    let count = reader.i32()?;
    if count < 0 {
        return Err(FormatError::parse(path, "cantidad de instancias negativa"));
    }
    let mut instances = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let name_bytes = reader.bytes(9)?;
        let name = name_bytes
            .split(|b| *b == 0)
            .next()
            .unwrap_or(&[])
            .iter()
            .map(|b| *b as char)
            .collect::<String>();
        // r, g, b y EnvRGB.
        reader.skip(3 + 4)?;
        let priority = reader.u8()?;
        let flag = reader.u8()?;
        // pad[2] y LodBias.
        reader.skip(2 + 4)?;
        let position = reader.v3()?;
        let mut matrix = [[0.0; 3]; 3];
        for row in &mut matrix {
            *row = reader.v3()?;
        }
        instances.push(Instance {
            name,
            priority,
            flag,
            position,
            matrix,
        });
    }
    Ok(instances)
}

pub fn bake(level_dir: &Path, instances: &[Instance]) -> Result<Vec<VisualMesh>, FormatError> {
    let mut meshes = Vec::new();
    let mut cache: Vec<(String, Prm)> = Vec::new();
    let mut missing = std::collections::HashSet::new();
    for instance in instances {
        if missing.contains(&instance.name) {
            continue;
        }
        if !cache.iter().any(|(name, _)| name == &instance.name) {
            let Some(path) = find_prm(level_dir, &instance.name) else {
                tracing::warn!(name = %instance.name, "instancia sin .prm");
                missing.insert(instance.name.clone());
                continue;
            };
            cache.push((instance.name.clone(), Prm::parse(&path)?));
        }
        let prm = &cache
            .iter()
            .find(|(name, _)| name == &instance.name)
            .expect("el prm se acaba de cachear")
            .1;
        meshes.extend(crate::mesh::bake_meshes(
            &instance.name,
            &prm.vertices,
            &prm.polygons,
            Some((&instance.matrix, instance.position)),
        )?);
    }
    Ok(meshes)
}

/// Colisión de cada instancia (`.ncp` junto al `.prm`), en el mismo sitio que el modelo.
/// `INSTANCE_NO_OBJECT_COLLISION`: la instancia solo frena la cámara.
const INSTANCE_NO_OBJECT_COLLISION: u8 = 32;
/// `INSTANCE_NO_CAMERA_COLLISION`: la cámara la atraviesa.
const INSTANCE_NO_CAMERA_COLLISION: u8 = 64;

pub fn bake_collision(
    level_dir: &Path,
    instances: &[Instance],
) -> Result<Vec<CollisionTri>, FormatError> {
    let mut triangles = Vec::new();
    let mut cache: Vec<(String, Vec<CollisionTri>)> = Vec::new();
    let mut missing = std::collections::HashSet::new();
    for instance in instances {
        if missing.contains(&instance.name) {
            continue;
        }
        if !cache.iter().any(|(name, _)| name == &instance.name) {
            let Some(path) = find_sidecar(level_dir, &instance.name, "ncp") else {
                missing.insert(instance.name.clone());
                continue;
            };
            match ncp::parse(&path) {
                Ok(tris) if !tris.is_empty() => cache.push((instance.name.clone(), tris)),
                Ok(_) => {
                    missing.insert(instance.name.clone());
                    continue;
                }
                Err(err) => {
                    tracing::warn!(%err, name = %instance.name, "ncp de instancia ilegible");
                    missing.insert(instance.name.clone());
                    continue;
                }
            }
        }
        let local = &cache
            .iter()
            .find(|(name, _)| name == &instance.name)
            .expect("el ncp se acaba de cachear")
            .1;
        // `RotTransPlane` gira la normal sin espejarla: en una instancia espejada el orden
        // de los vértices se invierte para que el frente siga del mismo lado.
        let m = instance.matrix;
        let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
        for tri in local {
            let mut positions = tri
                .positions
                .map(|point| place_instance(instance.matrix, instance.position, point));
            if det < 0.0 {
                positions.swap(1, 2);
            }
            triangles.push(CollisionTri {
                positions,
                surface: tri.surface,
                camera_only: tri.camera_only || instance.flag & INSTANCE_NO_OBJECT_COLLISION != 0,
                object_only: tri.object_only || instance.flag & INSTANCE_NO_CAMERA_COLLISION != 0,
            });
        }
    }
    if !missing.is_empty() {
        let mut names: Vec<_> = missing.into_iter().collect();
        names.sort();
        tracing::info!(?names, "modelos de instancia sin .ncp");
    }
    tracing::info!(
        modelos = cache.len(),
        tris = triangles.len(),
        "colisión de instancias"
    );
    Ok(triangles)
}

/// `.ncp` de cada modelo de instancia, en espacio del modelo y sin convertir ejes.
/// `BuildInstanceCollPolys` los lleva al mundo con `RotTransPlane`; eso lo hace la física.
/// El índice de cada entrada es el de la instancia en `instances`.
pub fn native_collision(
    level_dir: &Path,
    instances: &[Instance],
) -> Result<Vec<Vec<ncp::NcpPoly>>, FormatError> {
    let mut cache: Vec<(String, std::rc::Rc<Vec<ncp::NcpPoly>>)> = Vec::new();
    let mut out = Vec::with_capacity(instances.len());
    for instance in instances {
        let key = instance.name.to_ascii_lowercase();
        let polys = match cache.iter().find(|(name, _)| *name == key) {
            Some((_, polys)) => polys.clone(),
            None => {
                let polys = match find_sidecar(level_dir, &instance.name, "ncp") {
                    Some(path) => match ncp::parse_native(&path) {
                        Ok(file) => file.polys,
                        Err(err) => {
                            tracing::warn!(%err, name = %instance.name, "ncp de instancia ilegible");
                            Vec::new()
                        }
                    },
                    None => Vec::new(),
                };
                let polys = std::rc::Rc::new(polys);
                cache.push((key, polys.clone()));
                polys
            }
        };
        out.push((*polys).clone());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collision_transform_matches_the_mesh() {
        let rows = [[0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]];
        let translation = [120.0, -15.0, 40.0];
        let revolt = [8.0, 4.0, -6.0];
        let local = [
            rows[0][0] * revolt[0] + rows[1][0] * revolt[1] + rows[2][0] * revolt[2],
            rows[0][1] * revolt[0] + rows[1][1] * revolt[1] + rows[2][1] * revolt[2],
            rows[0][2] * revolt[0] + rows[1][2] * revolt[1] + rows[2][2] * revolt[2],
        ];
        let visual = axes::position([
            local[0] + translation[0],
            local[1] + translation[1],
            local[2] + translation[2],
        ]);
        let collision = place_instance(rows, translation, axes::position(revolt));
        assert!(
            (visual - collision).length() < 1e-4,
            "visual={visual:?} colisión={collision:?}"
        );
    }
}

fn place_instance(rows: [[f32; 3]; 3], translation: [f32; 3], point: Vec3) -> Vec3 {
    // Misma cuenta que `mesh::mul_rows`: el `.ncp` se transforma igual que el `.prm`.
    let scale = axes::REVOLT_TO_METERS;
    let p = [-point.x / scale, -point.y / scale, point.z / scale];
    let local = [
        rows[0][0] * p[0] + rows[1][0] * p[1] + rows[2][0] * p[2],
        rows[0][1] * p[0] + rows[1][1] * p[1] + rows[2][1] * p[2],
        rows[0][2] * p[0] + rows[1][2] * p[1] + rows[2][2] * p[2],
    ];
    axes::position([
        local[0] + translation[0],
        local[1] + translation[1],
        local[2] + translation[2],
    ])
}

fn find_prm(dir: &Path, name: &str) -> Option<std::path::PathBuf> {
    find_sidecar(dir, name, "prm")
}

fn find_sidecar(dir: &Path, name: &str, extension: &str) -> Option<std::path::PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut prefix_match = None;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
            continue;
        };
        if !ext.eq_ignore_ascii_case(extension) {
            continue;
        }
        if stem.eq_ignore_ascii_case(name) {
            return Some(path);
        }
        // El `.fin` guarda solo 8 caracteres. `WHITEPOS` es `whitepost.prm`.
        if name.chars().count() >= 8 {
            let truncated: String = stem.chars().take(8).collect();
            if truncated.eq_ignore_ascii_case(name) {
                prefix_match = Some(path);
            }
        }
    }
    prefix_match
}
