//! Motor de sonido de Revvy: Kira como salida y un modelo 3D que reproduce el de
//! `sfx.cpp` de Re-Volt PC, en metros.
//!
//! Cada auto (motor, derrape, roce, servo, golpes), los objetos (golpes y puntas de sus
//! caminos) y los emisores de la pista se actualizan una vez por frame; después el
//! mezclador recalcula volumen, paneo y Doppler de cada sonido 3D respecto de la cámara.

mod car;
mod level;
mod mixer;

use std::path::Path;

use revvy_formats::{CarDef, MotionCue, ObjectKind, TrackSounds};
use revvy_physics::VehicleSound;

pub use mixer::Listener;

use car::CarSounds;
use level::LevelSounds;
use mixer::{Mixer, SoundId, SFX_SAMPLE_RATE};

pub struct Audio {
    mixer: Mixer,
    cars: Vec<CarSounds>,
    level: LevelSounds,
    /// Los sonidos de cada tipo de objeto, en el orden de `ObjectCue`.
    objects: Vec<[Option<SoundId>; 3]>,
}

/// Qué suena de un objeto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectCue {
    /// Un golpe fuerte (`ImpactSound`).
    Impact,
    /// Sale del punto de partida de su camino.
    Start,
    /// Llega a la otra punta de su camino y vuelve.
    Turn,
}

impl From<MotionCue> for ObjectCue {
    fn from(cue: MotionCue) -> Self {
        match cue {
            MotionCue::Start => Self::Start,
            MotionCue::Turn => Self::Turn,
        }
    }
}

/// Un objeto que suena en este frame.
#[derive(Clone, Copy, Debug)]
pub struct ObjectSound {
    /// Índice en `TrackObjects::kinds`.
    pub kind: usize,
    pub cue: ObjectCue,
    pub pos: glam::Vec3,
    /// 0–127; el de un golpe ya sale de `ImpactSound::volume`.
    pub volume: i32,
}

impl Audio {
    /// `cars` va con la posición de largada de cada auto; `objects` son los tipos de
    /// objeto de la pista.
    pub fn new(
        content: &Path,
        sounds: &TrackSounds,
        cars: &[(&CarDef, glam::Vec3)],
        objects: &[ObjectKind],
        master_vol: i32,
    ) -> Self {
        let mut mixer = Mixer::new(master_vol);
        let cars = cars
            .iter()
            .map(|(car, pos)| CarSounds::new(&mut mixer, content, car, *pos))
            .collect();
        let level = LevelSounds::new(&mut mixer, content, sounds);
        let objects = objects
            .iter()
            .map(|kind| {
                let paths = [
                    kind.impact_sound.as_ref().map(|sound| &sound.file),
                    kind.motion_sounds.start.as_ref(),
                    kind.motion_sounds.turn.as_ref(),
                ];
                paths.map(|path| path.map(|path| mixer.load(path)))
            })
            .collect();
        Self {
            mixer,
            cars,
            level,
            objects,
        }
    }

    pub fn enabled(&self) -> bool {
        self.mixer.enabled()
    }

    /// Los autos (en el orden de `new`), los objetos, los emisores de la pista y el
    /// mezclador.
    pub fn update(
        &mut self,
        cars: &[VehicleSound],
        objects: &[ObjectSound],
        time_step: f32,
        listener: &Listener,
    ) {
        for (sounds, car) in self.cars.iter_mut().zip(cars) {
            sounds.update(&mut self.mixer, car, time_step, listener);
        }
        for event in objects {
            let sound = self
                .objects
                .get(event.kind)
                .and_then(|sounds| sounds[event.cue as usize]);
            if let Some(sound) = sound {
                self.mixer
                    .play_3d(sound, event.volume, SFX_SAMPLE_RATE, event.pos, listener);
            }
        }
        self.level.update(&mut self.mixer, time_step, listener);
        self.mixer.maintain(listener, time_step);
    }
}
