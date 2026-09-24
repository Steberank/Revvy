//! `COL_MaterialInfo` y `COL_CorrugationInfo` de `newcoll.cpp` (versión final).
//! El índice es el `Material` de cada polígono del `.ncp`.

use glam::Vec3;

use super::units::MPH2OGU_SPEED;

pub const MATERIAL_SPARK: u32 = 1;
pub const MATERIAL_SKID: u32 = 2;
pub const MATERIAL_OUTOFBOUNDS: u32 = 4;
pub const MATERIAL_CORRUGATED: u32 = 8;
pub const MATERIAL_MOVES: u32 = 16;
pub const MATERIAL_DUSTY: u32 = 32;

pub const MATERIAL_DEFAULT: usize = 0;
pub const MATERIAL_BOUNDARY: usize = 8;

#[derive(Clone, Copy, Debug)]
pub struct Material {
    pub kind: u32,
    pub roughness: f32,
    pub gripiness: f32,
    pub hardness: f32,
    pub skid_colour: u32,
    pub corrugation: usize,
    pub vel: Vec3,
}

const fn mat(kind: u32, roughness: f32, gripiness: f32, hardness: f32, skid_colour: u32, corrugation: usize) -> Material {
    Material {
        kind,
        roughness,
        gripiness,
        hardness,
        skid_colour,
        corrugation,
        vel: Vec3::ZERO,
    }
}

const fn moving(corrugation: usize, vel: Vec3) -> Material {
    Material {
        kind: MATERIAL_MOVES | MATERIAL_CORRUGATED,
        roughness: 1.0,
        gripiness: 1.0,
        hardness: 0.0,
        skid_colour: 0,
        corrugation,
        vel,
    }
}

const SKID_SPARK: u32 = MATERIAL_SKID | MATERIAL_SPARK;

pub const CORRUG_NONE: usize = 0;
pub const CORRUG_PEBBLES: usize = 1;
pub const CORRUG_GRAVEL: usize = 2;
pub const CORRUG_STEEL: usize = 3;
pub const CORRUG_DIRT1: usize = 5;
pub const CORRUG_DIRT2: usize = 6;
pub const CORRUG_DIRT3: usize = 7;

pub static MATERIALS: [Material; 27] = [
    // DEFAULT
    mat(SKID_SPARK, 1.0, 1.0, 1.0, 0x707070, CORRUG_NONE),
    // MARBLE
    mat(SKID_SPARK, 0.9, 0.9, 0.5, 0x707070, CORRUG_NONE),
    // STONE
    mat(SKID_SPARK, 0.9, 0.9, 0.5, 0x909090, CORRUG_NONE),
    // WOOD
    mat(MATERIAL_SKID, 0.8, 0.8, 0.3, 0x404040, CORRUG_NONE),
    // SAND
    mat(MATERIAL_SKID | MATERIAL_DUSTY, 0.5, 0.6, 0.0, 0x402040, CORRUG_NONE),
    // PLASTIC
    mat(MATERIAL_SKID, 0.7, 0.9, 0.2, 0x404040, CORRUG_NONE),
    // CARPET1
    mat(MATERIAL_SKID, 1.0, 0.7, 0.0, 0x202020, CORRUG_NONE),
    // CARPET2
    mat(MATERIAL_SKID, 1.0, 0.7, 0.0, 0x202020, CORRUG_NONE),
    // BOUNDARY
    mat(MATERIAL_OUTOFBOUNDS, 1.0, 1.0, 0.0, 0x0, CORRUG_NONE),
    // GLASS
    mat(SKID_SPARK, 0.7, 0.8, 0.3, 0x303030, CORRUG_NONE),
    // ICE1
    mat(SKID_SPARK, 0.4, 0.4, 0.2, 0x303030, CORRUG_NONE),
    // METAL
    mat(SKID_SPARK, 0.8, 0.8, 0.4, 0x505050, CORRUG_NONE),
    // GRASS
    mat(MATERIAL_SKID | MATERIAL_CORRUGATED | MATERIAL_DUSTY, 0.7, 0.5, 0.0, 0x8060FF, CORRUG_STEEL),
    // BUMPMETAL
    mat(SKID_SPARK | MATERIAL_CORRUGATED, 0.8, 0.8, 0.4, 0x505050, CORRUG_STEEL),
    // PEBBLES
    mat(SKID_SPARK | MATERIAL_CORRUGATED | MATERIAL_DUSTY, 0.7, 0.7, 0.2, 0x303030, CORRUG_PEBBLES),
    // GRAVEL
    mat(SKID_SPARK | MATERIAL_CORRUGATED | MATERIAL_DUSTY, 0.9, 0.8, 0.2, 0x303030, CORRUG_GRAVEL),
    // CONVEYOR1 (la Y del original lleva un doble signo: `-Real(-5 * 25.749f)`)
    moving(CORRUG_STEEL, Vec3::new(-5.0 * 57.476, 5.0 * 25.749, 5.0 * 77.676)),
    // CONVEYOR2
    moving(CORRUG_STEEL, Vec3::new(5.0 * 57.476, 5.0 * 25.749, -5.0 * 77.676)),
    // DIRT1
    mat(MATERIAL_SKID | MATERIAL_CORRUGATED | MATERIAL_DUSTY, 1.0, 1.0, 0.0, 0x88bbdd, CORRUG_DIRT1),
    // DIRT2
    mat(MATERIAL_SKID | MATERIAL_CORRUGATED | MATERIAL_DUSTY, 1.0, 1.0, 0.0, 0x88bbdd, CORRUG_DIRT2),
    // DIRT3
    mat(MATERIAL_SKID | MATERIAL_CORRUGATED | MATERIAL_DUSTY, 1.0, 1.0, 0.0, 0x88bbdd, CORRUG_DIRT3),
    // ICE2
    mat(SKID_SPARK, 0.45, 0.45, 0.2, 0x303030, CORRUG_NONE),
    // ICE3
    mat(SKID_SPARK, 0.5, 0.5, 0.2, 0x303030, CORRUG_NONE),
    // WOOD2
    mat(MATERIAL_SKID, 0.8, 0.8, 0.4, 0x303030, CORRUG_NONE),
    // CONVEYOR_MARKET1
    moving(CORRUG_STEEL, Vec3::new(0.0, 0.0, MPH2OGU_SPEED * 4.0)),
    // CONVEYOR_MARKET2
    moving(CORRUG_STEEL, Vec3::new(-MPH2OGU_SPEED * 4.0, 0.0, 0.0)),
    // PAVING
    mat(SKID_SPARK, 0.9, 0.9, 0.5, 0x909090, CORRUG_NONE),
];

/// Un material fuera de tabla (`.ncp` de otro juego o corrupto) se trata como `DEFAULT`,
/// igual que la verificación de `USE_DEBUG_ROUTINES` en `LoadNewCollPolys`.
pub fn material_index(raw: u32) -> usize {
    let index = raw as usize;
    if index < MATERIALS.len() {
        index
    } else {
        MATERIAL_DEFAULT
    }
}

/// `CORRUGATION`: amplitud y longitudes de onda en X y Z.
#[derive(Clone, Copy, Debug)]
pub struct Corrugation {
    pub amp: f32,
    pub lx: f32,
    pub ly: f32,
}

pub static CORRUGATIONS: [Corrugation; 8] = [
    Corrugation { amp: 0.0, lx: 0.0, ly: 0.0 },
    Corrugation { amp: 3.0, lx: 70.0, ly: 70.0 },
    Corrugation { amp: 1.0, lx: 40.0, ly: 40.0 },
    Corrugation { amp: 1.0, lx: 40.0, ly: 40.0 },
    Corrugation { amp: 1.0, lx: 80.0, ly: 80.0 },
    Corrugation { amp: 1.0, lx: 80.0, ly: 80.0 },
    Corrugation { amp: 1.0, lx: 80.0, ly: 80.0 },
    Corrugation { amp: 1.0, lx: 80.0, ly: 80.0 },
];

/// `CorrugationAmp`.
pub fn corrugation_amp(cor: &Corrugation, dx: f32, dy: f32) -> f32 {
    let arg_x = 2.0 * std::f32::consts::PI * dx / cor.lx;
    let arg_z = 2.0 * std::f32::consts::PI * dy / cor.ly;
    cor.amp * (arg_x.cos() * arg_z.cos())
}

/// Sonido de derrape por material (`SfxSkidList`): `true` = `skid_rough`.
/// La tabla original tiene 26 entradas para 27 materiales: `PAVING` queda en cero,
/// que en PC es el sonido de motor. Acá usa `skid_normal`, como el resto de pisos duros.
pub static SKID_ROUGH: [bool; 27] = [
    false, // default
    false, // marble
    false, // stone
    false, // wood
    true,  // sand
    false, // plastic
    true,  // carpet tile
    true,  // carpet shag
    false, // boundary
    false, // glass
    true,  // ice 1
    false, // metal
    true,  // grass
    false, // bumpy metal
    true,  // pebbles
    true,  // gravel
    false, // conveyor 1
    false, // conveyor 2
    true,  // dirt 1
    true,  // dirt 2
    true,  // dirt 3
    true,  // ice 2
    true,  // ice 3
    false, // wood 2
    false, // market1 conveyor
    false, // market2 conveyor
    false, // paving
];
