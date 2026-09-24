//! Sonidos de un auto: motor, roce, derrape, servo y golpes. Siguen las curvas de
//! `UpdateCarSfx` de Re-Volt PC (`OLD_AUDIO`) con el estado del vehículo de Revvy.

use std::path::{Path, PathBuf};

use revvy_formats::{CarDef, EngineSound};
use revvy_physics::surfaces;
use revvy_physics::VehicleSound;

use super::mixer::{Handle3D, Listener, Mixer, SoundId, SFX_MAX_VOL, SFX_SAMPLE_RATE};

/// Las curvas de Re-Volt cuentan la velocidad en sus unidades de 5 mm por segundo.
const SPEED_UNITS: f32 = 200.0;
/// `Revs / 9` de Re-Volt: 8 × la velocidad de rodadura (en sus unidades) sobre 9.
const REVS_PER_WHEEL_SPEED: f32 = 8.0 * SPEED_UNITS / 9.0;
/// Un golpe suena a partir de este cambio de velocidad (500 unidades de Re-Volt).
const BANG_MIN: f32 = 2.5;

pub struct CarSounds {
    engine_kind: EngineSound,
    engine: Option<Handle3D>,
    scrape_sound: SoundId,
    skid_normal: SoundId,
    skid_rough: SoundId,
    servo_sound: SoundId,
    hit_sound: SoundId,
    scrape: Option<Handle3D>,
    scrape_surface: Option<usize>,
    screech: Option<Handle3D>,
    skid_surface: Option<usize>,
    servo: Option<Handle3D>,
    servo_flag: f32,
}

/// Sample de motor: el propio del auto, o el de su tipo de motor.
pub fn engine_wav(content: &Path, car: &CarDef) -> PathBuf {
    if let Some(custom) = car.sound.sample.as_deref() {
        let custom = custom.replace('\\', "/");
        for candidate in [content.join(&custom), car.dir.join(&custom)] {
            if candidate.is_file() {
                return candidate;
            }
        }
        if let Some(name) = Path::new(&custom).file_name() {
            let in_car_dir = car.dir.join(name);
            if in_car_dir.is_file() {
                return in_car_dir;
            }
        }
        tracing::warn!(%custom, "sample de motor no encontrado; se usa el de su tipo");
    }
    let default = match car.sound.engine {
        EngineSound::Electric => "moto.wav",
        EngineSound::Petrol => "petrol.wav",
    };
    content.join("wavs").join(default)
}

impl CarSounds {
    /// El motor es un loop 3D que arranca mudo.
    pub fn new(mixer: &mut Mixer, content: &Path, car: &CarDef, pos: glam::Vec3) -> Self {
        let wavs = content.join("wavs");
        let engine_path = engine_wav(content, car);
        tracing::info!(auto = %car.name, motor = %engine_path.display(), "sonido de motor");
        let engine_sound = mixer.load(&engine_path);
        Self {
            engine_kind: car.sound.engine,
            engine: mixer.create_3d(engine_sound, 0, 0, true, pos),
            scrape_sound: mixer.load(&wavs.join("scrape.wav")),
            skid_normal: mixer.load(&wavs.join("skid_normal.wav")),
            skid_rough: mixer.load(&wavs.join("skid_rough.wav")),
            servo_sound: mixer.load(&wavs.join("servo.wav")),
            hit_sound: mixer.load(&wavs.join("hit2.wav")),
            scrape: None,
            scrape_surface: None,
            screech: None,
            skid_surface: None,
            servo: None,
            servo_flag: 0.0,
        }
    }

    /// Una vez por frame con el estado del auto.
    pub fn update(&mut self, mixer: &mut Mixer, car: &VehicleSound, time_step: f32, listener: &Listener) {
        let revs = ftol(car.wheel_speed.abs() * REVS_PER_WHEEL_SPEED);
        let vel = ftol(car.vel.length() * SPEED_UNITS);

        // Motor.
        if let Some(engine) = self.engine {
            match self.engine_kind {
                EngineSound::Electric => {
                    mixer.set_vol(engine, (revs / 20).min(SFX_MAX_VOL));
                    mixer.set_freq(engine, 10000 + revs * 8);
                }
                EngineSound::Petrol => {
                    mixer.set_vol(engine, SFX_MAX_VOL);
                    mixer.set_freq(engine, (7000 + revs * 15).min(70000));
                }
            }
            mixer.set_pos(engine, car.pos);
        }

        // El volumen de roce y derrape sube de a poco: como mucho `TimeStep × 600` por frame.
        let ramp = ftol(time_step * 600.0).max(1);

        // Roce del chasis o del costado de una rueda.
        match car.scrape {
            None => {
                if let Some(scrape) = self.scrape.take() {
                    mixer.free_3d(scrape);
                }
            }
            Some(surface) => {
                if self.scrape.is_none() {
                    self.scrape = mixer.create_3d(self.scrape_sound, 0, 0, true, car.pos);
                    self.scrape_surface = Some(surface.index());
                }
                if let Some(scrape) = self.scrape {
                    mixer.set_freq(scrape, 20000 + vel * 5);
                    let max_vol = (mixer.vol(scrape) + ramp).min(SFX_MAX_VOL);
                    mixer.set_vol(scrape, (vel / 10).min(max_vol));
                    mixer.set_pos(scrape, car.pos);
                    if self.scrape_surface != Some(surface.index()) {
                        mixer.change_sound(scrape, self.scrape_sound);
                        self.scrape_surface = Some(surface.index());
                    }
                }
            }
        }

        // Derrape: la superficie con más ruedas derrapando.
        match car.skid {
            None => {
                if let Some(screech) = self.screech.take() {
                    mixer.free_3d(screech);
                }
            }
            Some(surface) => {
                let sound = if surfaces::profile(surface).skid_rough {
                    self.skid_rough
                } else {
                    self.skid_normal
                };
                if self.screech.is_none() {
                    self.screech = mixer.create_3d(sound, 0, 0, true, car.pos);
                    self.skid_surface = Some(surface.index());
                }
                if let Some(screech) = self.screech {
                    mixer.set_freq(screech, 15000 + vel * 2);
                    let max_vol = (mixer.vol(screech) + ramp).min(SFX_MAX_VOL);
                    mixer.set_vol(screech, (vel / 10).min(max_vol));
                    mixer.set_pos(screech, car.pos);
                    if self.skid_surface != Some(surface.index()) {
                        mixer.change_sound(screech, sound);
                        self.skid_surface = Some(surface.index());
                    }
                }
            }
        }

        // Servo de la dirección mientras el volante se mueve.
        if (car.steer - car.last_steer).abs() > 0.01 {
            self.servo_flag = (self.servo_flag + time_step * 8.0).min(1.0);
        } else {
            self.servo_flag = (self.servo_flag - time_step * 8.0).max(0.0);
        }
        if self.servo_flag > 0.0 {
            if self.servo.is_none() {
                self.servo = mixer.create_3d(self.servo_sound, 0, 0, true, car.pos);
            }
            if let Some(servo) = self.servo {
                mixer.set_pos(servo, car.pos);
                mixer.set_vol(servo, (self.servo_flag * SFX_MAX_VOL as f32) as i32);
                let mut freq = (self.servo_flag * 10000.0) as i32 + SFX_SAMPLE_RATE;
                if car.steer < car.last_steer {
                    freq -= 5000;
                }
                mixer.set_freq(servo, freq);
            }
        } else if let Some(servo) = self.servo.take() {
            mixer.free_3d(servo);
        }

        // Golpe.
        if car.bang > BANG_MIN {
            let vol = ftol(car.bang * SPEED_UNITS / 10.0).min(SFX_MAX_VOL);
            mixer.play_3d(self.hit_sound, vol, SFX_SAMPLE_RATE, car.pos, listener);
        }
    }
}

fn ftol(value: f32) -> i32 {
    value.round() as i32
}
