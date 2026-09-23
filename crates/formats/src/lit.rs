//! Luces `.lit` y `.li-`. Se leen y no gobiernan la v1.

use std::path::Path;

use glam::Vec3;

use crate::axes;
use crate::binutil::Reader;
use crate::FormatError;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Light {
    pub position: Vec3,
}

pub fn parse(path: &Path) -> Result<Vec<Light>, FormatError> {
    let file = std::fs::File::open(path).map_err(|err| FormatError::io(path, err))?;
    let mut reader = Reader::new(std::io::BufReader::new(file));
    let count = reader.i32()?;
    if count < 0 {
        return Err(FormatError::parse(path, "cantidad de luces negativa"));
    }
    let mut lights = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let position = axes::position(reader.v3()?);
        reader.skip(72 - 12)?;
        lights.push(Light { position });
    }
    Ok(lights)
}
