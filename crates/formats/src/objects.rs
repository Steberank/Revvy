//! Objetos de una pista: qué son (modelo, forma, física, sonido) y dónde y cuándo aparecen.
//! También los autos sin conductor, que nadie maneja y se mueven cuando los empujan.
//!
//! Salen de la traducción de un `.fob` de Re-Volt (`revolt_objects.rs`) o de la carpeta
//! `objects/` de una pista propia (`revvy_objects.rs`). El motor recibe los mismos tipos
//! para los dos, en metros y con Y arriba.

use std::path::PathBuf;

use glam::{Quat, Vec3};

use crate::mesh::VisualMesh;
use crate::CarDef;

#[derive(Clone, Debug, Default)]
pub struct TrackObjects {
    pub kinds: Vec<ObjectKind>,
    pub spawns: Vec<ObjectSpawn>,
    /// Autos sin conductor: el chango de los supermercados de Re-Volt o los de `objects/`
    /// en una pista propia.
    pub cars: Vec<CarDef>,
    pub car_spawns: Vec<CarSpawn>,
}

/// Un tipo de objeto: se dibuja, choca y suena igual en todas sus apariciones.
#[derive(Clone, Debug)]
pub struct ObjectKind {
    /// Para los logs: `cono`, `basketball`…
    pub name: String,
    /// Mallas en el espacio del objeto, con el centro de masa en el origen.
    pub meshes: Vec<VisualMesh>,
    /// Texturas propias por página. Vacío: las mallas usan las páginas de la pista, como los
    /// objetos de Re-Volt, que traen su textura en las del nivel.
    pub textures: Vec<(i16, image::RgbaImage)>,
    pub shape: ObjectShape,
    /// kg.
    pub mass: f32,
    /// Inercia principal en los ejes del objeto (kg·m²). `None`: sale de la forma.
    pub inertia: Option<[f32; 3]>,
    /// Contra la pista se multiplica por la rugosidad de la superficie; contra otro cuerpo,
    /// por la fricción del otro, como en `body.cpp` de Re-Volt.
    pub friction: f32,
    /// Rebote, con las mismas reglas que `friction`.
    pub restitution: f32,
    /// Freno de la velocidad lineal y angular (1/s).
    pub linear_damping: f32,
    pub angular_damping: f32,
    /// Queda donde está, aunque sea en el aire, hasta que algo lo toca.
    pub starts_asleep: bool,
    pub impact_sound: Option<ImpactSound>,
    /// Las puntas de su camino, si una aparición le da uno (`ObjectSpawn::motion`).
    pub motion_sounds: MotionSounds,
}

#[derive(Clone, Debug)]
pub enum ObjectShape {
    /// Esfera centrada en el origen del objeto.
    Sphere { radius: f32 },
    /// Cascos convexos, con sus puntos en el espacio del objeto.
    Hulls(Vec<Vec<Vec3>>),
}

/// El golpe que suena cuando el objeto choca fuerte (`AI_BangNoiseHandler`).
#[derive(Clone, Debug)]
pub struct ImpactSound {
    pub file: PathBuf,
    /// Cambio de velocidad mínimo del golpe para que suene (m/s).
    pub min_speed: f32,
    /// El volumen (0–127) es `20 × golpe en m/s + volume_offset`: el de Re-Volt, que suma
    /// un décimo del golpe en unidades por segundo.
    pub volume_offset: f32,
}

impl ImpactSound {
    /// Volumen para un golpe de `speed` m/s, o `None` si no alcanza para sonar.
    pub fn volume(&self, speed: f32) -> Option<i32> {
        (speed > self.min_speed)
            .then(|| (speed * 20.0 + self.volume_offset).clamp(0.0, 127.0) as i32)
    }
}

/// Suenan a todo volumen en el punto de partida del camino, como las puertas de market2
/// (`AI_SliderHandler`).
#[derive(Clone, Debug, Default)]
pub struct MotionSounds {
    /// Cuando sale del punto de partida.
    pub start: Option<PathBuf>,
    /// Cuando llega a la otra punta y empieza a volver.
    pub turn: Option<PathBuf>,
}

/// Una aparición: qué objeto, dónde, con qué velocidad y cuándo.
#[derive(Clone, Debug)]
pub struct ObjectSpawn {
    /// Índice en `TrackObjects::kinds`.
    pub kind: usize,
    pub pos: Vec3,
    pub rot: Quat,
    /// Velocidad inicial (m/s).
    pub velocity: Vec3,
    pub when: SpawnWhen,
    /// `None`: lo mueve la física. Con un camino lo sigue sin que nada lo frene, desde
    /// `pos`, y empuja lo que encuentra.
    pub motion: Option<ObjectMotion>,
}

/// Un camino fijo que se repite. El reloj es el de la carrera: dos objetos con el mismo
/// camino van juntos aunque uno aparezca después.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ObjectMotion {
    /// Va en línea recta hasta `offset` (m, en ejes del mundo) y vuelve, siempre a la misma
    /// velocidad: `period` segundos ida y vuelta.
    Slide { offset: Vec3, period: f32 },
}

/// Una punta del camino.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionCue {
    /// Sale del punto de partida.
    Start,
    /// Llega a la otra punta y empieza a volver.
    Turn,
}

impl ObjectMotion {
    /// Cuánto se alejó del punto de partida a los `time` segundos.
    pub fn offset_at(&self, time: f64) -> Vec3 {
        match *self {
            Self::Slide { offset, period } => {
                let phase = (time / f64::from(period)).rem_euclid(1.0);
                offset * (1.0 - (2.0 * phase - 1.0).abs()) as f32
            }
        }
    }

    /// La punta por la que pasó entre `from` y `to` segundos, si pasó por una.
    pub fn cue_between(&self, from: f64, to: f64) -> Option<MotionCue> {
        match *self {
            Self::Slide { period, .. } => {
                let half = f64::from(period) / 2.0;
                let (before, after) = ((from / half).floor(), (to / half).floor());
                (after > before).then(|| {
                    if after.rem_euclid(2.0) == 0.0 {
                        MotionCue::Start
                    } else {
                        MotionCue::Turn
                    }
                })
            }
        }
    }
}

/// Un auto sin conductor: nadie lo maneja, Tab no lo elige y no suena. Se mueve cuando lo
/// empujan.
#[derive(Clone, Debug)]
pub struct CarSpawn {
    /// Índice en `TrackObjects::cars`.
    pub car: usize,
    /// Centro de masa (m).
    pub pos: Vec3,
    /// Hacia dónde mira, girando sobre +Y (rad).
    pub yaw: f32,
    /// Se endereza solo cuando la Y de su eje vertical baja de esto, como el chango
    /// (`TrolleyAIHandler`: 0.5). `None`: queda como cae.
    pub self_righting: Option<f32>,
}

#[derive(Clone, Debug)]
pub enum SpawnWhen {
    /// Al empezar la carrera.
    Start,
    /// Cuando un auto entra en la caja, como `TriggerObjectThrower`.
    Trigger(SpawnTrigger),
}

/// Caja orientada que dispara una aparición.
#[derive(Clone, Debug)]
pub struct SpawnTrigger {
    pub center: Vec3,
    pub rotation: Quat,
    pub half_extents: Vec3,
    /// `None`: una vez por carrera (Re-Volt). Con segundos, vuelve a disparar ese tiempo
    /// después de la última vez.
    pub rearm: Option<f32>,
}

impl SpawnTrigger {
    pub fn contains(&self, point: Vec3) -> bool {
        let local = self.rotation.inverse() * (point - self.center);
        local.abs().cmple(self.half_extents).all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trigger_is_an_oriented_box() {
        let trigger = SpawnTrigger {
            center: Vec3::new(10.0, 0.0, 0.0),
            rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            half_extents: Vec3::new(1.0, 1.0, 4.0),
            rearm: None,
        };
        // Girada 90°, la caja larga en Z queda larga en X.
        assert!(trigger.contains(Vec3::new(13.5, 0.0, 0.0)));
        assert!(!trigger.contains(Vec3::new(10.0, 0.0, 3.5)));
    }

    #[test]
    fn a_slide_goes_and_comes_back() {
        let motion = ObjectMotion::Slide {
            offset: Vec3::new(0.0, 0.0, 2.0),
            period: 3.0,
        };
        assert_eq!(motion.offset_at(0.0), Vec3::ZERO);
        assert!((motion.offset_at(0.75).z - 1.0).abs() < 1e-6);
        assert!((motion.offset_at(1.5).z - 2.0).abs() < 1e-6);
        assert!((motion.offset_at(2.25).z - 1.0).abs() < 1e-6);
        assert!(motion.offset_at(3.0).length() < 1e-6);
        // `AI_SliderHandler`: abre al salir, cierra al llegar a la otra punta. Al arrancar
        // la carrera no suena.
        assert_eq!(motion.cue_between(0.0, 0.01), None);
        assert_eq!(motion.cue_between(1.49, 1.51), Some(MotionCue::Turn));
        assert_eq!(motion.cue_between(2.99, 3.01), Some(MotionCue::Start));
        assert_eq!(motion.cue_between(3.01, 4.4), None);
    }

    #[test]
    fn a_soft_knock_is_silent() {
        let sound = ImpactSound {
            file: PathBuf::from("roadcone.wav"),
            min_speed: 1.5,
            volume_offset: -15.0,
        };
        assert_eq!(sound.volume(1.0), None);
        assert_eq!(sound.volume(3.0), Some(45));
        assert_eq!(sound.volume(50.0), Some(127));
    }
}
