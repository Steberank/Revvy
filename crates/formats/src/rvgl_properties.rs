//! `properties.txt` de las pistas de RVGL: lo que una pista redefine. De acá se usan los
//! materiales (`MATERIAL`) y los baches (`CORRUGATION`), que cambian cómo agarra y rebota
//! cada superficie. Polvo, chispas, estelas, viento, gravedad y pickups se leen y todavía
//! no se usan.
//!
//! Cada sección es `NOMBRE { Clave valores ; comentario }`, con `ID` diciendo qué material
//! (0 a 26) o qué tipo de bache (0 a 7) reemplaza. Una clave que falta deja el valor de
//! Re-Volt (`COL_MaterialInfo`, `COL_CorrugationInfo`).

use std::collections::BTreeMap;
use std::path::Path;

use glam::Vec3;

use crate::axes;
use crate::layout::{SurfaceTuning, SurfaceType};

/// `COL_CorrugationInfo`: amplitud y largos de onda en X y Z (unidades).
const CORRUGATIONS: [[f32; 3]; 8] = [
    [0.0, 0.0, 0.0],
    [3.0, 70.0, 70.0],
    [1.0, 40.0, 40.0],
    [1.0, 40.0, 40.0],
    [1.0, 80.0, 80.0],
    [1.0, 80.0, 80.0],
    [1.0, 80.0, 80.0],
    [1.0, 80.0, 80.0],
];

/// El tipo de bache de cada material en `COL_MaterialInfo`, si tiene `MATERIAL_CORRUGATED`
/// o se mueve: pasto, metal con relieve, piedritas, grava, las cintas y las tres tierras.
const STOCK_CORRUGATION: [Option<usize>; 27] = [
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some(3),
    Some(3),
    Some(1),
    Some(2),
    Some(3),
    Some(3),
    Some(5),
    Some(6),
    Some(7),
    None,
    None,
    None,
    Some(3),
    Some(3),
    None,
];

/// Una sección: su nombre y cada clave con sus valores.
struct Section {
    name: String,
    keys: BTreeMap<String, Vec<String>>,
}

impl Section {
    fn number(&self, key: &str) -> Option<f32> {
        self.keys.get(key)?.first()?.parse().ok()
    }

    fn numbers(&self, key: &str) -> Option<Vec<f32>> {
        self.keys
            .get(key)?
            .iter()
            .map(|value| value.parse().ok())
            .collect()
    }

    fn flag(&self, key: &str) -> Option<bool> {
        match self.keys.get(key)?.first()?.to_ascii_lowercase().as_str() {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        }
    }
}

/// Las superficies que redefine `properties.txt` de la carpeta del nivel. Sin archivo, o
/// si no redefine materiales ni baches, ninguna.
pub fn surface_tuning(level: &Path) -> Vec<SurfaceTuning> {
    let Some(path) = crate::find_file(level, "properties.txt") else {
        return Vec::new();
    };
    let text = match std::fs::read(&path) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(err) => {
            tracing::warn!(%err, archivo = %path.display(), "properties.txt ilegible");
            return Vec::new();
        }
    };
    let tuning = translate(&parse(&text));
    if !tuning.is_empty() {
        let surfaces: Vec<&str> = tuning.iter().map(|tune| tune.surface.name()).collect();
        tracing::info!(superficies = ?surfaces, "la pista redefine superficies (properties.txt)");
    }
    tuning
}

fn parse(text: &str) -> Vec<Section> {
    let mut sections = Vec::new();
    let mut current: Option<Section> = None;
    for line in text.lines() {
        let line = line.split(';').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_suffix('{') {
            current = Some(Section {
                name: name.trim().to_ascii_uppercase(),
                keys: BTreeMap::new(),
            });
            continue;
        }
        if line.starts_with('}') {
            sections.extend(current.take());
            continue;
        }
        if let Some(section) = &mut current {
            let mut words = line.split_whitespace();
            if let Some(key) = words.next() {
                let values = words
                    .map(|word| word.trim_matches('"').to_string())
                    .collect();
                section.keys.insert(key.to_ascii_lowercase(), values);
            }
        }
    }
    sections
}

fn translate(sections: &[Section]) -> Vec<SurfaceTuning> {
    let mut corrugations = CORRUGATIONS;
    let mut redefined = [false; 8];
    for section in sections
        .iter()
        .filter(|section| section.name == "CORRUGATION")
    {
        let Some(id) = section
            .number("id")
            .map(|id| id as usize)
            .filter(|&id| id < 8)
        else {
            continue;
        };
        if let Some(amp) = section.number("amplitude") {
            corrugations[id][0] = amp;
        }
        if let Some(waves) = section
            .numbers("wavelength")
            .filter(|waves| waves.len() == 2)
        {
            corrugations[id][1] = waves[0];
            corrugations[id][2] = waves[1];
        }
        redefined[id] = true;
    }

    let materials: BTreeMap<usize, &Section> = sections
        .iter()
        .filter(|section| section.name == "MATERIAL")
        .filter_map(|section| Some((section.number("id")? as usize, section)))
        .filter(|&(id, _)| id < SurfaceType::ALL.len())
        .collect();
    let meters = |[amp, lx, lz]: [f32; 3]| {
        let s = axes::REVOLT_TO_METERS;
        [amp * s, lx * s, lz * s]
    };

    let mut out = Vec::new();
    for (index, &surface) in SurfaceType::ALL.iter().enumerate() {
        let material = materials.get(&index);
        let kind = material
            .and_then(|m| m.number("corrugationtype"))
            .map(|kind| kind as usize)
            .filter(|&kind| kind < 8)
            .or(STOCK_CORRUGATION[index]);
        let bumpy = material
            .and_then(|m| m.flag("corrugated"))
            .unwrap_or(STOCK_CORRUGATION[index].is_some());
        let corrugation_changed = material.is_some_and(|m| {
            m.keys.contains_key("corrugated") || m.keys.contains_key("corrugationtype")
        }) || kind.is_some_and(|kind| redefined[kind]);
        let corrugation = corrugation_changed.then(|| match kind {
            Some(kind) if bumpy && kind > 0 => Some(meters(corrugations[kind])),
            _ => None,
        });
        let conveyor = material.and_then(|m| match m.flag("moves") {
            Some(false) => Some(Vec3::ZERO),
            _ => m
                .numbers("velocity")
                .filter(|v| v.len() == 3)
                .map(|v| axes::direction([v[0], v[1], v[2]]) * axes::REVOLT_TO_METERS),
        });
        let tune = SurfaceTuning {
            surface,
            roughness: material.and_then(|m| m.number("roughness")),
            grip: material.and_then(|m| m.number("grip")),
            hardness: material.and_then(|m| m.number("hardness")),
            corrugation,
            conveyor,
        };
        let changes = tune.roughness.is_some()
            || tune.grip.is_some()
            || tune.hardness.is_some()
            || tune.corrugation.is_some()
            || tune.conveyor.is_some();
        if changes {
            out.push(tune);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const WILDLAND: &str = "\
MATERIAL {
  ID              18                            ; Material to replace [0 - 26]
  Name            \"DIRT\"                      ; Display name
  Corrugated      true                          ; Material is bumpy
  Moves           false                         ; Moves like museum conveyors
  Roughness       0.730000                      ; Roughness of the material
  Grip            0.342500                      ; Grip of the material
  Hardness        0.200000                      ; Hardness of the material
  CorrugationType 2                             ; Type of bumpiness [0 - 7]
  Velocity        0.000000 0.000000 0.000000    ; Move cars
}
DUST {
  ID              4
  SparkType       4
}
";

    #[test]
    fn a_material_keeps_what_it_does_not_say() {
        let tuning = translate(&parse(WILDLAND));
        assert_eq!(tuning.len(), 1, "{tuning:?}");
        let dirt = &tuning[0];
        assert_eq!(dirt.surface, SurfaceType::Dirt);
        assert_eq!(
            (dirt.roughness, dirt.grip, dirt.hardness),
            (Some(0.73), Some(0.3425), Some(0.2))
        );
        // `CorrugationType 2` es la grava: 1 unidad de alto cada 40.
        let [amp, lx, lz] = dirt.corrugation.unwrap().unwrap();
        assert!((amp - 0.005).abs() < 1e-7 && (lx - 0.2).abs() < 1e-6 && (lz - 0.2).abs() < 1e-6);
        assert_eq!(dirt.conveyor, Some(Vec3::ZERO));
    }

    #[test]
    fn a_new_corrugation_reaches_every_material_that_uses_it() {
        let text = "CORRUGATION {\n  ID 5\n  Amplitude 1.4\n  Wavelength 60.0 60.0\n}\n";
        let tuning = translate(&parse(text));
        // Solo la tierra 1 usa el tipo 5.
        assert_eq!(tuning.len(), 1);
        assert_eq!(tuning[0].surface, SurfaceType::Dirt);
        assert_eq!(tuning[0].roughness, None);
        let [amp, lx, _] = tuning[0].corrugation.unwrap().unwrap();
        assert!((amp - 0.007).abs() < 1e-7 && (lx - 0.3).abs() < 1e-6);
    }
}
