//! `UpdateCarSfx` de `ai.cpp` (PC, `OLD_AUDIO`): motor, roce, derrape, servo y golpes.

use std::path::{Path, PathBuf};

use revvy_formats::CarDef;
use revvy_physics::revolt::material::SKID_ROUGH;
use revvy_physics::revolt::SfxState;

use super::mixer::{Handle3D, Listener, Mixer, SoundId, SFX_MAX_VOL, SFX_SAMPLE_RATE};

/// `CAR_CLASS_ELEC`: los eléctricos usan `moto.wav`; el resto, `petrol.wav`.
const CAR_CLASS_ELEC: i32 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Engine {
    Electric,
    Petrol,
}

pub struct CarSounds {
    engine_kind: Engine,
    engine: Option<Handle3D>,
    scrape_sound: SoundId,
    skid_normal: SoundId,
    skid_rough: SoundId,
    servo_sound: SoundId,
    hit_sound: SoundId,
    scrape: Option<Handle3D>,
    scrape_material: Option<usize>,
    screech: Option<Handle3D>,
    skid_material: Option<usize>,
    servo: Option<Handle3D>,
    servo_flag: f32,
}

/// Sonido de motor del auto: el `SFXENGINE` de RVGL si existe, o el de su clase.
pub fn engine_wav(content: &Path, car: &CarDef) -> PathBuf {
    if let Some(custom) = car.info.sfx_engine.as_deref() {
        let as_given = content.join(custom.replace('\\', "/"));
        if as_given.is_file() {
            return as_given;
        }
        if let Some(name) = Path::new(&custom.replace('\\', "/")).file_name() {
            let in_car_dir = car.dir.join(name);
            if in_car_dir.is_file() {
                return in_car_dir;
            }
        }
        tracing::warn!(custom, "SFXENGINE no encontrado; se usa el motor por defecto");
    }
    let default = if car.info.class == CAR_CLASS_ELEC {
        "moto.wav"
    } else {
        "petrol.wav"
    };
    content.join("wavs").join(default)
}

impl CarSounds {
    /// `AI_InitCarAI`: el motor es un loop 3D que arranca mudo.
    pub fn new(mixer: &mut Mixer, content: &Path, car: &CarDef, pos: glam::Vec3) -> Self {
        let wavs = content.join("wavs");
        let engine_path = engine_wav(content, car);
        tracing::info!(motor = %engine_path.display(), "sonido de motor");
        let engine_sound = mixer.load(&engine_path);
        // Un `SFXENGINE` propio cambia el sample, no la curva de volumen y tono.
        let engine_kind = if car.info.class == CAR_CLASS_ELEC {
            Engine::Electric
        } else {
            Engine::Petrol
        };
        Self {
            engine_kind,
            engine: mixer.create_3d(engine_sound, 0, 0, true, pos),
            scrape_sound: mixer.load(&wavs.join("scrape.wav")),
            skid_normal: mixer.load(&wavs.join("skid_normal.wav")),
            skid_rough: mixer.load(&wavs.join("skid_rough.wav")),
            servo_sound: mixer.load(&wavs.join("servo.wav")),
            hit_sound: mixer.load(&wavs.join("hit2.wav")),
            scrape: None,
            scrape_material: None,
            screech: None,
            skid_material: None,
            servo: None,
            servo_flag: 0.0,
        }
    }

    /// Una vez por frame con el estado del auto antes de mover (`TimeStep` entero).
    pub fn update(&mut self, mixer: &mut Mixer, car: &SfxState, time_step: f32, listener: &Listener) {
        let revs = ftol((car.revs / 9.0).abs());
        let vel = ftol(car.vel.length());

        // Motor.
        if let Some(engine) = self.engine {
            match self.engine_kind {
                Engine::Electric => {
                    mixer.set_vol(engine, (revs / 20).min(SFX_MAX_VOL));
                    mixer.set_freq(engine, 10000 + revs * 8);
                }
                Engine::Petrol => {
                    mixer.set_vol(engine, SFX_MAX_VOL);
                    mixer.set_freq(engine, (7000 + revs * 15).min(70000));
                }
            }
            mixer.set_pos(engine, car.pos);
        }

        // El volumen de roce y derrape sube de a poco: como mucho `TimeStep × 600` por frame.
        let ramp = ftol(time_step * 600.0).max(1);

        // Roce del cuerpo o del costado de una rueda.
        match car.scrape_material {
            None => {
                if let Some(scrape) = self.scrape.take() {
                    mixer.free_3d(scrape);
                }
            }
            Some(material) => {
                if self.scrape.is_none() {
                    self.scrape = mixer.create_3d(self.scrape_sound, 0, 0, true, car.pos);
                    self.scrape_material = Some(material);
                }
                if let Some(scrape) = self.scrape {
                    mixer.set_freq(scrape, 20000 + vel * 5);
                    let max_vol = (mixer.vol(scrape) + ramp).min(SFX_MAX_VOL);
                    mixer.set_vol(scrape, (vel / 10).min(max_vol));
                    mixer.set_pos(scrape, car.pos);
                    if self.scrape_material != Some(material) {
                        mixer.change_sound(scrape, self.scrape_sound);
                        self.scrape_material = Some(material);
                    }
                }
            }
        }

        // Derrape: el material con más ruedas derrapando.
        match car.skid_material {
            None => {
                if let Some(screech) = self.screech.take() {
                    mixer.free_3d(screech);
                }
            }
            Some(material) => {
                let sound = if SKID_ROUGH.get(material).copied().unwrap_or(false) {
                    self.skid_rough
                } else {
                    self.skid_normal
                };
                if self.screech.is_none() {
                    self.screech = mixer.create_3d(sound, 0, 0, true, car.pos);
                    self.skid_material = Some(material);
                }
                if let Some(screech) = self.screech {
                    mixer.set_freq(screech, 15000 + vel * 2);
                    let max_vol = (mixer.vol(screech) + ramp).min(SFX_MAX_VOL);
                    mixer.set_vol(screech, (vel / 10).min(max_vol));
                    mixer.set_pos(screech, car.pos);
                    if self.skid_material != Some(material) {
                        mixer.change_sound(screech, sound);
                        self.skid_material = Some(material);
                    }
                }
            }
        }

        // Servo de la dirección mientras el volante se mueve.
        if (car.steer_angle - car.last_steer_angle).abs() > 0.01 {
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
                if car.steer_angle < car.last_steer_angle {
                    freq -= 5000;
                }
                mixer.set_freq(servo, freq);
            }
        } else if let Some(servo) = self.servo.take() {
            mixer.free_3d(servo);
        }

        // Golpe (`UpdateCarMisc`).
        if car.bang_mag > 500.0 {
            let vol = ((car.bang_mag as i32) / 10).min(SFX_MAX_VOL);
            mixer.play_3d(self.hit_sound, vol, SFX_SAMPLE_RATE, car.pos, listener);
        }
    }
}

fn ftol(value: f32) -> i32 {
    value.round() as i32
}
