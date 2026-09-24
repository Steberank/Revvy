//! Pistas `revvy-glb-v1`: `track.toml` + `visual.glb` (+ `layout.ron`).
//!
//! La escena tiene los nodos `Visual` (se dibuja), `Collision` (no se dibuja; el nombre
//! del material es la superficie) y opcionalmente `Props` (se dibuja). Sin `Collision`
//! la pista se rechaza: nunca se choca con lo que se ve.

use std::path::Path;

use glam::Vec3;
use serde::Deserialize;

use crate::glb::{self, GlbNode};
use crate::layout::{StartSlot, TrackLayout};
use crate::sounds::TrackSounds;
use crate::{find_file, Collision, FormatError, LoadedTrack, TrackAsset, TrackLoad, Visual};

#[derive(Deserialize)]
struct TrackManifest {
    #[serde(default)]
    name: Option<String>,
    /// Archivo de la escena; por defecto `visual.glb`.
    #[serde(default)]
    visual: Option<String>,
}

pub fn load(dir: &Path, options: TrackLoad) -> Result<LoadedTrack, FormatError> {
    let id = dir.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let toml_path = find_file(dir, "track.toml").ok_or_else(|| FormatError::Missing("track.toml".into()))?;
    let text = std::fs::read_to_string(&toml_path).map_err(|err| FormatError::io(&toml_path, err))?;
    let manifest: TrackManifest =
        toml::from_str(&text).map_err(|err| FormatError::parse(&toml_path, err.to_string()))?;
    let visual_name = manifest.visual.unwrap_or_else(|| "visual.glb".to_string());
    let glb_path = find_file(dir, &visual_name).ok_or_else(|| FormatError::Missing(visual_name.clone()))?;
    let file = glb::read(&glb_path)?;

    let collision_nodes: Vec<&GlbNode> = file.nodes.iter().filter(|node| node.is_under("Collision")).collect();
    if collision_nodes.iter().all(|node| node.primitives.is_empty()) {
        return Err(FormatError::Missing(format!("{visual_name}: nodo Collision con mallas")));
    }

    let visual = options.visual.then(|| {
        let nodes: Vec<&GlbNode> = file
            .nodes
            .iter()
            .filter(|node| node.is_under("Visual") || node.is_under("Props"))
            .collect();
        Visual {
            meshes: glb::visual_meshes(&nodes, |node| node.world),
            textures: file
                .images
                .iter()
                .enumerate()
                .map(|(i, image)| (i as i16, image.clone()))
                .collect(),
            color_key: false,
            sky: None,
        }
    });
    let collision = options.collision.then(|| Collision {
        triangles: glb::collision_triangles(&collision_nodes),
    });

    let mut layout = TrackLayout::default();
    layout.start_grid = start_grid(dir);
    tracing::info!(
        pista = manifest.name.as_deref().unwrap_or(&id),
        largada = layout.start_grid.len(),
        "pista revvy-glb-v1"
    );
    Ok(LoadedTrack {
        id,
        asset: TrackAsset {
            visual,
            collision,
            layout,
            legacy: None,
            sounds: TrackSounds::default(),
        },
    })
}

/// La grilla de `layout.ron`. El resto del layout lo lee el editor (fase 9).
#[derive(Deserialize)]
#[serde(rename = "TrackLayout")]
struct LayoutFile {
    #[serde(default)]
    start_grid: Vec<SlotFile>,
}

#[derive(Deserialize)]
struct SlotFile {
    pos: V3,
    yaw: f32,
}

#[derive(Deserialize)]
struct V3 {
    x: f32,
    y: f32,
    z: f32,
}

fn start_grid(dir: &Path) -> Vec<StartSlot> {
    let fallback = || {
        vec![StartSlot {
            pos: Vec3::new(0.0, 0.5, 0.0),
            yaw: 0.0,
        }]
    };
    let Some(path) = find_file(dir, "layout.ron") else {
        tracing::warn!("pista sin layout.ron: se larga en el origen");
        return fallback();
    };
    let parsed = std::fs::read_to_string(&path)
        .map_err(|err| err.to_string())
        .and_then(|text| ron::from_str::<LayoutFile>(&text).map_err(|err| err.to_string()));
    match parsed {
        Ok(layout) if !layout.start_grid.is_empty() => layout
            .start_grid
            .into_iter()
            .map(|slot| StartSlot {
                pos: Vec3::new(slot.pos.x, slot.pos.y, slot.pos.z),
                yaw: slot.yaw,
            })
            .collect(),
        Ok(_) => {
            tracing::warn!("layout.ron sin start_grid: se larga en el origen");
            fallback()
        }
        Err(err) => {
            tracing::warn!(%err, "layout.ron ilegible: se larga en el origen");
            fallback()
        }
    }
}
