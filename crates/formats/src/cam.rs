//! Cámaras de replay `.cam`. Se leen para no fallar al abrir el nivel; no gobiernan la v1.

use std::path::Path;

use crate::binutil::Reader;
use crate::FormatError;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ReplayCamera {
    pub bytes: [u8; 36],
}

pub fn parse(path: &Path) -> Result<Vec<ReplayCamera>, FormatError> {
    read_records(path, 36).map(|rows| {
        rows.into_iter()
            .map(|bytes| ReplayCamera {
                bytes: bytes.try_into().unwrap(),
            })
            .collect()
    })
}

pub(crate) fn read_records(path: &Path, size: usize) -> Result<Vec<Vec<u8>>, FormatError> {
    let file = std::fs::File::open(path).map_err(|err| FormatError::io(path, err))?;
    let mut reader = Reader::new(std::io::BufReader::new(file));
    let count = reader.i32()?;
    if count < 0 {
        return Err(FormatError::parse(path, "cantidad negativa"));
    }
    let mut rows = Vec::with_capacity(count as usize);
    for _ in 0..count {
        rows.push(reader.bytes(size)?);
    }
    Ok(rows)
}
