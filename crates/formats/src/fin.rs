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
        reader.skip(3 + 4 + 4 + 4)?;
        let position = reader.v3()?;
        let mut matrix = [[0.0; 3]; 3];
        for row in &mut matrix {
            *row = reader.v3()?;
        }
        instances.push(Instance {
            name,
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
        for tri in local {
            let positions = tri
                .positions
                .map(|point| place_instance(instance.matrix, instance.position, point));
            triangles.push(CollisionTri {
                positions,
                surface: tri.surface,
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
