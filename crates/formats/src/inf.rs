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
    if let Some(pos) = keys.get("startpos") {
        let nums = numbers(pos);
        if nums.len() >= 3 {
            let turns = keys
                .get("startrot")
                .and_then(|v| numbers(v).first().copied())
                .unwrap_or(0.0);
            start_grid.push(StartSlot {
                pos: axes::position([nums[0], nums[1], nums[2]]),
                yaw: axes::yaw_from_turns(turns),
            });
        }
    }
    Ok(TrackInf {
        name,
        keys,
        start_grid,
    })
}

pub fn parse_car(text: &str) -> CarParams {
    parse_car_inner(text, true)
}

/// `CAR 0-28` de `CARINFO.TXT` rellena las claves que `parameters.txt` no trae.
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

    for raw in text.lines() {
        let line = strip_comment(raw).trim().to_string();
        if line.is_empty() {
            continue;
        }
        if line.ends_with('{') {
            sections = section_targets(line.trim_end_matches('{').trim());
            continue;
        }
        if line == "}" {
            sections.clear();
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
        for stored in stored_keys {
            keys.insert(stored, rest.clone());
        }
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

fn stock_car_body() -> &'static str {
    extract_first_car_body(include_str!("../../../rvsource/CARINFO.TXT"))
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
