//! Capa de traducción de autos de Re-Volt: `CAR_INFO` (+ `.hul`) → parámetros del
//! vehículo de Revvy.
//!
//! Una unidad de Re-Volt mide `s` = 5 mm y los ejes se giran con `C = diag(−1, −1, 1)`,
//! que es una rotación: los productos punto y cruz no cambian. Solo cambian las
//! magnitudes con unidades de largo, cada una según su dimensión:
//!
//! | magnitud | factor |
//! | --- | --- |
//! | largos, velocidades, aceleraciones | `s` |
//! | inercias, torques del motor | `s²` |
//! | fricción de eje (torque por velocidad) | `s` |
//! | freno del giro de la rueda (`spin_damping`) | 1 |
//! | agarre (por velocidad) | `1/s` |
//! | masas, rigidez y amortiguación de resortes, coeficientes de fricción | 1 |
//!
//! Así el mismo modelo en metros da el mismo movimiento que en unidades de Re-Volt.

use crate::axes;
use crate::hul::NativeHull;
use crate::inf::CarInfo;
use crate::vehicle::{CarSound, ChassisShape, EngineSound, SpringParams, VehicleParams, WheelParams};

/// `CAR_CLASS_ELEC`.
const CLASS_ELECTRIC: i32 = 0;

fn scale() -> f32 {
    axes::REVOLT_TO_METERS
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// Punto del modelo de Re-Volt, relativo al centro de masa, en el marco del auto de Revvy.
fn point(p: [f32; 3], com: [f32; 3]) -> [f32; 3] {
    axes::position(add(p, com)).to_array()
}

/// `C · I · C · s²`.
fn inertia(rows: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let sign = [-1.0, -1.0, 1.0];
    let s2 = scale() * scale();
    let mut out = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            out[i][j] = sign[i] * sign[j] * rows[i][j] * s2;
        }
    }
    out
}

/// `SetupCar` + `SetupWheel` + `LoadOneCarModelSet` en unidades de Revvy. `com` es el
/// desplazamiento que `MoveCarCoM` suma a todas las posiciones del modelo.
pub fn vehicle_params(info: &CarInfo, hull: &NativeHull) -> VehicleParams {
    let s = scale();
    let com = info.com;
    let wheels = std::array::from_fn(|i| {
        let w = &info.wheels[i];
        let spring = &info.springs[i];
        WheelParams {
            present: w.is_present,
            powered: w.is_powered,
            steered: w.is_turnable,
            offset: point(w.offset1, com),
            centre_offset: axes::position(w.offset2).to_array(),
            radius: w.radius * s,
            mass: w.mass,
            gravity: w.gravity * s,
            max_travel: w.max_pos * s,
            skid_width: w.skid_width * s,
            steer_ratio: w.steer_ratio,
            engine_ratio: w.engine_ratio * s * s,
            axle_friction: w.axle_friction * s,
            // `UpdateCarWheel` usa el mismo `AxleFriction` como freno sin unidades.
            spin_damping: w.axle_friction,
            grip: w.grip / s,
            static_friction: w.static_friction,
            kinetic_friction: w.kinetic_friction,
            spring: SpringParams {
                stiffness: spring.stiffness,
                damping: spring.damping,
                restitution: spring.restitution,
            },
        }
    });
    VehicleParams {
        mass: info.body.mass,
        inertia: inertia(info.body.inertia),
        gravity: info.body.gravity * s,
        hardness: info.body.hardness,
        resistance: info.body.resistance,
        angular_resistance: info.body.ang_res,
        angular_resistance_air: info.body.res_mod,
        grip: info.body.grip / s,
        static_friction: info.body.static_friction,
        kinetic_friction: info.body.kinetic_friction,
        body_offset: point(info.body.offset, com),
        steer_rate: info.steer_rate,
        engine_rate: info.engine_rate,
        top_speed: info.top_speed_mph * axes::MPH2OGU_SPEED * s,
        down_force: info.down_force_mod,
        wheels,
        chassis: ChassisShape {
            spheres: hull
                .spheres
                .iter()
                .map(|&[x, y, z, r]| {
                    let [px, py, pz] = point([x, y, z], com);
                    [px, py, pz, r * s]
                })
                .collect(),
            hulls: hull
                .hulls
                .iter()
                .filter(|points| points.len() >= 4)
                .map(|points| points.iter().map(|&p| point(p, com)).collect())
                .collect(),
        },
    }
}

/// Sonido de motor: la clase elige la curva y `SFXENGINE` (RVGL) el sample.
pub fn car_sound(info: &CarInfo) -> CarSound {
    CarSound {
        engine: if info.class == CLASS_ELECTRIC {
            EngineSound::Electric
        } else {
            EngineSound::Petrol
        },
        sample: info.sfx_engine.clone(),
    }
}
