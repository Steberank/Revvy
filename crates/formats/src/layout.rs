//! `TrackLayout`: el mismo tipo para el legado adaptado y para `layout.ron`.

use glam::{Quat, Vec3};

/// Material de contacto. Un id de Re-Volt desconocido cae en `Road`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceType {
    Road,
    Dirt,
    Ice,
    Grass,
    Metal,
    Wood,
    Sand,
}

impl SurfaceType {
    pub fn from_revolt(material: u32) -> Self {
        match material {
            3 | 23 => Self::Wood,
            4 => Self::Sand,
            10 | 21 | 22 => Self::Ice,
            11 | 13 => Self::Metal,
            12 => Self::Grass,
            14 | 15 | 18 | 19 | 20 => Self::Dirt,
            _ => Self::Road,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TrackLayout {
    pub version: u32,
    pub start_node: u32,
    pub total_distance: f32,
    pub start_grid: Vec<StartSlot>,
    pub zones: Vec<TrackZone>,
    pub pos_nodes: Vec<PosNode>,
    pub ai_nodes: Vec<AiNode>,
    pub pickups: Vec<PickupSpawn>,
    pub pickup_odds: Option<PowerupOdds>,
    pub force_fields: Vec<ForceField>,
    pub surface_effects: Option<Vec<(SurfaceType, SurfaceEffect)>>,
    pub surface_volumes: Vec<SurfaceVolume>,
    pub param_mods: Vec<ParamMod>,
    pub kill_volumes: Vec<KillVolume>,
}

impl Default for TrackLayout {
    fn default() -> Self {
        Self {
            version: 1,
            start_node: 0,
            total_distance: 0.0,
            start_grid: Vec::new(),
            zones: Vec::new(),
            pos_nodes: Vec::new(),
            ai_nodes: Vec::new(),
            pickups: Vec::new(),
            pickup_odds: None,
            force_fields: Vec::new(),
            surface_effects: None,
            surface_volumes: Vec::new(),
            param_mods: Vec::new(),
            kill_volumes: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct StartSlot {
    pub pos: Vec3,
    pub yaw: f32,
}

#[derive(Clone, Debug)]
pub struct TrackZone {
    pub id: i32,
    pub center: Vec3,
    pub rotation: Quat,
    pub half_extents: Vec3,
}

#[derive(Clone, Debug)]
pub struct PosNode {
    pub id: u32,
    pub position: Vec3,
    pub distance: f32,
    pub prev: Vec<i32>,
    pub next: Vec<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AiFlags {
    pub racing: bool,
    pub slowdown: bool,
    pub pickup_route: bool,
    pub careful: bool,
    pub wall_left: bool,
    pub wall_right: bool,
}

impl Default for AiFlags {
    fn default() -> Self {
        Self {
            racing: true,
            slowdown: false,
            pickup_route: false,
            careful: false,
            wall_left: false,
            wall_right: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AiNode {
    pub id: u32,
    pub left: Vec3,
    pub right: Vec3,
    pub racing_t: f32,
    pub overtaking_t: f32,
    pub prev: Vec<i32>,
    pub next: Vec<i32>,
    pub flags: AiFlags,
    pub speed_limit: Option<f32>,
}

#[derive(Clone, Debug)]
pub struct PickupSpawn {
    pub pos: Vec3,
    pub respawn_secs: f32,
    pub odds: Option<PowerupOdds>,
}

#[derive(Clone, Debug)]
pub struct PowerupOdds {
    pub weights: Vec<(String, u32)>,
}

#[derive(Clone, Debug)]
pub enum ForceKind {
    GravityScale(f32),
    Wind(Vec3),
    ConstantForce(Vec3),
}

#[derive(Clone, Debug)]
pub struct ForceField {
    pub id: u32,
    pub center: Vec3,
    pub rotation: Quat,
    pub half_extents: Vec3,
    pub kind: ForceKind,
}

#[derive(Clone, Copy, Debug)]
pub struct SurfaceEffect {
    pub friction: f32,
    pub lateral_grip: f32,
    pub rolling_resist: f32,
    pub speed_factor: f32,
}

#[derive(Clone, Debug)]
pub struct SurfaceVolume {
    pub center: Vec3,
    pub rotation: Quat,
    pub half_extents: Vec3,
    pub surface: SurfaceType,
}

#[derive(Clone, Debug)]
pub struct ParamMod {
    pub target: String,
    pub op: String,
    pub stat: String,
    pub surface: Option<SurfaceType>,
    pub value: f32,
}

#[derive(Clone, Debug)]
pub struct KillVolume {
    pub center: Vec3,
    pub rotation: Quat,
    pub half_extents: Vec3,
}

pub fn links(values: [i32; 4]) -> Vec<i32> {
    values.into_iter().filter(|id| *id >= 0).collect()
}

pub fn links2(values: [i32; 2]) -> Vec<i32> {
    values.into_iter().filter(|id| *id >= 0).collect()
}
