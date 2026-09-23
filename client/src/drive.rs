//! Vista del mapa: la pista legacy y una cámara libre. Sin auto ni audio.

use std::path::PathBuf;

use glam::Vec3;
use revvy_formats::{load_track, TrackLoad, VisualMesh};

use crate::input::FlyKeys;
use crate::render::CameraView;

const MOVE_SPEED: f32 = 24.0;
const FAST_SPEED: f32 = 80.0;

pub struct MapView {
    track_meshes: Vec<VisualMesh>,
    track_textures: Vec<(i16, image::RgbaImage)>,
    sky: Option<[image::RgbaImage; 6]>,
    eye: Vec3,
    yaw: f32,
    pitch: f32,
}

impl MapView {
    pub fn load() -> anyhow::Result<Self> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../ServerREVOLT");
        let config = crate::config::ClientConfig::load()?;
        let level = cli_level(&config.level);
        let level_dir = resolve_content(&root.join("levels"), &level);
        tracing::info!(level = %level_dir.display(), "cargando pista legacy");
        let track = load_track(&level_dir, TrackLoad::default())?;
        let asset = &track.asset;
        let visual = asset.visual.as_ref();
        let track_meshes = visual.map(|v| v.meshes.clone()).unwrap_or_default();
        let track_textures = visual.map(|v| v.textures.clone()).unwrap_or_default();
        let spawn = asset
            .layout
            .start_grid
            .first()
            .map(|slot| slot.pos)
            .unwrap_or(Vec3::ZERO);
        let yaw = asset
            .layout
            .start_grid
            .first()
            .map(|slot| slot.yaw)
            .unwrap_or(0.0);
        tracing::info!(meshes = track_meshes.len(), "pista lista");
        Ok(Self {
            track_meshes,
            track_textures,
            sky: load_sky(&level_dir),
            eye: spawn + Vec3::Y * 3.0,
            yaw,
            pitch: -0.15,
        })
    }

    pub fn track_meshes(&self) -> &[VisualMesh] {
        &self.track_meshes
    }

    pub fn track_textures(&self) -> &[(i16, image::RgbaImage)] {
        &self.track_textures
    }

    pub fn sky(&self) -> Option<&[image::RgbaImage; 6]> {
        self.sky.as_ref()
    }

    pub fn step(&mut self, dt: f32, keys: FlyKeys) {
        self.yaw -= keys.look_dx * 0.003;
        self.pitch = (self.pitch - keys.look_dy * 0.003).clamp(-1.4, 1.4);
        let look = look_dir(self.yaw, self.pitch);
        let right = look.cross(Vec3::Y).normalize_or_zero();
        let mut wish = Vec3::ZERO;
        if keys.forward {
            wish += look;
        }
        if keys.back {
            wish -= look;
        }
        if keys.left {
            wish -= right;
        }
        if keys.right {
            wish += right;
        }
        if keys.up {
            wish += Vec3::Y;
        }
        if keys.down {
            wish -= Vec3::Y;
        }
        if wish.length_squared() > 0.0 {
            let speed = if keys.fast { FAST_SPEED } else { MOVE_SPEED };
            self.eye += wish.normalize() * speed * dt.clamp(0.0, 0.1);
        }
    }

    pub fn camera(&self) -> CameraView {
        CameraView {
            eye: self.eye,
            target: self.eye + look_dir(self.yaw, self.pitch),
        }
    }
}

fn look_dir(yaw: f32, pitch: f32) -> Vec3 {
    Vec3::new(
        yaw.sin() * pitch.cos(),
        pitch.sin(),
        yaw.cos() * pitch.cos(),
    )
}

fn cli_level(level: &str) -> String {
    if cfg!(test) {
        return level.to_string();
    }
    std::env::args()
        .skip(1)
        .find(|arg| !arg.starts_with('-'))
        .unwrap_or_else(|| level.to_string())
}

fn resolve_content(base: &std::path::Path, spec: &str) -> PathBuf {
    let path = PathBuf::from(spec);
    if path.is_absolute() || spec.contains('/') || spec.contains('\\') {
        path
    } else {
        base.join(spec)
    }
}

fn load_sky(level: &std::path::Path) -> Option<[image::RgbaImage; 6]> {
    // `RenderSkybox`: ft, rt, bk, lt, tp, bt sobre +Z, -X, -Z, +X, arriba, abajo
    // en el archivo. Con el cambio de ejes eso es +Z, +X, -Z, -X, +Y, -Y.
    let names = ["sky_rt", "sky_lt", "sky_tp", "sky_bt", "sky_ft", "sky_bk"];
    let mut faces = Vec::with_capacity(6);
    for name in names {
        let image = revvy_formats::load_bmp(&level.join(format!("{name}.bmp"))).ok()?;
        faces.push(image);
    }
    let faces: [image::RgbaImage; 6] = faces.try_into().ok()?;
    let (width, height) = (faces[0].width(), faces[0].height());
    if width != height
        || faces
            .iter()
            .any(|face| face.width() != width || face.height() != height)
    {
        return None;
    }
    Some(faces)
}
