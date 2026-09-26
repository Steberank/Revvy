//! Reglas de sala (`config/rules/*.ron`, §1.6). Por ahora, las que usa la carrera local:
//! las vueltas y el tiempo fuera de pista. El resto de los campos del archivo se lee en la
//! fase 4 y hasta entonces se ignora.

use std::path::Path;

use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct GameplayRules {
    /// Vueltas de la carrera. La sala las puede cambiar antes de empezar.
    #[serde(default = "default_laps")]
    pub laps: u32,
    /// Segundos fuera de todas las zonas antes de reposicionar el auto (§7.12).
    #[serde(default = "default_off_track_secs")]
    pub off_track_secs: f32,
}

#[derive(Debug, thiserror::Error)]
pub enum RulesError {
    #[error("no se pudo leer {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("reglas inválidas en {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: Box<ron::error::SpannedError>,
    },
}

fn default_laps() -> u32 {
    3
}

fn default_off_track_secs() -> f32 {
    1.5
}

impl Default for GameplayRules {
    fn default() -> Self {
        Self {
            laps: default_laps(),
            off_track_secs: default_off_track_secs(),
        }
    }
}

impl GameplayRules {
    pub fn load(path: &Path) -> Result<Self, RulesError> {
        let text = std::fs::read_to_string(path).map_err(|source| RulesError::Read {
            path: path.display().to_string(),
            source,
        })?;
        ron::from_str(&text).map_err(|source| RulesError::Parse {
            path: path.display().to_string(),
            source: Box::new(source),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_rules_file_has_the_laps() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/rules/default.ron");
        let rules = GameplayRules::load(&path).expect("default.ron");
        assert_eq!(rules.laps, 3);
        assert_eq!(rules.off_track_secs, 1.5);
    }
}
