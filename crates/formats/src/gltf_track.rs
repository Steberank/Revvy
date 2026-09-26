//! Pistas `revvy-glb-v1`: `track.toml` + `visual.glb` (+ `layout.ron`).
//!
//! La escena tiene los nodos `Visual` (se dibuja), `Collision` (no se dibuja; el nombre
//! del material es la superficie) y opcionalmente `Props` (se dibuja). Sin `Collision`
//! la pista se rechaza: nunca se choca con lo que se ve.

use std::path::Path;

use glam::{Quat, Vec3};
use serde::Deserialize;

use crate::animations::TrackAnimations;
use crate::glb::{self, GlbNode};
use crate::layout::{KillVolume, PosNode, StartSlot, TrackLayout, TrackZone};
use crate::objects::TrackObjects;
use crate::revvy_objects;
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
            background: None,
            animations: TrackAnimations::default(),
        }
    });
    let collision = options.collision.then(|| Collision {
        triangles: glb::collision_triangles(&collision_nodes),
        surfaces: Vec::new(),
    });

    let layout_file = read_layout(dir);
    let mut layout = TrackLayout::default();
    layout.start_grid = start_grid(layout_file.as_ref());
    if let Some(file) = &layout_file {
        race_layout(file, &mut layout);
    }
    let objects = match &layout_file {
        Some(file) if options.collision => revvy_objects::load(dir, &file.objects),
        _ => TrackObjects::default(),
    };
    tracing::info!(
        pista = manifest.name.as_deref().unwrap_or(&id),
        largada = layout.start_grid.len(),
        zonas = layout.zones.len(),
        pos_nodes = layout.pos_nodes.len(),
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
            objects,
        },
    })
}

/// Lo que se lee de `layout.ron`: la grilla, lo que usa la carrera (zonas, POS nodes y
/// kill volumes) y los objetos. El resto llega con el editor (fase 9).
#[derive(Deserialize)]
#[serde(rename = "TrackLayout")]
struct LayoutFile {
    #[serde(default)]
    start_grid: Vec<SlotFile>,
    #[serde(default)]
    start_node: u32,
    /// Largo de la vuelta (m). Si falta, se toma el `distance` más grande.
    #[serde(default)]
    total_distance: f32,
    #[serde(default)]
    zones: Vec<ZoneFile>,
    #[serde(default)]
    pos_nodes: Vec<PosNodeFile>,
    #[serde(default)]
    kill_volumes: Vec<BoxFile>,
    #[serde(default)]
    objects: Vec<revvy_objects::Placement>,
}

#[derive(Deserialize)]
struct SlotFile {
    pos: V3,
    yaw: f32,
}

#[derive(Deserialize)]
#[serde(rename = "TrackZone")]
struct ZoneFile {
    id: i32,
    center: V3,
    #[serde(default)]
    rotation: Q4,
    half_extents: V3,
}

#[derive(Deserialize)]
#[serde(rename = "PosNode")]
struct PosNodeFile {
    id: u32,
    position: V3,
    /// Lo que falta hasta la meta (m): 0 en el nodo de largada.
    distance: f32,
    #[serde(default)]
    prev: Vec<i32>,
    #[serde(default)]
    next: Vec<i32>,
}

#[derive(Deserialize)]
#[serde(rename = "KillVolume")]
struct BoxFile {
    center: V3,
    #[serde(default)]
    rotation: Q4,
    half_extents: V3,
}

/// Cuaternión `(x, y, z, w)`; si falta, sin giro.
#[derive(Deserialize)]
struct Q4 {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}

impl Default for Q4 {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        }
    }
}

impl Q4 {
    fn quat(&self) -> Quat {
        Quat::from_xyzw(self.x, self.y, self.z, self.w).normalize()
    }
}

/// Zonas, POS nodes y kill volumes de `layout.ron`, con la misma semántica que `.taz`,
/// `.pan` y los triggers de Re-Volt (§7.4). Los POS nodes van en el orden de su `id`.
fn race_layout(file: &LayoutFile, layout: &mut TrackLayout) {
    layout.zones = file
        .zones
        .iter()
        .map(|zone| TrackZone {
            id: zone.id,
            center: zone.center.vec(),
            rotation: zone.rotation.quat(),
            half_extents: zone.half_extents.vec().abs(),
        })
        .collect();
    let mut nodes: Vec<&PosNodeFile> = file.pos_nodes.iter().collect();
    nodes.sort_by_key(|node| node.id);
    if nodes
        .iter()
        .enumerate()
        .any(|(i, node)| node.id as usize != i)
    {
        tracing::warn!("pos_nodes con ids salteados o repetidos: la pista no cuenta vueltas");
        nodes.clear();
    }
    layout.pos_nodes = nodes
        .iter()
        .map(|node| PosNode {
            id: node.id,
            position: node.position.vec(),
            distance: node.distance,
            prev: node.prev.clone(),
            next: node.next.clone(),
        })
        .collect();
    layout.start_node = file.start_node;
    layout.total_distance = if file.total_distance > 0.0 {
        file.total_distance
    } else {
        layout
            .pos_nodes
            .iter()
            .map(|node| node.distance)
            .fold(0.0, f32::max)
    };
    layout.kill_volumes = file
        .kill_volumes
        .iter()
        .map(|volume| KillVolume {
            center: volume.center.vec(),
            rotation: volume.rotation.quat(),
            half_extents: volume.half_extents.vec().abs(),
        })
        .collect();
}

#[derive(Deserialize, Default)]
pub(crate) struct V3 {
    x: f32,
    y: f32,
    z: f32,
}

impl V3 {
    pub(crate) fn vec(&self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }
}

fn read_layout(dir: &Path) -> Option<LayoutFile> {
    let Some(path) = find_file(dir, "layout.ron") else {
        tracing::warn!("pista sin layout.ron: se larga en el origen");
        return None;
    };
    let parsed = std::fs::read_to_string(&path)
        .map_err(|err| err.to_string())
        .and_then(|text| ron::from_str::<LayoutFile>(&text).map_err(|err| err.to_string()));
    match parsed {
        Ok(layout) => Some(layout),
        Err(err) => {
            tracing::warn!(%err, "layout.ron ilegible: se larga en el origen y sin objetos");
            None
        }
    }
}

fn start_grid(layout: Option<&LayoutFile>) -> Vec<StartSlot> {
    let slots: Vec<StartSlot> = layout
        .map(|layout| {
            layout
                .start_grid
                .iter()
                .map(|slot| StartSlot {
                    pos: slot.pos.vec(),
                    yaw: slot.yaw,
                })
                .collect()
        })
        .unwrap_or_default();
    if slots.is_empty() {
        if layout.is_some() {
            tracing::warn!("layout.ron sin start_grid: se larga en el origen");
        }
        return vec![StartSlot {
            pos: Vec3::new(0.0, 0.5, 0.0),
            yaw: 0.0,
        }];
    }
    slots
}
