//! Contenido de Revvy: pistas y autos de Re-Volt (parsers + capa de traducción) y
//! nuevos (`.glb`, `car.toml`). Afuera salen los mismos tipos de Revvy para los dos.

use std::path::{Path, PathBuf};

mod axes;
mod binutil;
mod bmp;
mod cam;
mod fan;
mod fin;
mod fld;
mod fob;
mod glb;
mod hul;
mod inf;
mod mesh;
mod ncp;
mod pan;
mod prm;
mod revolt_car;
mod revolt_sounds;
mod revvy_car;
mod taz;
mod vis;
mod world;

pub mod gltf_track;
pub mod layout;
pub mod lit;
pub mod por;
pub mod pro;
pub mod sounds;
pub mod vehicle;

pub use axes::REVOLT_TO_METERS;
pub use bmp::load as load_bmp;
pub use fin::Instance as LegacyInstance;
pub use fob::{FobObject, SOUND_3D_TYPE, SPRINKLER_TYPE};
pub use hul::{load as load_hull, load_native, load_native_spheres, HullSphere, NativeHull};
pub use inf::{revolt_start_grid, BodyInfo, CarInfo, CarStat, SpringInfo, WheelInfo};
pub use layout::{SurfaceType, TrackLayout};
pub use mesh::VisualMesh;
pub use ncp::{CollisionTri, NcpFile, NcpGrid, NcpPoly};
pub use sounds::{EmitterKind, SoundBank, SoundEmitter, TrackSounds};
pub use vehicle::{CarSound, ChassisShape, EngineSound, SpringParams, VehicleParams, WheelParams, WHEEL_COUNT};

#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("{0}")]
    Io(std::io::Error),
    #[error("{path}: {message}")]
    Parse { path: String, message: String },
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
    pub sounds: TrackSounds,
    /// Datos de Re-Volt sin traducir. Solo los usa el port de referencia en los tests;
    /// el motor de Revvy nunca los lee. `None` en pistas glTF.
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
    /// El negro puro de las texturas no se dibuja (páginas de pista de Re-Volt).
    pub color_key: bool,
    /// Cielo en el orden de un cubemap: +X, −X, +Y, −Y, +Z, −Z.
    pub sky: Option<[image::RgbaImage; 6]>,
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

/// Un auto listo para el motor de Revvy, venga de Re-Volt o sea propio.
#[derive(Clone, Debug)]
pub struct CarDef {
    pub id: String,
    pub name: String,
    /// Carpeta del auto.
    pub dir: PathBuf,
    /// Chasis, en el espacio del modelo: se dibuja en el centro de masa + `body_offset`.
    pub body: Vec<VisualMesh>,
    /// Una lista de meshes por rueda (FL, FR, BL, BR), centradas en el buje.
    pub wheels: [Vec<VisualMesh>; 4],
    /// Textura del auto. Todas las caras con textura usan esta página.
    pub texture: Option<image::RgbaImage>,
    pub vehicle: VehicleParams,
    pub sound: CarSound,
    /// Solo autos de Re-Volt: los datos crudos que usa el port de referencia en los tests.
    pub revolt: Option<RevoltCar>,
}

/// `CAR_INFO` sin traducir. El motor de Revvy nunca lo lee.
#[derive(Clone, Debug)]
pub struct RevoltCar {
    pub info: CarInfo,
    pub params: inf::CarParams,
    /// Esferas del `.hul` en espacio de Re-Volt: `[x, y, z, radio]`.
    pub hull_spheres: Vec<[f32; 4]>,
}

impl CarDef {
    pub fn stat(&self, stat: inf::CarStat) -> Option<f32> {
        self.revolt.as_ref()?.params.stats.get(&stat).copied()
    }

    pub fn param(&self, key: &str) -> Option<&str> {
        self.revolt.as_ref()?.params.keys.get(key).map(String::as_str)
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
                return gltf_track::load(dir, options);
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

    let objects = match find_stem(dir, &stem, "fob") {
        Some(path) => fob::parse_objects(&path)?,
        None => Vec::new(),
    };
    let sounds = revolt_sounds::track_sounds(&id, &objects);

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
            color_key: true,
            sky: load_sky(dir),
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
            sounds,
            legacy,
        },
    })
}

/// `RenderSkybox` pega `sky_ft`, `sky_rt`, `sky_bk`, `sky_lt`, `sky_tp` y `sky_bt` en +Z,
/// −X, −Z, +X, arriba y abajo del archivo. Con el giro de ejes, el cubemap queda
/// +X `sky_rt`, −X `sky_lt`, +Y `sky_tp`, −Y `sky_bt`, +Z `sky_ft`, −Z `sky_bk`.
fn load_sky(level: &Path) -> Option<[image::RgbaImage; 6]> {
    let names = ["sky_rt", "sky_lt", "sky_tp", "sky_bt", "sky_ft", "sky_bk"];
    let mut faces = Vec::with_capacity(6);
    for name in names {
        faces.push(bmp::load(&find_file(level, &format!("{name}.bmp"))?).ok()?);
    }
    let faces: [image::RgbaImage; 6] = faces.try_into().ok()?;
    let (width, height) = (faces[0].width(), faces[0].height());
    let square = width == height && faces.iter().all(|face| face.width() == width && face.height() == height);
    square.then_some(faces)
}

pub fn parse_car_text(text: &str) -> inf::CarParams {
    inf::parse_car(text)
}

/// Carga un auto. Con `car.toml` es un auto propio de Revvy y nunca usa datos de
/// Re-Volt, aunque haya un `parameters.txt` al lado. Si no, es un auto de Re-Volt:
/// `parameters.txt` (o `.inf`) encima de los defaults de `CARINFO.TXT`, traducido.
pub fn load_car(dir: &Path) -> Result<CarDef, FormatError> {
    if let Some(toml_path) = find_file(dir, "car.toml") {
        return revvy_car::load(dir, &toml_path);
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
    let hull = match info.coll.as_deref().and_then(|coll| {
        let file_name = Path::new(coll).file_name()?.to_string_lossy().into_owned();
        find_file(dir, &file_name)
    }) {
        Some(path) => hul::load_native(&path)?,
        None => {
            tracing::warn!("auto sin .hul: el cuerpo no choca con el mundo");
            NativeHull::default()
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
        vehicle: revolt_car::vehicle_params(&info, &hull),
        sound: revolt_car::car_sound(&info),
        revolt: Some(RevoltCar {
            info,
            params,
            hull_spheres: hull.spheres,
        }),
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
