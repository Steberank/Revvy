//! Texturas `.bmp`. Los `.bmq` son mipmaps del juego y no se cargan.
//!
//! RVGL acepta con extensión `.bmp` también PNG, JPG, WEBP, GIF, TIF, ICO y PNM: se
//! reconoce el formato por el contenido.

use std::path::Path;

use crate::FormatError;

pub fn load(path: &Path) -> Result<image::RgbaImage, FormatError> {
    let bytes = std::fs::read(path).map_err(|err| FormatError::io(path, err))?;
    let image = image::load_from_memory(&bytes).map_err(|err| FormatError::Parse {
        path: path.display().to_string(),
        message: err.to_string(),
    })?;
    Ok(image.to_rgba8())
}

pub fn load_pages(level_dir: &Path, stem: &str) -> Vec<(i16, image::RgbaImage)> {
    let mut pages = Vec::new();
    for index in 0..26 {
        let letter = (b'a' + index as u8) as char;
        let name = format!("{stem}{letter}.bmp");
        let Some(path) = crate::level_file(level_dir, &name) else {
            continue;
        };
        match load(&path) {
            Ok(image) => pages.push((index, image)),
            Err(err) => tracing::warn!(%err, "textura de pista"),
        }
    }
    pages
}
