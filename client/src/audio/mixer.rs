//! `sfx.cpp` de Re-Volt PC (Miles Sound System) sobre Kira.
//!
//! Un `SAMPLE_3D` tiene volumen 0–127, frecuencia en Hz y posición en el espacio de
//! Re-Volt. Cada frame `MaintainAllSfx` lo atenúa por distancia a la cámara, lo panea
//! por su X en pantalla y le aplica Doppler. Los que hacen loop se apagan fuera de
//! rango y vuelven a sonar desde el principio al entrar.

use std::path::Path;

use glam::Vec3;
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
use kira::sound::PlaybackState;
use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Panning, Tween};
use revvy_physics::revolt::math::{mat_mul_vec, Mat};

/// `SFX_MAX_VOL`.
pub const SFX_MAX_VOL: i32 = 127;
/// `SFX_SAMPLE_RATE`: frecuencia base de los `.wav` de Re-Volt.
pub const SFX_SAMPLE_RATE: i32 = 22050;
const SFX_LEFT_PAN: f32 = 0.0;
const SFX_CENTRE_PAN: f32 = 64.0;
const SFX_RIGHT_PAN: f32 = 127.0;
const SFX_3D_PAN_MUL: f32 = 64.0;
const SFX_3D_MIN_DIST: f32 = 600.0;
const SFX_3D_SUB_DIST: f32 = 8.0 / SFX_MAX_VOL as f32;
const SFX_3D_SOS: f32 = 1024.0;
/// `RenderSettings.GeomPers` (`BaseGeomPers`) y `REAL_SCREEN_XSIZE`.
const GEOM_PERS: f32 = 512.0;
const REAL_SCREEN_XSIZE: f32 = 640.0;

/// La cámara que escucha, en el espacio de Re-Volt.
#[derive(Clone, Copy, Debug)]
pub struct Listener {
    pub pos: Vec3,
    pub mat: Mat,
    pub vel: Vec3,
}

pub type SoundId = usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handle3D(usize);

struct Sample3D {
    sound: SoundId,
    vol: i32,
    freq: i32,
    looping: bool,
    pos: Vec3,
    old_pos: Vec3,
    range_mul: f32,
    /// Un sonido sin loop recién creado arranca en el próximo `maintain`.
    pending: bool,
    playing: Option<StaticSoundHandle>,
}

pub struct Mixer {
    manager: Option<AudioManager<DefaultBackend>>,
    bank: Vec<Option<StaticSoundData>>,
    samples: Vec<Option<Sample3D>>,
    master_vol: i32,
}

impl Mixer {
    /// Abre el dispositivo. Sin audio el juego sigue: todas las llamadas son no-op.
    pub fn new(master_vol: i32) -> Self {
        let manager = match AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()) {
            Ok(manager) => Some(manager),
            Err(err) => {
                tracing::warn!(%err, "sin audio: no se pudo abrir el dispositivo");
                None
            }
        };
        Self {
            manager,
            bank: Vec::new(),
            samples: Vec::new(),
            master_vol: master_vol.clamp(0, SFX_MAX_VOL),
        }
    }

    pub fn enabled(&self) -> bool {
        self.manager.is_some()
    }

    /// Carga un `.wav` al banco. Si falta o no se decodifica, el id queda mudo.
    pub fn load(&mut self, path: &Path) -> SoundId {
        let data = match StaticSoundData::from_file(path) {
            Ok(data) => Some(data),
            Err(err) => {
                tracing::warn!(%err, path = %path.display(), "sonido no cargado");
                None
            }
        };
        self.bank.push(data);
        self.bank.len() - 1
    }

    pub fn loaded(&self, sound: SoundId) -> bool {
        self.bank.get(sound).is_some_and(Option::is_some)
    }

    /// `CreateSfx3D`.
    pub fn create_3d(&mut self, sound: SoundId, vol: i32, freq: i32, looping: bool, pos: Vec3) -> Option<Handle3D> {
        if !looping && !self.loaded(sound) {
            return None;
        }
        let sample = Sample3D {
            sound,
            vol,
            freq,
            looping,
            pos,
            old_pos: pos,
            range_mul: 1.0,
            pending: !looping,
            playing: None,
        };
        let slot = match self.samples.iter().position(Option::is_none) {
            Some(slot) => {
                self.samples[slot] = Some(sample);
                slot
            }
            None => {
                self.samples.push(Some(sample));
                self.samples.len() - 1
            }
        };
        Some(Handle3D(slot))
    }

    /// `FreeSfx3D`.
    pub fn free_3d(&mut self, handle: Handle3D) {
        if let Some(Some(mut sample)) = self.samples.get_mut(handle.0).map(Option::take) {
            if let Some(mut playing) = sample.playing.take() {
                playing.stop(Tween::default());
            }
        }
    }

    /// `false` cuando un sonido sin loop terminó y se liberó solo.
    pub fn alive(&self, handle: Handle3D) -> bool {
        self.samples.get(handle.0).is_some_and(Option::is_some)
    }

    pub fn set_vol(&mut self, handle: Handle3D, vol: i32) {
        if let Some(Some(sample)) = self.samples.get_mut(handle.0) {
            sample.vol = vol;
        }
    }

    pub fn vol(&self, handle: Handle3D) -> i32 {
        self.samples
            .get(handle.0)
            .and_then(Option::as_ref)
            .map_or(0, |sample| sample.vol)
    }

    pub fn set_freq(&mut self, handle: Handle3D, freq: i32) {
        if let Some(Some(sample)) = self.samples.get_mut(handle.0) {
            sample.freq = freq;
        }
    }

    pub fn set_pos(&mut self, handle: Handle3D, pos: Vec3) {
        if let Some(Some(sample)) = self.samples.get_mut(handle.0) {
            sample.pos = pos;
        }
    }

    /// `RangeMul`: multiplica la distancia a la que se deja de oír. 0 = ambiente global.
    pub fn set_range(&mut self, handle: Handle3D, range_mul: f32) {
        if let Some(Some(sample)) = self.samples.get_mut(handle.0) {
            sample.range_mul = range_mul;
        }
    }

    /// `ChangeSfxSample3D`: corta el sample actual; el loop vuelve a arrancar con el nuevo.
    pub fn change_sound(&mut self, handle: Handle3D, sound: SoundId) {
        if let Some(Some(sample)) = self.samples.get_mut(handle.0) {
            if let Some(mut playing) = sample.playing.take() {
                playing.stop(Tween::default());
            }
            sample.sound = sound;
        }
    }

    /// `PlaySfx3D`: sonido suelto, sin seguimiento.
    pub fn play_3d(&mut self, sound: SoundId, vol: i32, freq: i32, pos: Vec3, listener: &Listener) {
        let (vol, pan, freq) = settings_3d(vol, freq, pos, 0.0, 1.0, listener);
        // `PlaySfx`: sin volumen no se gasta un canal.
        if vol == 0 {
            return;
        }
        self.start(sound, vol, pan, freq, false);
    }

    fn start(&mut self, sound: SoundId, vol: i32, pan: i32, freq: i32, looping: bool) -> Option<StaticSoundHandle> {
        let master = self.master_vol;
        let manager = self.manager.as_mut()?;
        let data = self.bank.get(sound)?.as_ref()?;
        let mut data = data
            .volume(volume_db(vol, master))
            .panning(panning(pan))
            .playback_rate(playback_rate(freq, data.sample_rate));
        if looping {
            data = data.loop_region(..);
        }
        match manager.play(data) {
            Ok(handle) => Some(handle),
            Err(err) => {
                tracing::warn!(%err, "no se pudo reproducir un sonido");
                None
            }
        }
    }

    /// `MaintainAllSfx`: una vez por frame, con la cámara actual.
    pub fn maintain(&mut self, listener: &Listener, time_step: f32) {
        let master = self.master_vol;
        for slot in 0..self.samples.len() {
            let Some(sample) = self.samples[slot].as_mut() else {
                continue;
            };

            // Un sonido sin loop que terminó se libera.
            if !sample.looping && !sample.pending {
                let done = sample
                    .playing
                    .as_ref()
                    .is_none_or(|playing| playing.state() == PlaybackState::Stopped);
                if done {
                    self.samples[slot] = None;
                    continue;
                }
            }

            let vel = if time_step > 1e-5 {
                (sample.pos - sample.old_pos) / time_step
            } else {
                Vec3::ZERO
            };
            sample.old_pos = sample.pos;
            let rel = vel - listener.vel;
            let to_sample = sample.pos - listener.pos;
            let dist = to_sample.length();
            let doppler = if dist > 1e-5 {
                rel.dot(to_sample / dist) * 0.005
            } else {
                0.0
            };
            let (vol, pan, freq) = settings_3d(sample.vol, sample.freq, sample.pos, doppler, sample.range_mul, listener);

            if sample.looping && vol == 0 {
                if let Some(mut playing) = sample.playing.take() {
                    playing.stop(Tween::default());
                }
                continue;
            }

            if sample.looping && sample.playing.is_none() || sample.pending {
                sample.pending = false;
                let (sound, looping) = (sample.sound, sample.looping);
                let started = self.start(sound, vol, pan, freq, looping);
                if let Some(sample) = self.samples[slot].as_mut() {
                    sample.playing = started;
                }
                continue;
            }

            if let Some(playing) = sample.playing.as_mut() {
                let rate = self
                    .bank
                    .get(sample.sound)
                    .and_then(Option::as_ref)
                    .map_or(SFX_SAMPLE_RATE as u32, |data| data.sample_rate);
                playing.set_volume(volume_db(vol, master), Tween::default());
                playing.set_panning(panning(pan), Tween::default());
                playing.set_playback_rate(playback_rate(freq, rate), Tween::default());
            }
        }
    }
}

/// `GetSfxSettings3D` (PC 1.2): volumen por distancia, paneo por la X en pantalla y
/// Doppler por la velocidad relativa. Devuelve (vol, pan, freq) en unidades de Miles.
pub fn settings_3d(vol: i32, freq: i32, pos: Vec3, vel: f32, range_mul: f32, listener: &Listener) -> (i32, i32, i32) {
    if range_mul == 0.0 {
        return (vol, SFX_CENTRE_PAN as i32, freq);
    }
    let cam_space = mat_mul_vec(&listener.mat, pos - listener.pos);
    let len = cam_space.length();

    let f = ((SFX_3D_MIN_DIST * range_mul) / len - SFX_3D_SUB_DIST).clamp(0.0, 1.0);
    let per = ftol(f * 256.0);
    let vol = vol * per / 256;

    let near = (len / 100.0).min(1.0);
    let x = if cam_space.z.abs() > 1e-5 {
        cam_space.x * GEOM_PERS / cam_space.z.abs() * near
    } else if cam_space.x == 0.0 {
        0.0
    } else {
        cam_space.x.signum() * f32::MAX
    };
    let x = (x * SFX_3D_PAN_MUL / REAL_SCREEN_XSIZE + SFX_CENTRE_PAN).clamp(SFX_LEFT_PAN + 1.0, SFX_RIGHT_PAN - 1.0);
    let pan = ftol(x);

    let f = SFX_3D_SOS / (vel + SFX_3D_SOS);
    let per = ftol(f * 256.0);
    let freq = freq * per / 256;
    (vol, pan, freq)
}

/// `FTOL`: el truco de la FPU redondea al más cercano.
fn ftol(value: f32) -> i32 {
    value.round() as i32
}

/// `AIL_set_sample_volume(vol · master / 127)`: lineal en 0–127.
fn volume_db(vol: i32, master: i32) -> Decibels {
    let amplitude = (vol.clamp(0, SFX_MAX_VOL) * master / SFX_MAX_VOL) as f32 / SFX_MAX_VOL as f32;
    if amplitude <= 0.0 {
        Decibels::SILENCE
    } else {
        Decibels(20.0 * amplitude.log10())
    }
}

/// Paneo de Miles 0–127 (64 al centro) a −1…1.
fn panning(pan: i32) -> Panning {
    Panning(((pan as f32 - SFX_CENTRE_PAN) / 63.0).clamp(-1.0, 1.0))
}

/// `AIL_set_sample_playback_rate(freq)` es la frecuencia en Hz; Kira pide la relación.
fn playback_rate(freq: i32, sample_rate: u32) -> f64 {
    (freq.max(1) as f64 / sample_rate.max(1) as f64).clamp(0.05, 8.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use revvy_physics::revolt::math::IDENTITY;

    fn listener() -> Listener {
        Listener {
            pos: Vec3::ZERO,
            mat: IDENTITY,
            vel: Vec3::ZERO,
        }
    }

    #[test]
    fn close_sounds_play_at_full_volume_and_far_ones_fade_out() {
        let (vol, pan, freq) = settings_3d(127, 22050, Vec3::new(0.0, 0.0, 300.0), 0.0, 1.0, &listener());
        assert_eq!(vol, 127);
        assert_eq!(pan, 64);
        assert_eq!(freq, 22050);
        let (vol, _, _) = settings_3d(127, 22050, Vec3::new(0.0, 0.0, 20000.0), 0.0, 1.0, &listener());
        assert_eq!(vol, 0);
        let (vol, _, _) = settings_3d(127, 22050, Vec3::new(0.0, 0.0, 20000.0), 0.0, 0.0, &listener());
        assert_eq!(vol, 127, "rango 0 es ambiente global");
    }

    #[test]
    fn right_of_the_camera_pans_right() {
        let (_, pan, _) = settings_3d(127, 22050, Vec3::new(500.0, 0.0, 500.0), 0.0, 1.0, &listener());
        assert!(pan > 64, "pan {pan}");
    }

    #[test]
    fn approaching_sounds_go_up_in_pitch() {
        let (_, _, freq) = settings_3d(127, 22050, Vec3::new(0.0, 0.0, 300.0), -300.0, 1.0, &listener());
        assert!(freq > 22050, "freq {freq}");
    }

    #[test]
    fn volume_is_linear_in_miles_units() {
        assert!((volume_db(127, 127).0).abs() < 1e-4);
        assert_eq!(volume_db(0, 127), Decibels::SILENCE);
    }
}
