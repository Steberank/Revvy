//! Objetos `.fob`. El rayito de stock es el tipo 30: en `nhood1` es el que
//! más cerca cae de los POS nodes. Las tablas de odds custom no están en el
//! binario; si no hay tabla, `odds` queda en `None`.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use crate::axes;
use crate::binutil::Reader;
use crate::layout::PickupSpawn;
use crate::FormatError;

pub const PICKUP_TYPE: i32 = 30;

pub fn parse_pickups(path: &Path) -> Result<Vec<PickupSpawn>, FormatError> {
    let file = File::open(path).map_err(|err| FormatError::io(path, err))?;
    let mut reader = Reader::new(BufReader::new(file));
    let count = reader.i32()?;
    if count < 0 {
        return Err(FormatError::parse(path, "cantidad de objetos negativa"));
    }
    let mut pickups = Vec::new();
    for _ in 0..count {
        let kind = reader.i32()?;
        reader.skip(16)?;
        let pos = axes::position(reader.v3()?);
        reader.skip(24)?;
        if kind == PICKUP_TYPE {
            pickups.push(PickupSpawn {
                pos,
                respawn_secs: 5.0,
                odds: None,
            });
        }
    }
    Ok(pickups)
}

/// `FILE_OBJECT` sin convertir: id de tipo, los cuatro flags, posición y ejes
/// up / look en espacio de Re-Volt. La matriz del objeto es `R = U × L`.
#[derive(Clone, Debug)]
pub struct FobObject {
    pub id: i32,
    pub flags: [i32; 4],
    pub pos: [f32; 3],
    pub up: [f32; 3],
    pub look: [f32; 3],
}

/// Tipo 49 (`OBJECT_TYPE_3DSOUND`): flags = índice en la tabla de sonidos,
/// rango ×0.1 y 0 = continuo / otro = aleatorio.
pub const SOUND_3D_TYPE: i32 = 49;
/// Tipo 40 (`OBJECT_TYPE_SPRINKLER`).
pub const SPRINKLER_TYPE: i32 = 40;

pub fn parse_objects(path: &Path) -> Result<Vec<FobObject>, FormatError> {
    let file = File::open(path).map_err(|err| FormatError::io(path, err))?;
    let mut reader = Reader::new(BufReader::new(file));
    let count = reader.i32()?;
    if count < 0 {
        return Err(FormatError::parse(path, "cantidad de objetos negativa"));
    }
    let mut objects = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let id = reader.i32()?;
        let flags = [reader.i32()?, reader.i32()?, reader.i32()?, reader.i32()?];
        let pos = reader.v3()?;
        let up = reader.v3()?;
        let look = reader.v3()?;
        objects.push(FobObject {
            id,
            flags,
            pos,
            up,
            look,
        });
    }
    Ok(objects)
}
