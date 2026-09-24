use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ClientConfig {
    pub window_title: String,
    pub window_width: u32,
    pub window_height: u32,
    pub clear_color: [f64; 4],
    /// Raíz de contenido (`levels/`, `cars/`, `wavs/`, `gfx/`). Relativa al repo o absoluta.
    #[serde(default = "default_content_root")]
    pub content_root: String,
    /// Id dentro de `<content_root>/levels`, o una ruta a la carpeta del mapa.
    #[serde(default = "default_level")]
    pub level: String,
    /// Id dentro de `<content_root>/cars`, o una ruta a la carpeta del auto.
    #[serde(default = "default_car")]
    pub car: String,
    /// Más autos en los puestos siguientes de la grilla, quietos hasta que se los maneje
    /// (Tab cambia de auto). Pueden ser de Re-Volt o propios.
    #[serde(default)]
    pub extra_cars: Vec<String>,
    /// Volumen maestro de efectos, 0–127.
    #[serde(default = "default_sfx_volume")]
    pub sfx_volume: i32,
}

fn default_content_root() -> String {
    "content".into()
}

fn default_level() -> String {
    "nhood1".into()
}

fn default_car() -> String {
    "phim_calcure".into()
}

fn default_sfx_volume() -> i32 {
    90
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("no se pudo leer {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("config inválida en {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: toml::de::Error,
    },
}

impl ClientConfig {
    /// La raíz de contenido resuelta contra la raíz del repo.
    pub fn content_dir(&self) -> PathBuf {
        let path = PathBuf::from(&self.content_root);
        if path.is_absolute() {
            path
        } else {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(path)
        }
    }

    pub fn load() -> Result<Self, ConfigError> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../config/client.toml");
        let text = std::fs::read_to_string(&path).map_err(|source| ConfigError::Read {
            path: path.display().to_string(),
            source,
        })?;
        toml::from_str(&text).map_err(|source| ConfigError::Parse {
            path: path.display().to_string(),
            source,
        })
    }
}

pub fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_toml_parses() {
        let config = ClientConfig::load().expect("config/client.toml");
        assert_eq!(config.window_title, "Revvy");
        assert!(config.window_width > 0);
        assert!(config.window_height > 0);
        assert_eq!(config.clear_color.len(), 4);
        assert!(config.content_dir().join("levels").join(&config.level).is_dir());
        assert!(config.content_dir().join("cars").join(&config.car).is_dir());
        for car in &config.extra_cars {
            assert!(config.content_dir().join("cars").join(car).is_dir(), "{car}");
        }
    }
}
