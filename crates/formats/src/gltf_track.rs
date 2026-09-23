//! Pistas `revvy-glb-v1`. El pipeline entra en la fase 8.

use std::path::Path;

use crate::FormatError;

pub fn load(_path: &Path) -> Result<(), FormatError> {
    Err(FormatError::GlbNotImplemented)
}
