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

pub use axes::REVOLT_TO_METERS;
pub use bmp::load as load_bmp;
pub use fin::Instance as LegacyInstance;
pub use fob::{FobObject, SOUND_3D_TYPE, SPRINKLER_TYPE};
pub use hul::{load as load_hull, load_native_spheres, HullSphere};
pub use inf::{BodyInfo, CarInfo, CarStat, SpringInfo, WheelInfo};
pub use layout::TrackLayout;
pub use mesh::VisualMesh;
pub use ncp::{NcpFile, NcpGrid, NcpPoly};

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
    /// Un auto propio de Revvy trae sus parámetros: nunca se completa con los de Re-Volt.
    #[error("auto de Revvy (car.toml): todavía no está implementado")]
    RevvyCarNotImplemented,
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
    /// Datos del legado en el espacio de Re-Volt (sin girar ejes ni escalar).
    /// Los consume la física portada de Re-Volt. `None` en pistas glTF.
    pub legacy: Option<LegacyLevel>,
}

/// Lo que el motor de Re-Volt lee de la carpeta del nivel, sin convertir.
#[derive(Clone, Debug)]
pub struct LegacyLevel {
    /// Nombre de la carpeta (`nhood1`). Elige el banco de sonidos del nivel.
    pub dir_name: String,
    pub start_pos: [f32; 3],
    /// `STARTROT` en vueltas.
    pub start_rot: f32,
    pub start_grid_type: i32,
    /// `.ncp` del mundo con su grilla.
    pub world: NcpFile,
    /// Instancias del `.fin` y el `.ncp` de su modelo (vacío si no choca).
    pub instances: Vec<(LegacyInstance, Vec<NcpPoly>)>,
    /// Objetos del `.fob` (sonidos 3D, regadores, rayitos…).
    pub objects: Vec<FobObject>,
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
    /// Carpeta del auto.
    pub dir: PathBuf,
    pub body: Vec<VisualMesh>,
    /// Una lista de meshes por rueda (FL, FR, BL, BR), del `ModelNum` de cada `WHEEL`.
    pub wheels: [Vec<VisualMesh>; 4],
    /// `TPAGE` del auto. Todas las caras con textura usan esta página.
    pub texture: Option<image::RgbaImage>,
    /// Esferas del `.hul` en espacio de Re-Volt: `[x, y, z, radio]`.
    pub hull_spheres: Vec<[f32; 4]>,
    pub info: CarInfo,
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
    layout.start_grid = track_inf.start_grid.clone();

    let legacy = if options.collision {
        let world_ncp = ncp::parse_native(&ncp_path)?;
        let instances = match find_stem(dir, &stem, "fin") {
            Some(path) => {
                let instances = fin::parse(&path)?;
                let polys = fin::native_collision(dir, &instances)?;
                instances.into_iter().zip(polys).collect()
            }
            None => Vec::new(),
        };
        let objects = match find_stem(dir, &stem, "fob") {
            Some(path) => fob::parse_objects(&path)?,
            None => Vec::new(),
        };
        Some(LegacyLevel {
            dir_name: id.clone(),
            start_pos: track_inf.start_pos.unwrap_or([0.0; 3]),
            start_rot: track_inf.start_rot,
            start_grid_type: track_inf.start_grid_type,
            world: world_ncp,
            instances,
            objects,
        })
    } else {
        None
    };

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
            legacy,
        },
    })
}

pub fn parse_car_text(text: &str) -> inf::CarParams {
    inf::parse_car(text)
}

/// Auto de Re-Volt: `parameters.txt` (o `.inf`) encima de los defaults de `CARINFO.TXT`.
/// Una carpeta con `car.toml` es un auto propio y no pasa por acá.
pub fn load_car(dir: &Path) -> Result<CarDef, FormatError> {
    if find_file(dir, "car.toml").is_some() {
        return Err(FormatError::RevvyCarNotImplemented);
    }
    let params_path = find_file(dir, "parameters.txt")
        .or_else(|| find_with_extension(dir, "inf"))
        .ok_or_else(|| FormatError::Missing("parameters.txt o .inf".into()))?;
    let text =
        std::fs::read_to_string(&params_path).map_err(|err| FormatError::io(&params_path, err))?;
    let params = inf::merge_stock_defaults(inf::parse_car(&text));
    let info = CarInfo::from_params(&params);
    let body_index = params
        .body_model
        .ok_or_else(|| FormatError::Missing("BODY.ModelNum".into()))?;
    let body_name = params
        .models
        .get(&body_index)
        .cloned()
        .ok_or_else(|| FormatError::Missing(format!("MODEL {body_index}")))?;
    let body = load_named_mesh(dir, &body_name)?;
    let mut wheels: [Vec<VisualMesh>; 4] = Default::default();
    for (slot, wheel) in info.wheels.iter().enumerate() {
        if !wheel.is_present || wheel.model_num < 0 {
            continue;
        }
        if let Some(name) = params.models.get(&wheel.model_num) {
            if !name.eq_ignore_ascii_case("none") {
                wheels[slot] = load_named_mesh(dir, name)?;
            }
        }
    }
    let texture = info.tpage.as_deref().and_then(|tpage| {
        let file_name = Path::new(tpage).file_name()?.to_string_lossy().into_owned();
        let path = find_file(dir, &file_name)?;
        match bmp::load(&path) {
            Ok(image) => Some(image),
            Err(err) => {
                tracing::warn!(%err, tpage, "textura de auto ilegible");
                None
            }
        }
    });
    let hull_spheres = match info.coll.as_deref().and_then(|coll| {
        let file_name = Path::new(coll).file_name()?.to_string_lossy().into_owned();
        find_file(dir, &file_name)
    }) {
        Some(path) => hul::load_native_spheres(&path)?,
        None => {
            tracing::warn!("auto sin .hul: el cuerpo no choca con el mundo");
            Vec::new()
        }
    };
    let id = dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    Ok(CarDef {
        id,
        name: params.name.clone(),
        dir: dir.to_path_buf(),
        body,
        wheels,
        texture,
        hull_spheres,
        info,
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
