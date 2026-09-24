//! Capa de traducción de sonidos de nivel de Re-Volt: el banco de cada nivel
//! (`SfxLevel`) y los objetos del `.fob` que suenan, como emisores de Revvy.

use glam::Vec3;

use crate::axes;
use crate::fob::{FobObject, SOUND_3D_TYPE, SPRINKLER_TYPE};
use crate::sounds::{EmitterKind, SoundBank, SoundEmitter, TrackSounds};

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
const SOUND_3D_SLOTS: [usize; 40] = [
    1, 4, 5, 7, 8, 9, 2, 3, 7, 10, // hood birds1 … hood stream
    0, 1, 2, 0, 0, 1, 2, 3, 4, 3, // garden tropics, muse amb, market, muse escalator
    4, 5, 3, 0, 1, 2, 3, 4, 0, 1, // muse barrel/door, garden stream, ghost, ship
    2, 4, 3, 5, 4, 5, 6, 7, 11, 5, // ship, garden animals, hood amb, ghost bell
];
/// `SFX_HOOD_SPRINKLER`.
const SPRINKLER_SLOT: usize = 6;
/// `SprinklerHeadOffset`, en unidades de Re-Volt sobre el eje up del objeto.
const SPRINKLER_HEAD_OFFSET: f32 = -38.0;

/// Banco y emisores de un nivel de Re-Volt. `level` es el nombre de la carpeta.
pub fn track_sounds(level: &str, objects: &[FobObject]) -> TrackSounds {
    let name = level.to_ascii_lowercase();
    let Some(set) = LEVEL_SETS.iter().find(|set| set.levels.contains(&name.as_str())) else {
        return TrackSounds::default();
    };
    let bank = SoundBank {
        folder: set.folder.to_string(),
        files: set.files.iter().map(|file| file.to_string()).collect(),
    };
    let mut emitters = Vec::new();
    for object in objects {
        match object.id {
            SOUND_3D_TYPE => {
                let Some(&slot) = usize::try_from(object.flags[0])
                    .ok()
                    .and_then(|i| SOUND_3D_SLOTS.get(i))
                else {
                    continue;
                };
                emitters.push(SoundEmitter {
                    slot,
                    pos: axes::position(object.pos),
                    range: object.flags[1] as f32 * 0.1,
                    kind: if object.flags[2] == 0 {
                        EmitterKind::Loop
                    } else {
                        EmitterKind::Random
                    },
                });
            }
            SPRINKLER_TYPE => {
                // El cabezal está sobre el eje up del objeto.
                let up = Vec3::from(object.up);
                let head = Vec3::from(object.pos) + up * SPRINKLER_HEAD_OFFSET;
                emitters.push(SoundEmitter {
                    slot: SPRINKLER_SLOT,
                    pos: axes::position(head.to_array()),
                    range: 1.0,
                    kind: EmitterKind::Sprinkler,
                });
            }
            _ => {}
        }
    }
    TrackSounds {
        bank: Some(bank),
        emitters,
    }
}
