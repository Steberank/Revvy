//! Sonidos de una pista para el motor de sonido de Revvy: el banco de samples y los
//! emisores, en metros. Una pista de Re-Volt llega acá por `revolt_sounds`.

use glam::Vec3;

#[derive(Clone, Debug, Default)]
pub struct TrackSounds {
    pub bank: Option<SoundBank>,
    pub emitters: Vec<SoundEmitter>,
}

/// Samples de la pista. `files[slot]` es el nombre sin extensión dentro de `wavs/<folder>/`.
#[derive(Clone, Debug)]
pub struct SoundBank {
    pub folder: String,
    pub files: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct SoundEmitter {
    /// Índice en `SoundBank::files`.
    pub slot: usize,
    pub pos: Vec3,
    /// Multiplicador de la distancia a la que el sonido se deja de oír.
    pub range: f32,
    pub kind: EmitterKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EmitterKind {
    /// Suena siempre.
    Loop,
    /// Suena cada tanto, entre 10 y 30 segundos.
    Random,
    /// Un chorro de regador por cada vaivén del cabezal.
    Sprinkler,
}
