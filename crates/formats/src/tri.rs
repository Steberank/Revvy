//! Triggers `.tri`: cajas orientadas que hacen algo cuando entra un auto. El tipo es el
//! índice en `TriggerInfo` de `trigger.cpp` y `flag` es su parámetro. Los de tipo 2
//! (`TriggerTrackDir`) son las flechas que indican hacia dónde sigue la pista.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use glam::{Quat, Vec3};

use crate::axes;
use crate::binutil::Reader;
use crate::FormatError;

/// `TriggerObjectThrower`: el lanzador del `.fob` con `ID == flag` tira su objeto.
pub const TRIGGER_OBJECT_THROWER: i32 = 6;
/// `TriggerRepositionCar`: el auto quedó afuera y vuelve a la pista.
pub const TRIGGER_REPOSITION: i32 = 8;

#[derive(Clone, Debug)]
pub struct Trigger {
    pub kind: i32,
    pub flag: i32,
    pub center: Vec3,
    pub rotation: Quat,
    pub half_extents: Vec3,
}

/// `FILE_TRIGGER`: tipo, flag, posición, matriz y medio tamaño en cada eje, 68 bytes.
pub fn parse(path: &Path) -> Result<Vec<Trigger>, FormatError> {
    let file = File::open(path).map_err(|err| FormatError::io(path, err))?;
    let mut reader = Reader::new(BufReader::new(file));
    let count = reader.i32()?;
    if count < 0 {
        return Err(FormatError::parse(path, "cantidad de triggers negativa"));
    }
    let mut triggers = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let kind = reader.i32()?;
        let flag = reader.i32()?;
        let center = axes::position(reader.v3()?);
        let mut rows = [[0.0; 3]; 3];
        for row in &mut rows {
            *row = reader.v3()?;
        }
        let half_extents = axes::position(reader.v3()?).abs();
        triggers.push(Trigger {
            kind,
            flag,
            center,
            rotation: axes::rotation(rows),
            half_extents,
        });
    }
    Ok(triggers)
}
