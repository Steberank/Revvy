//! `.inf` de pista y parámetros de auto (`parameters.txt` o `.inf`).
//! Las claves no se tiran: las desconocidas quedan en el mapa y se avisan.

use std::collections::BTreeMap;
use std::path::Path;

use crate::axes;
use crate::layout::StartSlot;
use crate::FormatError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CarStat {
    Engine,
    Grip,
    Mass,
    Steer,
}

#[derive(Clone, Debug)]
pub struct CarParams {
    pub name: String,
    pub models: BTreeMap<i32, String>,
    pub body_model: Option<i32>,
    pub wheel_models: Vec<i32>,
    pub stats: BTreeMap<CarStat, f32>,
    pub keys: BTreeMap<String, String>,
    pub unknown: Vec<String>,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct TrackInf {
    pub name: String,
    pub keys: BTreeMap<String, String>,
    pub start_grid: Vec<StartSlot>,
    /// `STARTPOS` tal cual (espacio de Re-Volt).
    pub start_pos: Option<[f32; 3]>,
    /// `STARTROT` en vueltas (0 – 1).
    pub start_rot: f32,
    /// `STARTGRID`: tipo de grilla de `CarGridStarts`. Si falta, 0.
    pub start_grid_type: i32,
    /// `FOGCOLOR`: el color de la niebla y del fondo. Si falta, negro, como en
    /// `LevelInfo.cpp`.
    pub fog_color: [u8; 3],
    /// `MODELRGBPER`: porcentaje del color de vértice de los modelos de objetos. Si falta, 100.
    pub model_rgb_per: u32,
}

pub fn parse_track(path: &Path) -> Result<TrackInf, FormatError> {
    let text = std::fs::read_to_string(path).map_err(|err| FormatError::io(path, err))?;
    let keys = scan_keys(&text);
    let name = keys.get("name").cloned().unwrap_or_else(|| {
        path.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    });
    let mut start_grid = Vec::new();
    let start_pos = keys.get("startpos").and_then(|pos| {
        let nums = numbers(pos);
        (nums.len() >= 3).then(|| [nums[0], nums[1], nums[2]])
    });
    let start_rot = keys
        .get("startrot")
        .and_then(|v| numbers(v).first().copied())
        .unwrap_or(0.0);
    let start_grid_type = keys
        .get("startgrid")
        .and_then(|v| numbers(v).first().copied())
        .map(|v| v as i32)
        .unwrap_or(0);
    if let Some(pos) = keys.get("startpos") {
        let nums = numbers(pos);
        if nums.len() >= 3 {
            let turns = keys
                .get("startrot")
                .and_then(|v| numbers(v).first().copied())
                .unwrap_or(0.0);
            start_grid = revolt_start_grid([nums[0], nums[1], nums[2]], turns, start_grid_type);
        }
    }
    let fog_color = keys
        .get("fogcolor")
        .map(|value| numbers(value))
        .filter(|nums| nums.len() >= 3)
        .map_or([0; 3], |nums| [0, 1, 2].map(|i| nums[i].clamp(0.0, 255.0) as u8));
    let model_rgb_per = keys
        .get("modelrgbper")
        .and_then(|value| numbers(value).first().copied())
        .map_or(100, |per| per.max(0.0) as u32);
    Ok(TrackInf {
        name,
        keys,
        start_grid,
        start_pos,
        start_rot,
        start_grid_type,
        fog_color,
        model_rgb_per,
    })
}

/// `CarGridStarts`: (x, y, z, rotoff) de cada puesto, en unidades de Re-Volt y relativo
/// a `STARTPOS` girado por `STARTROT`. El tipo 2 es del menú y tiene cuatro puestos.
const CAR_GRID_STARTS: [&[[f32; 4]]; 4] = [
    // Tipo 0: de a dos.
    &[
        [0.0, 0.0, 0.0, 0.0],
        [256.0, 0.0, -40.0, 0.0],
        [0.0, 0.0, -300.0, 0.0],
        [256.0, 0.0, -340.0, 0.0],
        [0.0, 0.0, -600.0, 0.0],
        [256.0, 0.0, -640.0, 0.0],
        [0.0, 0.0, -900.0, 0.0],
        [256.0, 0.0, -940.0, 0.0],
        [0.0, 0.0, -1200.0, 0.0],
        [256.0, 0.0, -1240.0, 0.0],
        [0.0, 0.0, -1500.0, 0.0],
        [256.0, 0.0, -1540.0, 0.0],
    ],
    // Tipo 1: de a dos, espejado. El puesto 7 dice -950 en el original.
    &[
        [0.0, 0.0, 0.0, 0.0],
        [-256.0, 0.0, -40.0, 0.0],
        [0.0, 0.0, -300.0, 0.0],
        [-256.0, 0.0, -340.0, 0.0],
        [0.0, 0.0, -600.0, 0.0],
        [-256.0, 0.0, -640.0, 0.0],
        [0.0, 0.0, -900.0, 0.0],
        [-256.0, 0.0, -950.0, 0.0],
        [0.0, 0.0, -1200.0, 0.0],
        [-256.0, 0.0, -1240.0, 0.0],
        [0.0, 0.0, -1500.0, 0.0],
        [-256.0, 0.0, -1540.0, 0.0],
    ],
    // Tipo 2: el del menú.
    &[
        [-1600.0, -250.0, -1100.0, 0.0],
        [-1700.0, -250.0, -1200.0, 0.0],
        [-1500.0, -250.0, -1000.0, 0.0],
        [1600.0, -200.0, 1200.0, -0.25],
    ],
    // Tipo 3: de a tres, para carreras de 12.
    &[
        [0.0, 0.0, 300.0, 0.0],
        [256.0, 0.0, 270.0, 0.0],
        [-256.0, 0.0, 240.0, 0.0],
        [-44.0, 0.0, 100.0, 0.0],
        [212.0, 0.0, 70.0, 0.0],
        [-300.0, 0.0, 40.0, 0.0],
        [44.0, 0.0, -100.0, 0.0],
        [300.0, 0.0, -130.0, 0.0],
        [-212.0, 0.0, -160.0, 0.0],
        [0.0, 0.0, -300.0, 0.0],
        [256.0, 0.0, -330.0, 0.0],
        [-256.0, 0.0, -360.0, 0.0],
    ],
];

/// `GetCarStartGrid` para todos los puestos, ya en el espacio de Revvy. El puesto se
/// gira con `RotMatrixY(-STARTROT)` y el auto mira con `RotMatrixY(-STARTROT - rotoff)`.
pub fn revolt_start_grid(start_pos: [f32; 3], turns: f32, grid_type: i32) -> Vec<StartSlot> {
    let table = CAR_GRID_STARTS[grid_type.clamp(0, CAR_GRID_STARTS.len() as i32 - 1) as usize];
    let base = axes::position(start_pos);
    let rotation = glam::Quat::from_rotation_y(axes::yaw_from_turns(turns));
    table
        .iter()
        .map(|&[x, y, z, rotoff]| StartSlot {
            pos: base + rotation * axes::position([x, y, z]),
            yaw: axes::yaw_from_turns(turns + rotoff),
        })
        .collect()
}

pub fn parse_car(text: &str) -> CarParams {
    parse_car_inner(text, true)
}

/// `CAR 0-28` de `CARINFO.TXT` rellena las claves que `parameters.txt` no trae.
/// Son datos de Re-Volt: completan autos de Re-Volt, nunca un auto propio de Revvy.
pub fn merge_stock_defaults(mut car: CarParams) -> CarParams {
    let defaults = parse_car_inner(stock_car_body(), false);
    for (key, value) in defaults.keys {
        car.keys.entry(key).or_insert(value);
    }
    car.stats = stats_from_keys(&car.keys);
    car
}

fn parse_car_inner(text: &str, warn_unknown: bool) -> CarParams {
    let mut keys = BTreeMap::new();
    let mut models = BTreeMap::new();
    let mut body_model = None;
    let mut wheel_models = Vec::new();
    let mut sections: Vec<String> = Vec::new();
    let mut unknown = Vec::new();
    let mut last_keys: Vec<String> = Vec::new();

    for raw in text.lines() {
        let line = strip_comment(rvgl_line(raw)).trim().to_string();
        if line.is_empty() {
            continue;
        }
        // `ReadMat` lee nueve números seguidos: `Inertia` sigue en las dos líneas de abajo.
        if starts_with_number(&line) && !last_keys.is_empty() {
            for key in &last_keys {
                if let Some(value) = keys.get_mut(key) {
                    let value: &mut String = value;
                    value.push(' ');
                    value.push_str(&line);
                }
            }
            continue;
        }
        if line.ends_with('{') {
            sections = section_targets(line.trim_end_matches('{').trim());
            last_keys.clear();
            continue;
        }
        if line == "}" {
            sections.clear();
            last_keys.clear();
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(key) = parts.next() else { continue };
        let key_l = key.to_ascii_lowercase();
        let rest = parts.collect::<Vec<_>>().join(" ");
        let rest = unquote(&rest);

        if key_l == "model" {
            let mut model_parts = rest.split_whitespace();
            if let Some(index) = model_parts.next().and_then(|n| n.parse().ok()) {
                let file = model_parts.collect::<Vec<_>>().join(" ");
                models.insert(index, unquote(&file));
            }
            last_keys.clear();
            continue;
        }

        let stored_keys = if sections.is_empty() {
            vec![key_l.clone()]
        } else {
            sections
                .iter()
                .map(|section| format!("{section}.{key_l}"))
                .collect()
        };
        if warn_unknown && !known_car_key(&key_l) {
            for stored in &stored_keys {
                unknown.push(stored.clone());
                tracing::warn!(key = %stored, "clave de auto desconocida");
            }
        }
        for section in &sections {
            if section.starts_with("body") && key_l == "modelnum" {
                body_model = rest.parse().ok();
            }
            if section.starts_with("wheel") && key_l == "modelnum" {
                if let Ok(index) = rest.parse::<i32>() {
                    if index >= 0 {
                        wheel_models.push(index);
                    }
                }
            }
        }
        for stored in &stored_keys {
            keys.insert(stored.clone(), rest.clone());
        }
        last_keys = stored_keys;
    }

    let stats = stats_from_keys(&keys);

    let name = keys.get("name").cloned().unwrap_or_default();
    CarParams {
        name,
        models,
        body_model,
        wheel_models,
        stats,
        keys,
        unknown,
    }
}

/// `CAR_INFO` de Re-Volt con las unidades del archivo (`TopSpeed` en mph, largos en
/// unidades de Re-Volt). `ReadInit` convierte `TopSpeed` al leer; acá lo hace la física.
#[derive(Clone, Debug, Default)]
pub struct CarInfo {
    pub name: String,
    /// 0 = eléctrico, 1 = glow (nafta), 2 = otro. Elige el sonido de motor por defecto.
    pub class: i32,
    pub top_end: f32,
    pub steer_rate: f32,
    pub steer_mod: f32,
    pub engine_rate: f32,
    pub top_speed_mph: f32,
    pub max_revs: f32,
    pub down_force_mod: f32,
    pub com: [f32; 3],
    pub weapon: [f32; 3],
    pub body: BodyInfo,
    pub wheels: [WheelInfo; 4],
    pub springs: [SpringInfo; 4],
    /// Ruta de `COLL` tal cual está en el archivo (`cars/<id>/hull.hul`).
    pub coll: Option<String>,
    /// Ruta de `TPAGE`.
    pub tpage: Option<String>,
    /// `SFXENGINE` de RVGL. `None` o `"NONE"` usa el motor por defecto de la clase.
    pub sfx_engine: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct BodyInfo {
    pub model_num: i32,
    pub offset: [f32; 3],
    pub mass: f32,
    /// Tres filas, como `ReadMat`.
    pub inertia: [[f32; 3]; 3],
    pub gravity: f32,
    pub hardness: f32,
    pub resistance: f32,
    pub ang_res: f32,
    pub res_mod: f32,
    pub grip: f32,
    pub static_friction: f32,
    pub kinetic_friction: f32,
}

#[derive(Clone, Debug, Default)]
pub struct WheelInfo {
    pub model_num: i32,
    pub offset1: [f32; 3],
    pub offset2: [f32; 3],
    pub is_present: bool,
    pub is_powered: bool,
    pub is_turnable: bool,
    pub steer_ratio: f32,
    pub engine_ratio: f32,
    pub radius: f32,
    pub mass: f32,
    pub gravity: f32,
    pub max_pos: f32,
    pub skid_width: f32,
    pub toe_in: f32,
    pub axle_friction: f32,
    pub grip: f32,
    pub static_friction: f32,
    pub kinetic_friction: f32,
}

#[derive(Clone, Debug, Default)]
pub struct SpringInfo {
    pub model_num: i32,
    pub offset: [f32; 3],
    pub length: f32,
    pub stiffness: f32,
    pub damping: f32,
    pub restitution: f32,
}

impl CarInfo {
    /// Arma el `CAR_INFO` a partir de las claves (ya mezcladas con `CAR 0-28`).
    pub fn from_params(params: &CarParams) -> Self {
        let keys = &params.keys;
        let real = |key: &str| keys.get(key).and_then(|v| first_number(v)).unwrap_or(0.0);
        let vec3 = |key: &str| {
            keys.get(key)
                .map(|v| numbers(v))
                .filter(|n| n.len() >= 3)
                .map(|n| [n[0], n[1], n[2]])
                .unwrap_or([0.0; 3])
        };
        let int = |key: &str| real(key) as i32;
        let boolean = |key: &str| keys.get(key).is_some_and(|v| parse_bool(v));
        let path = |key: &str| {
            keys.get(key)
                .map(|v| unquote(v))
                .filter(|v| !v.is_empty() && !v.eq_ignore_ascii_case("none"))
        };

        let inertia = keys
            .get("body.inertia")
            .map(|v| numbers(v))
            .filter(|n| n.len() >= 9)
            .map(|n| [[n[0], n[1], n[2]], [n[3], n[4], n[5]], [n[6], n[7], n[8]]])
            .unwrap_or([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);

        let wheel = |i: usize| {
            let k = |name: &str| format!("wheel {i}.{name}");
            WheelInfo {
                model_num: keys.get(&k("modelnum")).and_then(|v| first_number(v)).map_or(-1, |v| v as i32),
                offset1: vec3(&k("offset1")),
                offset2: vec3(&k("offset2")),
                is_present: boolean(&k("ispresent")),
                is_powered: boolean(&k("ispowered")),
                is_turnable: boolean(&k("isturnable")),
                steer_ratio: real(&k("steerratio")),
                engine_ratio: real(&k("engineratio")),
                radius: real(&k("radius")),
                mass: real(&k("mass")),
                gravity: real(&k("gravity")),
                max_pos: real(&k("maxpos")),
                skid_width: real(&k("skidwidth")),
                toe_in: real(&k("toein")),
                axle_friction: real(&k("axlefriction")),
                grip: real(&k("grip")),
                static_friction: real(&k("staticfriction")),
                kinetic_friction: real(&k("kineticfriction")),
            }
        };
        let spring = |i: usize| {
            let k = |name: &str| format!("spring {i}.{name}");
            SpringInfo {
                model_num: keys.get(&k("modelnum")).and_then(|v| first_number(v)).map_or(-1, |v| v as i32),
                offset: vec3(&k("offset")),
                length: real(&k("length")),
                stiffness: real(&k("stiffness")),
                damping: real(&k("damping")),
                restitution: real(&k("restitution")),
            }
        };

        CarInfo {
            name: params.name.clone(),
            class: int("class"),
            top_end: real("topend"),
            steer_rate: real("steerrate"),
            steer_mod: real("steermod"),
            engine_rate: real("enginerate"),
            top_speed_mph: real("topspeed"),
            max_revs: real("maxrevs"),
            down_force_mod: real("downforcemod"),
            com: vec3("com"),
            weapon: vec3("weapon"),
            body: BodyInfo {
                model_num: keys.get("body.modelnum").and_then(|v| first_number(v)).map_or(-1, |v| v as i32),
                offset: vec3("body.offset"),
                mass: real("body.mass"),
                inertia,
                gravity: real("body.gravity"),
                hardness: real("body.hardness"),
                resistance: real("body.resistance"),
                ang_res: real("body.angres"),
                res_mod: real("body.resmod"),
                grip: real("body.grip"),
                static_friction: real("body.staticfriction"),
                kinetic_friction: real("body.kineticfriction"),
            },
            wheels: [wheel(0), wheel(1), wheel(2), wheel(3)],
            springs: [spring(0), spring(1), spring(2), spring(3)],
            coll: path("coll"),
            tpage: path("tpage"),
            sfx_engine: path("sfxengine"),
        }
    }
}

fn parse_bool(value: &str) -> bool {
    let word = value
        .split(|c: char| c.is_whitespace() || c == ',')
        .find(|w| !w.is_empty())
        .unwrap_or("");
    matches!(word.to_ascii_lowercase().as_str(), "true" | "yes" | "1")
}

fn stats_from_keys(keys: &BTreeMap<String, String>) -> BTreeMap<CarStat, f32> {
    let mut stats = BTreeMap::new();
    if let Some(value) = keys.get("enginerate").and_then(|v| first_number(v)) {
        stats.insert(CarStat::Engine, value);
    }
    if let Some(value) = keys.get("body.mass").and_then(|v| first_number(v)) {
        stats.insert(CarStat::Mass, value);
    }
    if let Some(value) = keys.get("body.grip").and_then(|v| first_number(v)) {
        stats.insert(CarStat::Grip, value);
    }
    if let Some(value) = keys.get("steerrate").and_then(|v| first_number(v)) {
        stats.insert(CarStat::Steer, value);
    }
    stats
}

/// `WHEEL 0 - 3` se expande a `wheel 0` … `wheel 3`, igual que `ReadNumberList`.
fn section_targets(header: &str) -> Vec<String> {
    let header = header.to_ascii_lowercase();
    let mut parts = header.split_whitespace();
    let Some(kind) = parts.next() else {
        return Vec::new();
    };
    let rest: Vec<&str> = parts.collect();
    if rest.is_empty() {
        return vec![kind.to_string()];
    }
    let indices = expand_indices(&rest.join(" "));
    if indices.is_empty() {
        return vec![header];
    }
    indices
        .into_iter()
        .map(|index| format!("{kind} {index}"))
        .collect()
}

fn expand_indices(text: &str) -> Vec<i32> {
    let cleaned = text.replace(',', " ");
    let tokens: Vec<&str> = cleaned.split_whitespace().collect();
    let mut out = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let token = tokens[index];
        if token == "-" {
            index += 1;
            continue;
        }
        if let Some((start, end)) = token.split_once('-') {
            if let (Ok(start), Ok(end)) = (start.parse::<i32>(), end.parse::<i32>()) {
                push_range(&mut out, start, end);
                index += 1;
                continue;
            }
        }
        if let Ok(start) = token.parse::<i32>() {
            if index + 2 < tokens.len() && tokens[index + 1] == "-" {
                if let Ok(end) = tokens[index + 2].parse::<i32>() {
                    push_range(&mut out, start, end);
                    index += 3;
                    continue;
                }
            }
            out.push(start);
        }
        index += 1;
    }
    out
}

fn push_range(out: &mut Vec<i32>, start: i32, end: i32) {
    if start <= end {
        out.extend(start..=end);
    } else {
        out.extend(end..=start);
    }
}

/// El `CARINFO.TXT` de Re-Volt va versionado junto al crate: `rvsource/` no está en git.
fn stock_car_body() -> &'static str {
    extract_first_car_body(include_str!("../revolt/CARINFO.TXT"))
}

fn extract_first_car_body(text: &'static str) -> &'static str {
    // El primer bloque `CAR … { … }` es el default de todos los autos.
    let lower = text.to_ascii_lowercase();
    let mut search = 0;
    while let Some(rel) = lower[search..].find("car") {
        let at = search + rel;
        let line_start = lower[..at].rfind('\n').map(|pos| pos + 1).unwrap_or(0);
        let prefix = lower[line_start..at].trim();
        if !prefix.is_empty() {
            search = at + 3;
            continue;
        }
        let Some(brace) = text[at..].find('{') else {
            search = at + 3;
            continue;
        };
        let open = at + brace;
        let mut depth = 0;
        for (offset, ch) in text[open..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        let end = open + offset;
                        return &text[open + 1..end];
                    }
                }
                _ => {}
            }
        }
        break;
    }
    ""
}

fn known_car_key(key: &str) -> bool {
    matches!(
        key,
        "name"
            | "besttime"
            | "selectable"
            | "class"
            | "obtain"
            | "rating"
            | "topend"
            | "acc"
            | "weight"
            | "trans"
            | "maxrevs"
            | "model"
            | "coll"
            | "tpage"
            | "envrgb"
            | "steerrate"
            | "steermod"
            | "enginerate"
            | "topspeed"
            | "downforcemod"
            | "com"
            | "weapon"
            | "modelnum"
            | "offset"
            | "offset1"
            | "offset2"
            | "mass"
            | "inertia"
            | "gravity"
            | "hardness"
            | "resistance"
            | "angres"
            | "resmod"
            | "grip"
            | "staticfriction"
            | "kineticfriction"
            | "ispresent"
            | "ispowered"
            | "isturnable"
            | "steerratio"
            | "engineratio"
            | "radius"
            | "maxpos"
            | "skidwidth"
            | "toein"
            | "axlefriction"
            | "length"
            | "stiffness"
            | "damping"
            | "restitution"
            | "axis"
            | "angvel"
            | "secmodelnum"
            | "topmodelnum"
            | "direction"
            | "underthresh"
            | "underrange"
            | "underfront"
            | "underrear"
            | "undermax"
            | "overthresh"
            | "overrange"
            | "overmax"
            | "overaccthresh"
            | "overaccrange"
            | "pickupbias"
            | "blockbias"
            | "overtakebias"
            | "suspension"
            | "aggression"
            // Claves de RVGL (líneas `;)`).
            | "cpuselectable"
            | "statistics"
            | "tcarbox"
            | "tshadow"
            | "shadowindex"
            | "shadowtable"
            | "sfxengine"
            | "sfxservo"
            | "sfxhonk"
            | "flippable"
            | "flying"
            | "clothfx"
            | "hoodoffset"
            | "hoodlook"
            | "rearoffset"
            | "rearlook"
            | "fixedoffset"
            | "fixedlook"
            | "usedefault"
            | "camber"
            | "type"
            | "transvel"
            | "handling"
    )
}

fn scan_keys(text: &str) -> BTreeMap<String, String> {
    let mut keys = BTreeMap::new();
    for raw in text.lines() {
        let line = strip_comment(raw).trim().to_string();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(key) = parts.next() else { continue };
        let rest = unquote(&parts.collect::<Vec<_>>().join(" "));
        keys.insert(key.to_ascii_lowercase(), rest);
    }
    keys
}

/// RVGL marca con `;)` las claves que el Re-Volt original ignora (para él es un
/// comentario). Revvy lee como RVGL: la línea vale sin el prefijo.
fn rvgl_line(line: &str) -> &str {
    let trimmed = line.trim_start();
    trimmed.strip_prefix(";)").unwrap_or(line)
}

fn starts_with_number(line: &str) -> bool {
    line.chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit() || c == '-' || c == '+' || c == '.')
}

fn strip_comment(line: &str) -> &str {
    let mut quote = false;
    for (index, ch) in line.char_indices() {
        match ch {
            '\'' | '"' => quote = !quote,
            ';' if !quote => return &line[..index],
            _ => {}
        }
    }
    line
}

fn unquote(value: &str) -> String {
    value
        .trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .to_string()
}

fn numbers(value: &str) -> Vec<f32> {
    value
        .split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+'))
        .filter_map(|part| part.parse().ok())
        .collect()
}

fn first_number(value: &str) -> Option<f32> {
    numbers(value).into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_defaults_fill_a_missing_key_and_do_not_override() {
        let car = merge_stock_defaults(parse_car(
            "Name \"Parcial\"\nTopSpeed 12\nBODY {\nMASS 2.0f\n}\n",
        ));
        assert_eq!(first_number(car.keys.get("topspeed").unwrap()), Some(12.0));
        assert_eq!(first_number(car.keys.get("body.mass").unwrap()), Some(2.0));
        assert_eq!(
            car.keys.get("wheel 0.radius").map(String::as_str),
            Some("10")
        );
        assert_eq!(
            car.keys.get("wheel 3.radius").map(String::as_str),
            Some("10")
        );
        assert_eq!(
            first_number(car.keys.get("body.gravity").unwrap()),
            Some(2200.0)
        );
        assert_eq!(first_number(car.keys.get("steerrate").unwrap()), Some(2.5));
    }
}
