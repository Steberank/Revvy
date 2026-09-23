//! Perfil de IA `.pro`. En el árbol de pistas de este repo no hay muestras,
//! así que el parser acepta el archivo y no falla si el layout no se reconoce.

use std::path::Path;

use crate::FormatError;

#[derive(Clone, Debug)]
pub struct AiProfile {
    pub bytes: usize,
}

pub fn parse(path: &Path) -> Result<AiProfile, FormatError> {
    let bytes = std::fs::read(path).map_err(|err| FormatError::io(path, err))?;
    Ok(AiProfile { bytes: bytes.len() })
}
