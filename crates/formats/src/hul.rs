//! Piel de colisión `.hul`: cascos convexos y las esferas con las que el cuerpo toca el mundo.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use glam::Vec3;

use crate::axes;
use crate::binutil::Reader;
use crate::FormatError;

#[derive(Clone, Debug)]
pub struct HullSphere {
    pub center: Vec3,
    pub radius: f32,
}

pub fn load(path: &Path) -> Result<Vec<HullSphere>, FormatError> {
    let file = File::open(path).map_err(|err| FormatError::io(path, err))?;
    let mut reader = Reader::new(BufReader::new(file));
    let hulls = reader.i16()?;
    if hulls < 0 {
        return Err(FormatError::parse(path, "cantidad de cascos negativa"));
    }
    for _ in 0..hulls {
        let vertices = reader.i16()?;
        let edges = reader.i16()?;
        let faces = reader.i16()?;
        if vertices < 0 || edges < 0 || faces < 0 {
            return Err(FormatError::parse(path, "casco con conteos negativos"));
        }
        reader.skip(24 + 12)?;
        reader.skip(vertices as usize * 12)?;
        reader.skip(edges as usize * 4)?;
        reader.skip(faces as usize * 16)?;
    }
    let count = reader.i16()?;
    if count < 0 {
        return Err(FormatError::parse(path, "cantidad de esferas negativa"));
    }
    let mut spheres = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let center = axes::position(reader.v3()?);
        let radius = reader.f32()? * axes::REVOLT_TO_METERS;
        if radius > 0.0 && center.is_finite() {
            spheres.push(HullSphere { center, radius });
        }
    }
    Ok(spheres)
}

/// Esfera del `.hul` en espacio del modelo de Re-Volt (sin convertir): `[x, y, z, radio]`.
pub fn load_native_spheres(path: &Path) -> Result<Vec<[f32; 4]>, FormatError> {
    let file = File::open(path).map_err(|err| FormatError::io(path, err))?;
    let mut reader = Reader::new(BufReader::new(file));
    let hulls = reader.i16()?;
    if hulls < 0 {
        return Err(FormatError::parse(path, "cantidad de cascos negativa"));
    }
    for _ in 0..hulls {
        let vertices = reader.i16()?;
        let edges = reader.i16()?;
        let faces = reader.i16()?;
        if vertices < 0 || edges < 0 || faces < 0 {
            return Err(FormatError::parse(path, "casco con conteos negativos"));
        }
        reader.skip(24 + 12)?;
        reader.skip(vertices as usize * 12)?;
        reader.skip(edges as usize * 4)?;
        reader.skip(faces as usize * 16)?;
    }
    let count = reader.i16()?;
    if count < 0 {
        return Err(FormatError::parse(path, "cantidad de esferas negativa"));
    }
    let mut spheres = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let [x, y, z] = reader.v3()?;
        let radius = reader.f32()?;
        spheres.push([x, y, z, radius]);
    }
    Ok(spheres)
}
