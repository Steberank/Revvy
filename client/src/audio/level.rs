//! Sonidos del nivel: banco por nivel (`SfxLevel`) y los objetos del `.fob` que suenan.
//!
//! - `OBJECT_TYPE_3DSOUND` (49): `flags[0]` elige el sonido en `Sound3D`, `flags[1]`
//!   es el rango ×0.1 y `flags[2]` = 0 hace loop continuo; si no, suena cada tanto
//!   (`AI_3DSoundHandler`).
//! - `OBJECT_TYPE_SPRINKLER` (40): un chorro por vaivén del cabezal (`AI_SprinklerHandler`).
//!
//! La pelota de básquet y los conos (`basketball.wav`, `roadcone.wav`) suenan al chocar
//! objetos físicos que Revvy todavía no simula.

use std::path::Path;

use glam::Vec3;
use revvy_formats::{FobObject, LegacyLevel, SOUND_3D_TYPE, SPRINKLER_TYPE};
use revvy_physics::revolt::math::vec_mul_mat;
use revvy_physics::revolt::Mat;

use super::mixer::{Handle3D, Listener, Mixer, SoundId, SFX_MAX_VOL, SFX_SAMPLE_RATE};

/// Banco de cada nivel, en el orden de `sfx.cpp`. El índice es el slot `SFX_*`.
struct LevelSet {
    levels: &'static [&'static str],
    folder: &'static str,
    files: &'static [&'static str],
}

const LEVEL_SETS: &[LevelSet] = &[
    LevelSet {
        levels: &["toylite", "toy2"],
        folder: "toy",
        files: &["piano", "plane", "copter", "dragon", "creak", "train", "whistle", "arcade", "toybrick"],
    },
    LevelSet {
        levels: &["nhood1", "nhood2", "stunts", "nhood1_battle"],
        folder: "hood",
        files: &[
            "basketball",
            "birds1",
            "birds2",
            "birds3",
            "dogbark",
            "kids",
            "sprink",
            "tv",
            "lawnmower",
            "digger",
            "stream",
            "cityamb2",
            "roadcone",
        ],
    },
    LevelSet {
        levels: &["garden1", "bot_bat"],
        folder: "garden",
        files: &["tropics2", "tropics3", "tropics4", "stream", "animal1", "animal2", "animal3", "animal4"],
    },
    LevelSet {
        levels: &["muse1", "muse2", "muse_bat"],
        folder: "muse",
        files: &["museumam", "laserhum", "alarm2", "escalate", "rotating", "largdoor"],
    },
    LevelSet {
        levels: &["market1", "market2", "markar"],
        folder: "market",
        files: &["aircond1", "cabnhum2", "carpark", "freezer1", "iceyarea", "sdrsopen", "sdrsclos", "carton"],
    },
    LevelSet {
        levels: &["wild_west1", "wild_west2"],
        folder: "ghost",
        files: &["coyote1", "bats", "eagle1", "minedrip", "rattler", "townbell", "tumbweed"],
    },
    LevelSet {
        levels: &["ship1", "ship2"],
        folder: "ship",
        files: &["intamb1", "seagulls", "shiphorn", "strmrain", "thunder1", "wash"],
    },
];

/// `Sound3D` de `obj_init.cpp`: slot del banco del nivel para cada índice de `flags[0]`.
/// El slot es relativo al banco del nivel actual, igual que `SFX_GENERIC_NUM + n`.
const SOUND_3D_SLOTS: [usize; 40] = [
    1, 4, 5, 7, 8, 9, 2, 3, 7, 10, // hood birds1 … hood stream
    0, 1, 2, 0, 0, 1, 2, 3, 4, 3, // garden tropics, muse amb, market, muse escalator
    4, 5, 3, 0, 1, 2, 3, 4, 0, 1, // muse barrel/door, garden stream, ghost, ship
    2, 4, 3, 5, 4, 5, 6, 7, 11, 5, // ship, garden animals, hood amb, ghost bell
];
/// `SFX_HOOD_SPRINKLER`.
const SPRINKLER_SLOT: usize = 6;
/// `SOUND_3D_MAX_WAIT`.
const SOUND_3D_MAX_WAIT: f32 = 20.0;
/// `SprinklerHeadOffset`.
const SPRINKLER_HEAD_OFFSET: Vec3 = Vec3::new(0.0, -38.0, 0.0);

enum Emitter {
    /// Loop continuo: lo mantiene el mezclador, acá no hay nada que hacer.
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
        head_pos: Vec3,
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

/// `frand`: uniforme en [0, max). Xorshift en lugar del `rand()` de la CRT.
fn frand(rng: &mut u32, max: f32) -> f32 {
    *rng ^= *rng << 13;
    *rng ^= *rng >> 17;
    *rng ^= *rng << 5;
    (*rng as f32 / u32::MAX as f32) * max
}

impl LevelSounds {
    pub fn new(mixer: &mut Mixer, content: &Path, level: &LegacyLevel) -> Self {
        let name = level.dir_name.to_ascii_lowercase();
        let set = LEVEL_SETS.iter().find(|set| set.levels.contains(&name.as_str()));
        let mut bank: Vec<Option<SoundId>> = Vec::new();
        match set {
            Some(set) => {
                let folder = content.join("wavs").join(set.folder);
                for file in set.files {
                    bank.push(find_wav(&folder, file).map(|path| mixer.load(&path)));
                }
                tracing::info!(
                    banco = set.folder,
                    cargados = bank.iter().filter(|s| s.is_some()).count(),
                    "sonidos del nivel"
                );
            }
            None => tracing::info!(nivel = %level.dir_name, "el nivel no tiene banco de sonidos"),
        }
        let slot = |n: usize| bank.get(n).copied().flatten();

        let mut rng = 0x1234_5678u32;
        let mut emitters = Vec::new();
        for object in &level.objects {
            match object.id {
                SOUND_3D_TYPE => {
                    let Some(&slot_index) = usize::try_from(object.flags[0])
                        .ok()
                        .and_then(|i| SOUND_3D_SLOTS.get(i))
                    else {
                        continue;
                    };
                    let Some(sound) = slot(slot_index) else {
                        continue;
                    };
                    let range = object.flags[1] as f32 * 0.1;
                    let pos = Vec3::from(object.pos);
                    if object.flags[2] == 0 {
                        if let Some(handle) = mixer.create_3d(sound, SFX_MAX_VOL, SFX_SAMPLE_RATE, true, pos) {
                            mixer.set_range(handle, range);
                            emitters.push(Emitter::Loop);
                        }
                    } else {
                        emitters.push(Emitter::Random {
                            sound,
                            range,
                            pos,
                            timer: frand(&mut rng, SOUND_3D_MAX_WAIT),
                            playing: None,
                        });
                    }
                }
                SPRINKLER_TYPE => {
                    let Some(sound) = slot(SPRINKLER_SLOT) else {
                        continue;
                    };
                    let mat = object_matrix(object);
                    let head_pos = vec_mul_mat(SPRINKLER_HEAD_OFFSET, &mat) + Vec3::from(object.pos);
                    emitters.push(Emitter::Sprinkler {
                        sound,
                        head_pos,
                        reach: 1.0,
                        sine: 0.0,
                        last_rot: 0.0,
                        next_sfx: false,
                    });
                }
                _ => {}
            }
        }
        tracing::info!(emisores = emitters.len(), "objetos de sonido del nivel");
        Self { emitters, rng }
    }

    /// `AI_3DSoundHandler` y `AI_SprinklerHandler`, una vez por frame.
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
                                None => *timer = frand(&mut self.rng, SOUND_3D_MAX_WAIT) + 10.0,
                            }
                        }
                    }
                    Some(handle) => {
                        if !mixer.alive(*handle) {
                            *playing = None;
                            *timer = frand(&mut self.rng, SOUND_3D_MAX_WAIT) + 10.0;
                        }
                    }
                },
                Emitter::Sprinkler {
                    sound,
                    head_pos,
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
                        mixer.play_3d(*sound, vol, SFX_SAMPLE_RATE, *head_pos, listener);
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

/// Matriz de un objeto del `.fob`: `U` y `L` del archivo, `R = U × L`.
fn object_matrix(object: &FobObject) -> Mat {
    let u = Vec3::from(object.up);
    let l = Vec3::from(object.look);
    Mat { r: u.cross(l), u, l }
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
    tracing::warn!(archivo = %wanted, carpeta = %folder.display(), "falta un sonido del nivel");
    None
}
