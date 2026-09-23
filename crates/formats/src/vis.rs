//! Visiboxes `.vis`. Cada registro es un id y un AABB. No gobiernan la v1.

use std::path::Path;

use glam::Vec3;

use crate::axes;
use crate::binutil::Reader;
use crate::FormatError;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Visibox {
    pub id: i32,
    pub min: Vec3,
    pub max: Vec3,
}

pub fn parse(path: &Path) -> Result<Vec<Visibox>, FormatError> {
    let file = std::fs::File::open(path).map_err(|err| FormatError::io(path, err))?;
    let mut reader = Reader::new(std::io::BufReader::new(file));
    let count = reader.i32()?;
    if count < 0 {
        return Err(FormatError::parse(path, "cantidad de visiboxes negativa"));
    }
    let mut boxes = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let id = reader.i32()?;
        let a = axes::position(reader.v3()?);
        let b = axes::position(reader.v3()?);
        boxes.push(Visibox {
            id,
            min: a.min(b),
            max: a.max(b),
        });
    }
    Ok(boxes)
}
