//! Sonidos de la pista: el banco y los emisores de `TrackSounds`, ya en metros.
//!
//! - `Loop`: suena siempre, con su rango.
//! - `Random`: suena cada 10–30 s (como `AI_3DSoundHandler`).
//! - `Sprinkler`: un chorro por cada vaivén del cabezal (como `AI_SprinklerHandler`).

use std::path::Path;

use glam::Vec3;
use revvy_formats::{EmitterKind, TrackSounds};

use super::mixer::{Handle3D, Listener, Mixer, SoundId, SFX_MAX_VOL, SFX_SAMPLE_RATE};

/// Espera máxima al azar antes de un sonido `Random` (`SOUND_3D_MAX_WAIT`).
const RANDOM_MAX_WAIT: f32 = 20.0;
/// Después de sonar, espera al menos esto.
const RANDOM_MIN_WAIT: f32 = 10.0;

enum Emitter {
    /// Lo mantiene el mezclador: acá no hay nada que hacer.
    Loop,
    Random {
        sound: SoundId,
        range: f32,
        pos: Vec3,
        timer: f32,
        playing: Option<Handle3D>,
    },
    Sprinkler {
        sound: SoundId,
        pos: Vec3,
        reach: f32,
        sine: f32,
        last_rot: f32,
        next_sfx: bool,
    },
}

pub struct LevelSounds {
    emitters: Vec<Emitter>,
    rng: u32,
}

/// Uniforme en [0, max). Xorshift en lugar del `rand()` de la CRT.
fn frand(rng: &mut u32, max: f32) -> f32 {
    *rng ^= *rng << 13;
    *rng ^= *rng >> 17;
    *rng ^= *rng << 5;
    (*rng as f32 / u32::MAX as f32) * max
}

impl LevelSounds {
    pub fn new(mixer: &mut Mixer, content: &Path, sounds: &TrackSounds) -> Self {
        let bank: Vec<Option<SoundId>> = match &sounds.bank {
            Some(bank) => {
                let folder = content.join("wavs").join(&bank.folder);
                let loaded: Vec<_> = bank
                    .files
                    .iter()
                    .map(|file| find_wav(&folder, file).map(|path| mixer.load(&path)))
                    .collect();
                tracing::info!(
                    banco = %bank.folder,
                    cargados = loaded.iter().filter(|s| s.is_some()).count(),
                    "sonidos de la pista"
                );
                loaded
            }
            None => {
                tracing::info!("la pista no tiene banco de sonidos");
                Vec::new()
            }
        };

        let mut rng = 0x1234_5678u32;
        let mut emitters = Vec::new();
        for emitter in &sounds.emitters {
            let Some(sound) = bank.get(emitter.slot).copied().flatten() else {
                continue;
            };
            match emitter.kind {
                EmitterKind::Loop => {
                    if let Some(handle) = mixer.create_3d(sound, SFX_MAX_VOL, SFX_SAMPLE_RATE, true, emitter.pos) {
                        mixer.set_range(handle, emitter.range);
                        emitters.push(Emitter::Loop);
                    }
                }
                EmitterKind::Random => emitters.push(Emitter::Random {
                    sound,
                    range: emitter.range,
                    pos: emitter.pos,
                    timer: frand(&mut rng, RANDOM_MAX_WAIT),
                    playing: None,
                }),
                EmitterKind::Sprinkler => emitters.push(Emitter::Sprinkler {
                    sound,
                    pos: emitter.pos,
                    reach: 1.0,
                    sine: 0.0,
                    last_rot: 0.0,
                    next_sfx: false,
                }),
            }
        }
        tracing::info!(emisores = emitters.len(), "emisores de sonido de la pista");
        Self { emitters, rng }
    }

    pub fn update(&mut self, mixer: &mut Mixer, time_step: f32, listener: &Listener) {
        for emitter in &mut self.emitters {
            match emitter {
                Emitter::Loop => {}
                Emitter::Random {
                    sound,
                    range,
                    pos,
                    timer,
                    playing,
                } => match playing {
                    None => {
                        *timer -= time_step;
                        if *timer < 0.0 {
                            *playing = mixer.create_3d(*sound, SFX_MAX_VOL, SFX_SAMPLE_RATE, false, *pos);
                            match *playing {
                                Some(handle) => mixer.set_range(handle, *range),
                                None => *timer = frand(&mut self.rng, RANDOM_MAX_WAIT) + RANDOM_MIN_WAIT,
                            }
                        }
                    }
                    Some(handle) => {
                        if !mixer.alive(*handle) {
                            *playing = None;
                            *timer = frand(&mut self.rng, RANDOM_MAX_WAIT) + RANDOM_MIN_WAIT;
                        }
                    }
                },
                Emitter::Sprinkler {
                    sound,
                    pos,
                    reach,
                    sine,
                    last_rot,
                    next_sfx,
                } => {
                    // Nadie pisa la manguera: el alcance queda al máximo.
                    *reach = (*reach + time_step * 0.5).min(1.0);
                    *sine += (time_step * 14.285) * ((1.5 - *reach) * 2.0);
                    let rot = sine.sin() * (0.04 * *reach);
                    if rot < *last_rot && *next_sfx {
                        *next_sfx = false;
                        let vol = (SFX_MAX_VOL as f32 * *reach) as i32;
                        mixer.play_3d(*sound, vol, SFX_SAMPLE_RATE, *pos, listener);
                    }
                    if rot >= *last_rot {
                        *next_sfx = true;
                    }
                    *last_rot = rot;
                }
            }
        }
    }
}

/// Los `.wav` de Re-Volt se piden sin distinguir mayúsculas (`Birds1.wav`).
fn find_wav(folder: &Path, stem: &str) -> Option<std::path::PathBuf> {
    let wanted = format!("{stem}.wav");
    let entries = std::fs::read_dir(folder).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case(&wanted))
        {
            return Some(path);
        }
    }
    tracing::warn!(archivo = %wanted, carpeta = %folder.display(), "falta un sonido de la pista");
    None
}
