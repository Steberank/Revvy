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
