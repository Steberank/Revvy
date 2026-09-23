//! Portales `.por`. Registro de 88 bytes cuando hay alguno. No gobiernan la v1.

use std::path::Path;

use crate::cam::read_records;
use crate::FormatError;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Portal {
    pub bytes: Vec<u8>,
}

pub fn parse(path: &Path) -> Result<Vec<Portal>, FormatError> {
    let bytes = std::fs::read(path).map_err(|err| FormatError::io(path, err))?;
    if bytes.len() <= 4 {
        return Ok(Vec::new());
    }
    Ok(read_records(path, 88)?
        .into_iter()
        .map(|bytes| Portal { bytes })
        .collect())
}
