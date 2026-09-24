//! Sonido del juego: el modelo de `sfx.cpp` de Re-Volt PC sobre Kira.
//!
//! El auto (motor, derrape, roce, servo, golpes) y los objetos que suenan en el nivel
//! se actualizan una vez por frame; después el mezclador recalcula volumen, paneo y
//! Doppler de cada sonido 3D respecto de la cámara.

mod car;
mod level;
mod mixer;

use std::path::Path;

use revvy_formats::{CarDef, LegacyLevel};
use revvy_physics::revolt::SfxState;

pub use mixer::Listener;

use car::CarSounds;
use level::LevelSounds;
use mixer::Mixer;

pub struct Audio {
    mixer: Mixer,
    car: CarSounds,
    level: LevelSounds,
}

impl Audio {
    pub fn new(content: &Path, level: &LegacyLevel, car: &CarDef, car_pos: glam::Vec3, master_vol: i32) -> Self {
        let mut mixer = Mixer::new(master_vol);
        let car = CarSounds::new(&mut mixer, content, car, car_pos);
        let level = LevelSounds::new(&mut mixer, content, level);
        Self { mixer, car, level }
    }

    pub fn enabled(&self) -> bool {
        self.mixer.enabled()
    }

    /// `UpdateCarSfx` + objetos del nivel + `MaintainAllSfx`.
    pub fn update(&mut self, car: &SfxState, time_step: f32, listener: &Listener) {
        self.car.update(&mut self.mixer, car, time_step, listener);
        self.level.update(&mut self.mixer, time_step, listener);
        self.mixer.maintain(listener, time_step);
    }
}
