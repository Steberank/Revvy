//! `TrackLayout`: el mismo tipo para el legado adaptado y para `layout.ron`.

use glam::{Quat, Vec3};

/// Superficie de contacto. Cubre los 27 materiales de Re-Volt (`COL_MaterialInfo`), en el
/// mismo orden que su índice; los siete de siempre (`Road`, `Dirt`, `Ice`, `Grass`,
/// `Metal`, `Wood`, `Sand`) son `DEFAULT`, `DIRT1`, `ICE1`, `GRASS`, `METAL`, `WOOD` y `SAND`.
/// En una pista `.glb`, el nombre del material de `Collision` es el nombre de la variante.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SurfaceType {
    /// `DEFAULT`.
    Road,
    /// `MARBLE`.
    Marble,
    /// `STONE`.
    Stone,
    /// `WOOD`.
    Wood,
    /// `SAND`.
    Sand,
    /// `PLASTIC`.
    Plastic,
    /// `CARPET1`.
    CarpetTile,
    /// `CARPET2`.
    CarpetShag,
    /// `BOUNDARY`.
    Boundary,
    /// `GLASS`.
    Glass,
    /// `ICE1`.
    Ice,
    /// `METAL`.
    Metal,
    /// `GRASS`.
    Grass,
    /// `BUMPMETAL`.
    BumpMetal,
    /// `PEBBLES`.
    Pebbles,
    /// `GRAVEL`.
    Gravel,
    /// `CONVEYOR1`.
    Conveyor1,
    /// `CONVEYOR2`.
    Conveyor2,
    /// `DIRT1`.
    Dirt,
    /// `DIRT2`.
    Dirt2,
    /// `DIRT3`.
    Dirt3,
    /// `ICE2`.
    Ice2,
    /// `ICE3`.
    Ice3,
    /// `WOOD2`.
    Wood2,
    /// `CONVEYOR_MARKET1`.
    ConveyorMarket1,
    /// `CONVEYOR_MARKET2`.
    ConveyorMarket2,
    /// `PAVING`.
    Paving,
}

impl SurfaceType {
    pub const ALL: [SurfaceType; 27] = [
        Self::Road,
        Self::Marble,
        Self::Stone,
        Self::Wood,
        Self::Sand,
        Self::Plastic,
        Self::CarpetTile,
        Self::CarpetShag,
        Self::Boundary,
        Self::Glass,
        Self::Ice,
        Self::Metal,
        Self::Grass,
        Self::BumpMetal,
        Self::Pebbles,
        Self::Gravel,
        Self::Conveyor1,
        Self::Conveyor2,
        Self::Dirt,
        Self::Dirt2,
        Self::Dirt3,
        Self::Ice2,
        Self::Ice3,
        Self::Wood2,
        Self::ConveyorMarket1,
        Self::ConveyorMarket2,
        Self::Paving,
    ];

    /// Material de un polígono del `.ncp`. Uno fuera de tabla cae en `Road`, como en el juego.
    pub fn from_revolt(material: u32) -> Self {
        Self::ALL.get(material as usize).copied().unwrap_or(Self::Road)
    }

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Road => "Road",
            Self::Marble => "Marble",
            Self::Stone => "Stone",
            Self::Wood => "Wood",
            Self::Sand => "Sand",
            Self::Plastic => "Plastic",
            Self::CarpetTile => "CarpetTile",
            Self::CarpetShag => "CarpetShag",
            Self::Boundary => "Boundary",
            Self::Glass => "Glass",
            Self::Ice => "Ice",
            Self::Metal => "Metal",
            Self::Grass => "Grass",
            Self::BumpMetal => "BumpMetal",
            Self::Pebbles => "Pebbles",
            Self::Gravel => "Gravel",
            Self::Conveyor1 => "Conveyor1",
            Self::Conveyor2 => "Conveyor2",
            Self::Dirt => "Dirt",
            Self::Dirt2 => "Dirt2",
            Self::Dirt3 => "Dirt3",
            Self::Ice2 => "Ice2",
            Self::Ice3 => "Ice3",
            Self::Wood2 => "Wood2",
            Self::ConveyorMarket1 => "ConveyorMarket1",
            Self::ConveyorMarket2 => "ConveyorMarket2",
            Self::Paving => "Paving",
        }
    }

    /// Nombre de material de una pista `.glb`. Distingue mayúsculas, como los nodos.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|surface| surface.name() == name)
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

/// Una superficie que la pista redefine: `MATERIAL` y `CORRUGATION` de `properties.txt`
/// en las pistas de RVGL. Lo que queda en `None` sigue como en Re-Volt.
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceTuning {
    pub surface: SurfaceType,
    /// Multiplica la fricción (`Roughness`).
    pub roughness: Option<f32>,
    /// Multiplica el agarre lateral de las ruedas (`Grip`).
    pub grip: Option<f32>,
    /// Multiplica el rebote del chasis (`Hardness`).
    pub hardness: Option<f32>,
    /// Baches: amplitud y largos de onda en X y Z (m). `Some(None)`: sin baches.
    pub corrugation: Option<Option<[f32; 3]>>,
    /// Cinta transportadora (m/s, ejes de Revvy). `Some(Vec3::ZERO)`: no mueve.
    pub conveyor: Option<Vec3>,
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
