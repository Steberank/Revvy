//! Parsers de pistas y autos legacy, y el `TrackAsset` que unifica con glTF.

use std::path::{Path, PathBuf};

mod axes;
mod binutil;
mod bmp;
mod cam;
mod fan;
mod fin;
mod fld;
mod fob;
mod hul;
mod inf;
mod mesh;
mod ncp;
mod pan;
mod prm;
mod taz;
mod vis;
mod world;

pub mod gltf_track;
pub mod layout;
pub mod lit;
pub mod por;
pub mod pro;

pub use bmp::load as load_bmp;
pub use hul::{load as load_hull, HullSphere};
pub use inf::CarStat;
pub use layout::TrackLayout;
pub use mesh::VisualMesh;

#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("{0}")]
    Io(std::io::Error),
    #[error("{path}: {message}")]
    Parse { path: String, message: String },
    #[error("formato glTF revvy-glb-v1 todavía no está implementado")]
    GlbNotImplemented,
    #[error("la carpeta mezcla un .glb con un mundo .w")]
    MixedFormat,
    #[error("falta {0}")]
    Missing(String),
}

impl FormatError {
    fn io(path: &Path, err: std::io::Error) -> Self {
        Self::Parse {
            path: path.display().to_string(),
            message: err.to_string(),
        }
    }

    fn parse(path: &Path, message: impl Into<String>) -> Self {
        Self::Parse {
            path: path.display().to_string(),
            message: message.into(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TrackLoad {
    pub visual: bool,
    pub collision: bool,
}

impl Default for TrackLoad {
    fn default() -> Self {
        Self {
            visual: true,
            collision: true,
        }
    }
}

impl TrackLoad {
    pub fn collision_only() -> Self {
        Self {
            visual: false,
            collision: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TrackAsset {
    pub visual: Option<Visual>,
    pub collision: Option<Collision>,
    pub layout: TrackLayout,
}

#[derive(Clone, Debug)]
pub struct Visual {
    pub meshes: Vec<VisualMesh>,
    pub textures: Vec<(i16, image::RgbaImage)>,
}

#[derive(Clone, Debug)]
pub struct Collision {
    pub triangles: Vec<ncp::CollisionTri>,
}

pub trait Track {
    fn id(&self) -> &str;
    fn asset(&self) -> &TrackAsset;
}

#[derive(Clone, Debug)]
pub struct LoadedTrack {
    pub id: String,
    pub asset: TrackAsset,
}

impl Track for LoadedTrack {
    fn id(&self) -> &str {
        &self.id
    }

    fn asset(&self) -> &TrackAsset {
        &self.asset
    }
}

#[derive(Clone, Debug)]
pub struct CarDef {
    pub id: String,
    pub name: String,
    pub body: Vec<VisualMesh>,
    pub wheels: Vec<VisualMesh>,
    pub params: inf::CarParams,
}

impl CarDef {
    pub fn stat(&self, stat: inf::CarStat) -> Option<f32> {
        self.params.stats.get(&stat).copied()
    }

    pub fn param(&self, key: &str) -> Option<&str> {
        self.params.keys.get(key).map(String::as_str)
    }
}

pub fn load_track(dir: &Path, options: TrackLoad) -> Result<LoadedTrack, FormatError> {
    let id = dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let has_w = find_with_extension(dir, "w").is_some();
    let has_glb = find_with_extension(dir, "glb").is_some();
    if has_w && has_glb {
        return Err(FormatError::MixedFormat);
    }
    if let Some(toml_path) = find_file(dir, "track.toml") {
        let text =
            std::fs::read_to_string(&toml_path).map_err(|err| FormatError::io(&toml_path, err))?;
        if let Ok(value) = toml::from_str::<toml::Value>(&text) {
            if value.get("format").and_then(|v| v.as_str()) == Some("revvy-glb-v1") {
                return gltf_track::load(dir).map(|_| unreachable!());
            }
        }
    }
    if !has_w {
        return Err(FormatError::Missing(format!("{id}.w")));
    }

    let stem = id.clone();
    let world_path =
        find_stem(dir, &stem, "w").ok_or_else(|| FormatError::Missing(format!("{stem}.w")))?;
    let ncp_path =
        find_stem(dir, &stem, "ncp").ok_or_else(|| FormatError::Missing(format!("{stem}.ncp")))?;
    let inf_path =
        find_stem(dir, &stem, "inf").ok_or_else(|| FormatError::Missing(format!("{stem}.inf")))?;

    let world = world::World::parse(&world_path)?;
    let collision = if options.collision {
        let mut triangles = ncp::parse(&ncp_path)?;
        if let Some(path) = find_stem(dir, &stem, "fin") {
            let instances = fin::parse(&path)?;
            let props = fin::bake_collision(dir, &instances)?;
            tracing::info!(props = props.len(), "colisión de instancias");
            triangles.extend(props);
        }
        Some(Collision { triangles })
    } else {
        None
    };

    let mut layout = TrackLayout::default();
    let track_inf = inf::parse_track(&inf_path)?;
    layout.start_grid = track_inf.start_grid;

    if let Some(path) = find_stem(dir, &stem, "taz") {
        layout.zones = taz::parse(&path)?;
    }
    if let Some(path) = find_stem(dir, &stem, "pan") {
        let pan = pan::parse(&path)?;
        layout.start_node = pan.start_node;
        layout.total_distance = pan.total_distance;
        layout.pos_nodes = pan.nodes;
    }
    if let Some(path) = find_stem(dir, &stem, "fan") {
        layout.ai_nodes = fan::parse(&path)?;
    }
    if let Some(path) = find_stem(dir, &stem, "fob") {
        layout.pickups = fob::parse_pickups(&path)?;
    }
    if let Some(path) = find_stem(dir, &stem, "fld") {
        layout.force_fields = fld::parse(&path)?;
    }
    if let Some(path) = find_stem(dir, &stem, "tri") {
        layout.kill_volumes = parse_kill_triggers(&path)?;
    }

    // Se leen para no abortar. El gameplay de v1 no los usa.
    if let Some(path) = find_stem(dir, &stem, "vis") {
        let _ = vis::parse(&path)?;
    }
    if let Some(path) = find_stem(dir, &stem, "cam") {
        let _ = cam::parse(&path)?;
    }
    if let Some(path) = find_stem(dir, &stem, "lit") {
        let _ = lit::parse(&path)?;
    }
    if let Some(path) = find_li_minus(dir, &stem) {
        let _ = lit::parse(&path)?;
    }
    if let Some(path) = find_stem(dir, &stem, "por") {
        let _ = por::parse(&path)?;
    }
    if let Some(path) = find_stem(dir, &stem, "pro") {
        let _ = pro::parse(&path)?;
    }

    let visual = if options.visual {
        let mut meshes = world.to_meshes()?;
        if let Some(path) = find_stem(dir, &stem, "fin") {
            let instances = fin::parse(&path)?;
            meshes.extend(fin::bake(dir, &instances)?);
        }
        Some(Visual {
            meshes,
            textures: bmp::load_pages(dir, &stem),
        })
    } else {
        None
    };

    Ok(LoadedTrack {
        id,
        asset: TrackAsset {
            visual,
            collision,
            layout,
        },
    })
}

pub fn parse_car_text(text: &str) -> inf::CarParams {
    inf::parse_car(text)
}

pub fn load_car(dir: &Path) -> Result<CarDef, FormatError> {
    let params_path = find_file(dir, "parameters.txt")
        .or_else(|| find_with_extension(dir, "inf"))
        .ok_or_else(|| FormatError::Missing("parameters.txt o .inf".into()))?;
    let text =
        std::fs::read_to_string(&params_path).map_err(|err| FormatError::io(&params_path, err))?;
    let params = inf::merge_stock_defaults(inf::parse_car(&text));
    let body_index = params
        .body_model
        .ok_or_else(|| FormatError::Missing("BODY.ModelNum".into()))?;
    let body_name = params
        .models
        .get(&body_index)
        .cloned()
        .ok_or_else(|| FormatError::Missing(format!("MODEL {body_index}")))?;
    let body = load_named_mesh(dir, &body_name)?;
    let mut wheels = Vec::new();
    for index in &params.wheel_models {
        if let Some(name) = params.models.get(index) {
            if !name.eq_ignore_ascii_case("none") {
                wheels.extend(load_named_mesh(dir, name)?);
            }
        }
    }
    let id = dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    Ok(CarDef {
        id,
        name: params.name.clone(),
        body,
        wheels,
        params,
    })
}

fn load_named_mesh(dir: &Path, model: &str) -> Result<Vec<VisualMesh>, FormatError> {
    let file_name = Path::new(model)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let Some(path) = find_file(dir, &file_name) else {
        tracing::warn!(model, "modelo de auto ausente");
        return Ok(Vec::new());
    };
    Prm::parse(&path)?.to_meshes(&file_name)
}

fn parse_kill_triggers(path: &Path) -> Result<Vec<layout::KillVolume>, FormatError> {
    let file = std::fs::File::open(path).map_err(|err| FormatError::io(path, err))?;
    let mut reader = binutil::Reader::new(std::io::BufReader::new(file));
    let count = reader.i32()?;
    if count < 0 {
        return Err(FormatError::parse(path, "cantidad de triggers negativa"));
    }
    let mut volumes = Vec::new();
    for _ in 0..count {
        let kind = reader.i32()?;
        let _flag = reader.i32()?;
        let center = axes::position(reader.v3()?);
        let mut rows = [[0.0; 3]; 3];
        for row in &mut rows {
            *row = reader.v3()?;
        }
        let half_extents = axes::position(reader.v3()?).abs();
        // En nhood1 el tipo 2 es el reposition que devuelve el auto a la pista.
        if kind == 2 {
            volumes.push(layout::KillVolume {
                center,
                rotation: axes::rotation(rows),
                half_extents,
            });
        }
    }
    Ok(volumes)
}

pub(crate) fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    let wanted = name.to_ascii_lowercase();
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.to_ascii_lowercase() == wanted)
        {
            return Some(path);
        }
    }
    None
}

fn find_stem(dir: &Path, stem: &str, ext: &str) -> Option<PathBuf> {
    find_file(dir, &format!("{stem}.{ext}"))
}

fn find_with_extension(dir: &Path, ext: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case(ext))
        {
            return Some(path);
        }
    }
    None
}

fn find_li_minus(dir: &Path, stem: &str) -> Option<PathBuf> {
    let wanted = format!("{stem}.li-").to_ascii_lowercase();
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.to_ascii_lowercase() == wanted)
        {
            return Some(path);
        }
    }
    None
}

use prm::Prm;
