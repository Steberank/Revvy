//! Parámetros del vehículo de Revvy: lo que el motor de física necesita de un auto.
//!
//! Espacio de Revvy (Y arriba, +Z adelante, +X a la izquierda del auto) y unidades SI:
//! metros, kilogramos, segundos. Un auto de Re-Volt llega acá por la traducción de
//! `parameters.txt` (`revolt_car`); un auto propio, leído de su `car.toml`. El motor no
//! distingue uno de otro.
//!
//! Las posiciones del auto son relativas a su centro de masa, en el marco del auto.

use serde::{Deserialize, Serialize};

/// Ruedas: delantera izquierda, delantera derecha, trasera izquierda, trasera derecha.
pub const WHEEL_COUNT: usize = 4;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VehicleParams {
    /// Masa del chasis (kg). Las ruedas no suman masa al cuerpo.
    pub mass: f32,
    /// Tensor de inercia del chasis (kg·m²), por filas, en el marco del auto.
    pub inertia: [[f32; 3]; 3],
    /// Gravedad que usa el tope de fricción de las ruedas (m/s²). No mueve el auto: eso
    /// lo hace la gravedad del mundo.
    pub gravity: f32,
    /// Rebote del chasis contra el mundo (0–1), multiplicado por la dureza de la superficie.
    pub hardness: f32,
    /// Freno de la velocidad lineal: la velocidad se multiplica por `1 − 120 · r · dt`.
    pub resistance: f32,
    /// Freno de la velocidad angular con alguna rueda en contacto, igual que `resistance`.
    pub angular_resistance: f32,
    /// Factor de `angular_resistance` con las cuatro ruedas en el aire.
    pub angular_resistance_air: f32,
    /// Agarre del chasis cuando se arrastra (s/m).
    pub grip: f32,
    pub static_friction: f32,
    pub kinetic_friction: f32,
    /// Dónde va el origen de la malla del chasis, relativo al centro de masa (m).
    pub body_offset: [f32; 3],
    /// Velocidad del volante: fracción del recorrido por segundo.
    pub steer_rate: f32,
    /// Velocidad con la que el voltaje del motor llega al pedido: fracción por segundo.
    pub engine_rate: f32,
    /// Velocidad a la que el torque del motor se anula (m/s).
    pub top_speed: f32,
    /// Empuje contra el piso con dos ruedas de un mismo lado en el aire (kg/s).
    pub down_force: f32,
    pub wheels: [WheelParams; WHEEL_COUNT],
    /// En un auto propio puede faltar: sale de `collision.glb`.
    #[serde(default)]
    pub chassis: ChassisShape,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WheelParams {
    pub present: bool,
    pub powered: bool,
    pub steered: bool,
    /// Anclaje de la rueda (m): donde está el buje con la suspensión en reposo.
    pub offset: [f32; 3],
    /// Centro de la esfera de la rueda relativo al anclaje (m).
    #[serde(default)]
    pub centre_offset: [f32; 3],
    /// Radio (m).
    pub radius: f32,
    /// Masa de la rueda (kg): solo entra en la suspensión y en el giro de la rueda.
    pub mass: f32,
    /// Gravedad de la rueda en la suspensión (m/s²).
    pub gravity: f32,
    /// Recorrido de la suspensión hacia cada lado del reposo (m).
    pub max_travel: f32,
    /// Ancho de la marca de derrape (m).
    #[serde(default)]
    pub skid_width: f32,
    /// Ángulo de la rueda con el volante a tope (rad). Solo en ruedas que doblan.
    #[serde(default)]
    pub steer_ratio: f32,
    /// Torque del motor por unidad de voltaje (kg·m²/s²). Negativo gira al revés.
    #[serde(default)]
    pub engine_ratio: f32,
    /// Freno del eje cuando no hay motor o se frena: torque por velocidad de rodadura (kg·m).
    pub axle_friction: f32,
    /// Cuánto se frena el giro de la rueda por sí solo: la velocidad angular se multiplica
    /// por `1 − 120 · d · dt`. Sin unidades.
    #[serde(default = "default_spin_damping")]
    pub spin_damping: f32,
    /// Agarre lateral (s/m): cuánto frena la rueda el deslizamiento de costado.
    pub grip: f32,
    /// Fricción estática contra el piso (antes de patinar).
    pub static_friction: f32,
    /// Fricción cinética contra el piso (patinando).
    pub kinetic_friction: f32,
    pub spring: SpringParams,
}

fn default_spin_damping() -> f32 {
    0.01
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpringParams {
    /// Rigidez (N/m).
    pub stiffness: f32,
    /// Amortiguación (N·s/m).
    pub damping: f32,
    /// Qué parte del golpe de la rueda devuelve la suspensión. Negativa se come el golpe.
    pub restitution: f32,
}

/// Forma de choque del chasis, en el marco del auto y relativa al centro de masa.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChassisShape {
    /// Esferas que tocan el mundo: `[x, y, z, radio]` (m).
    #[serde(default)]
    pub spheres: Vec<[f32; 4]>,
    /// Cascos convexos que chocan con otros autos, como en el modo Simulación de Re-Volt.
    /// Sin esferas, también tocan el mundo.
    #[serde(default)]
    pub hulls: Vec<Vec<[f32; 3]>>,
}

/// Sonido de motor del auto. El motor de sonido elige la curva de volumen y tono.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineSound {
    /// Suben el volumen y el tono con las vueltas de las ruedas.
    #[default]
    Electric,
    /// Volumen fijo; sube solo el tono.
    Petrol,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CarSound {
    pub engine: EngineSound,
    /// Sample de motor propio. Ruta relativa a la raíz de contenido o a la carpeta del
    /// auto; sin él, el motor de sonido usa el de `engine`.
    pub sample: Option<String>,
}
