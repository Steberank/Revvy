//! Perfil físico de cada superficie del motor de Revvy.
//!
//! Los 27 `SurfaceType` llevan los valores de `COL_MaterialInfo` de Re-Volt, pasados a
//! metros: así una pista de Re-Volt agarra igual y una `.glb` puede usar las mismas
//! superficies.

use glam::Vec3;
use revvy_formats::SurfaceType;

#[derive(Clone, Copy, Debug)]
pub struct SurfaceProfile {
    /// Multiplica la fricción estática y cinética de ruedas y chasis.
    pub roughness: f32,
    /// Multiplica el agarre lateral de las ruedas.
    pub gripiness: f32,
    /// Rebote del chasis (multiplica su `hardness`).
    pub hardness: f32,
    /// Baches: la profundidad del contacto de la rueda sube y baja con la posición.
    pub corrugation: Option<Corrugation>,
    /// Velocidad de la cinta transportadora (m/s, espacio de Revvy).
    pub conveyor: Vec3,
    /// El derrape suena áspero (`skid_rough`) en vez de liso.
    pub skid_rough: bool,
    /// Borde del mundo: un golpe acá no cuenta.
    pub boundary: bool,
}

/// Amplitud y largos de onda en X y Z (m).
#[derive(Clone, Copy, Debug)]
pub struct Corrugation {
    pub amp: f32,
    pub lx: f32,
    pub lz: f32,
}

impl Corrugation {
    /// Profundidad extra en un punto del piso: `amp · cos(2πx/lx) · cos(2πz/lz)`.
    pub fn depth(&self, x: f32, z: f32) -> f32 {
        let tau = std::f32::consts::TAU;
        self.amp * (tau * x / self.lx).cos() * (tau * z / self.lz).cos()
    }
}

const PEBBLES: Option<Corrugation> = Some(Corrugation { amp: 0.015, lx: 0.35, lz: 0.35 });
const GRAVEL: Option<Corrugation> = Some(Corrugation { amp: 0.005, lx: 0.2, lz: 0.2 });
const STEEL: Option<Corrugation> = Some(Corrugation { amp: 0.005, lx: 0.2, lz: 0.2 });
const DIRT: Option<Corrugation> = Some(Corrugation { amp: 0.005, lx: 0.4, lz: 0.4 });

const fn flat(roughness: f32, gripiness: f32, hardness: f32, skid_rough: bool) -> SurfaceProfile {
    SurfaceProfile {
        roughness,
        gripiness,
        hardness,
        corrugation: None,
        conveyor: Vec3::ZERO,
        skid_rough,
        boundary: false,
    }
}

const fn bumpy(roughness: f32, gripiness: f32, hardness: f32, corrugation: Option<Corrugation>, skid_rough: bool) -> SurfaceProfile {
    SurfaceProfile {
        roughness,
        gripiness,
        hardness,
        corrugation,
        conveyor: Vec3::ZERO,
        skid_rough,
        boundary: false,
    }
}

const fn conveyor(velocity: Vec3) -> SurfaceProfile {
    SurfaceProfile {
        roughness: 1.0,
        gripiness: 1.0,
        hardness: 0.0,
        corrugation: STEEL,
        conveyor: velocity,
        skid_rough: false,
        boundary: false,
    }
}

/// En el orden de `SurfaceType::ALL`.
static PROFILES: [SurfaceProfile; 27] = [
    flat(1.0, 1.0, 1.0, false),                  // Road
    flat(0.9, 0.9, 0.5, false),                  // Marble
    flat(0.9, 0.9, 0.5, false),                  // Stone
    flat(0.8, 0.8, 0.3, false),                  // Wood
    flat(0.5, 0.6, 0.0, true),                   // Sand
    flat(0.7, 0.9, 0.2, false),                  // Plastic
    flat(1.0, 0.7, 0.0, true),                   // CarpetTile
    flat(1.0, 0.7, 0.0, true),                   // CarpetShag
    SurfaceProfile {
        boundary: true,
        ..flat(1.0, 1.0, 0.0, false)
    }, // Boundary
    flat(0.7, 0.8, 0.3, false),                  // Glass
    flat(0.4, 0.4, 0.2, true),                   // Ice
    flat(0.8, 0.8, 0.4, false),                  // Metal
    bumpy(0.7, 0.5, 0.0, STEEL, true),           // Grass
    bumpy(0.8, 0.8, 0.4, STEEL, false),          // BumpMetal
    bumpy(0.7, 0.7, 0.2, PEBBLES, true),         // Pebbles
    bumpy(0.9, 0.8, 0.2, GRAVEL, true),          // Gravel
    conveyor(Vec3::new(1.4369, -0.6437, 1.9419)),  // Conveyor1
    conveyor(Vec3::new(-1.4369, -0.6437, -1.9419)), // Conveyor2
    bumpy(1.0, 1.0, 0.0, DIRT, true),            // Dirt
    bumpy(1.0, 1.0, 0.0, DIRT, true),            // Dirt2
    bumpy(1.0, 1.0, 0.0, DIRT, true),            // Dirt3
    flat(0.45, 0.45, 0.2, true),                 // Ice2
    flat(0.5, 0.5, 0.2, true),                   // Ice3
    flat(0.8, 0.8, 0.4, false),                  // Wood2
    conveyor(Vec3::new(0.0, 0.0, 1.7889)),       // ConveyorMarket1
    conveyor(Vec3::new(1.7889, 0.0, 0.0)),       // ConveyorMarket2
    flat(0.9, 0.9, 0.5, false),                  // Paving
];

pub fn profile(surface: SurfaceType) -> &'static SurfaceProfile {
    &PROFILES[surface.index()]
}
