//! Track zones `.taz` → `TrackZone`. El `size` del archivo es el half-extent.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use crate::axes;
use crate::binutil::Reader;
use crate::layout::TrackZone;
use crate::FormatError;

pub fn parse(path: &Path) -> Result<Vec<TrackZone>, FormatError> {
    let file = File::open(path).map_err(|err| FormatError::io(path, err))?;
    let mut reader = Reader::new(BufReader::new(file));
    let count = reader.i32()?;
    if count < 0 {
        return Err(FormatError::parse(path, "cantidad de zonas negativa"));
    }
    let mut zones = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let id = reader.i32()?;
        let center = axes::position(reader.v3()?);
        let mut rows = [[0.0; 3]; 3];
        for row in &mut rows {
            *row = reader.v3()?;
        }
        let half_extents = axes::position(reader.v3()?).abs();
        zones.push(TrackZone {
            id,
            center,
            rotation: axes::rotation(rows),
            half_extents,
        });
    }
    Ok(zones)
}
