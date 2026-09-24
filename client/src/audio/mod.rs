//! Motor de sonido de Revvy: Kira como salida y un modelo 3D que reproduce el de
//! `sfx.cpp` de Re-Volt PC, en metros.
//!
//! Cada auto (motor, derrape, roce, servo, golpes) y los emisores de la pista se
//! actualizan una vez por frame; después el mezclador recalcula volumen, paneo y
//! Doppler de cada sonido 3D respecto de la cámara.

mod car;
mod level;
mod mixer;

use std::path::Path;

use revvy_formats::{CarDef, TrackSounds};
use revvy_physics::VehicleSound;

pub use mixer::Listener;

use car::CarSounds;
use level::LevelSounds;
use mixer::Mixer;

pub struct Audio {
    mixer: Mixer,
    cars: Vec<CarSounds>,
    level: LevelSounds,
}

impl Audio {
    /// `cars` va con la posición de largada de cada auto.
    pub fn new(content: &Path, sounds: &TrackSounds, cars: &[(&CarDef, glam::Vec3)], master_vol: i32) -> Self {
        let mut mixer = Mixer::new(master_vol);
        let cars = cars
            .iter()
            .map(|(car, pos)| CarSounds::new(&mut mixer, content, car, *pos))
            .collect();
        let level = LevelSounds::new(&mut mixer, content, sounds);
        Self { mixer, cars, level }
    }

    pub fn enabled(&self) -> bool {
        self.mixer.enabled()
    }

    /// Los autos (en el orden de `new`), los emisores de la pista y el mezclador.
    pub fn update(&mut self, cars: &[VehicleSound], time_step: f32, listener: &Listener) {
        for (sounds, car) in self.cars.iter_mut().zip(cars) {
            sounds.update(&mut self.mixer, car, time_step, listener);
        }
        self.level.update(&mut self.mixer, time_step, listener);
        self.mixer.maintain(listener, time_step);
    }
}
