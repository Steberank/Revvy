//! Campos de fuerza `.fld`. Tipo 0 → `GravityScale` según el eje Y de Re-Volt.
//! El resto se guarda como `ConstantForce` con la dirección del archivo.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use crate::axes;
use crate::binutil::Reader;
use crate::layout::{ForceField, ForceKind};
use crate::FormatError;

pub fn parse(path: &Path) -> Result<Vec<ForceField>, FormatError> {
    let bytes = std::fs::read(path).map_err(|err| FormatError::io(path, err))?;
    if bytes.len() < 4 {
        return Ok(Vec::new());
    }
    let count = i32::from_le_bytes(bytes[0..4].try_into().unwrap());
    if count <= 0 {
        return Ok(Vec::new());
    }
    let rest = bytes.len() - 4;
    let record = if rest == count as usize * 100 {
        100
    } else if rest == count as usize * 104 {
        104
    } else {
        tracing::warn!(
            path = %path.display(),
            bytes = bytes.len(),
            count,
            "fld con tamaño no reconocido; se ignora"
        );
        return Ok(Vec::new());
    };

    let file = File::open(path).map_err(|err| FormatError::io(path, err))?;
    let mut reader = Reader::new(BufReader::new(file));
    let _count = reader.i32()?;
    let mut fields = Vec::with_capacity(count as usize);
    for id in 0..count as u32 {
        let kind_id = reader.i32()?;
        let center = axes::position(reader.v3()?);
        let mut rows = [[0.0; 3]; 3];
        for row in &mut rows {
            *row = reader.v3()?;
        }
        let half_extents = axes::position(reader.v3()?).abs();
        let direction = reader.v3()?;
        let _magnitude = reader.f32()?;
        reader.skip(record - (4 + 12 + 36 + 12 + 12 + 4))?;
        let kind = if kind_id == 0 {
            // Y positiva de Re-Volt apunta hacia abajo: 1 es gravedad normal.
            ForceKind::GravityScale(direction[1])
        } else {
            ForceKind::ConstantForce(axes::direction(direction))
        };
        fields.push(ForceField {
            id,
            center,
            rotation: axes::rotation(rows),
            half_extents,
            kind,
        });
    }
    Ok(fields)
}
