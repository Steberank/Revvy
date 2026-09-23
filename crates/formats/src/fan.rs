//! AI nodes `.fan`.
//!
//! Confirmado contra `nhood1.fan` y otros niveles de RVGL: cabecera de 12 bytes
//! y nodos de 76. Algunos archivos agregan un `end_node` de 4 bytes al final
//! (pistas sprint). Izquierda y derecha son las esferas verde y roja.
//! `racing_t` y `overtaking_t` están en \[0, 1\]. Los links `-1` no conectan;
//! el archivo trae dos slots por sentido, no el tope histórico de 4 del `.pan`.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use crate::axes;
use crate::binutil::Reader;
use crate::layout::{links2, AiFlags, AiNode};
use crate::FormatError;

const NODE_SIZE: usize = 76;

pub fn parse(path: &Path) -> Result<Vec<AiNode>, FormatError> {
    let file = File::open(path).map_err(|err| FormatError::io(path, err))?;
    let mut reader = Reader::new(BufReader::new(file));
    let count16 = reader.u16()? as usize;
    let _version16 = reader.u16()?;
    let _header_rest = reader.i32()?;
    let _header_float = reader.f32()?;
    let body = reader.rest()?;
    let (count, nodes_bytes) = if body.len() % NODE_SIZE == 0 {
        (body.len() / NODE_SIZE, body.as_slice())
    } else if body.len() >= 4 && (body.len() - 4) % NODE_SIZE == 0 {
        ((body.len() - 4) / NODE_SIZE, &body[..body.len() - 4])
    } else {
        return Err(FormatError::parse(
            path,
            format!("tamaño de .fan no cierra ({} bytes)", body.len() + 12),
        ));
    };
    if count != count16 && count16 != 0 {
        // La versión nueva guarda el count como i32; el u16 bajo coincide igual.
        let count32 = u32::from_le_bytes([
            count16 as u8,
            (count16 >> 8) as u8,
            _version16 as u8,
            (_version16 >> 8) as u8,
        ]) as usize;
        if count != count16 && count != count32 {
            return Err(FormatError::parse(
                path,
                format!("count de cabecera {count16}/{count32} != {count} nodos"),
            ));
        }
    }

    let mut nodes = Vec::with_capacity(count);
    for id in 0..count {
        let node = &nodes_bytes[id * NODE_SIZE..(id + 1) * NODE_SIZE];
        nodes.push(read_node(id as u32, node));
    }
    Ok(nodes)
}

fn read_node(id: u32, node: &[u8]) -> AiNode {
    let racing_t = f32_at(node, 4);
    let overtaking_t = f32_at(node, 72);
    let next = links2([i32_at(node, 20), i32_at(node, 24)]);
    let prev = links2([i32_at(node, 28), i32_at(node, 32)]);
    let left = axes::position([f32_at(node, 40), f32_at(node, 44), f32_at(node, 48)]);
    let right = axes::position([f32_at(node, 56), f32_at(node, 60), f32_at(node, 64)]);
    let extra = node[68];
    let racing_prop = node[70];
    let flags = map_flags(extra, racing_prop);
    AiNode {
        id,
        left,
        right,
        racing_t,
        overtaking_t: if (0.0..=1.0).contains(&overtaking_t) {
            overtaking_t
        } else {
            racing_t.clamp(0.0, 1.0)
        },
        prev,
        next,
        flags,
        speed_limit: None,
    }
}

fn map_flags(extra: u8, racing_prop: u8) -> AiFlags {
    // Bytes vistos en nhood1. El enum cerrado de v1 se llena con lo que el
    // byte de propiedad permite afirmar; WallLeft/WallRight no viajan en este archivo.
    let mut flags = AiFlags::default();
    flags.racing = racing_prop == 0;
    flags.pickup_route = racing_prop == 1;
    flags.careful = racing_prop == 2;
    flags.slowdown = extra == 5 || racing_prop == 3;
    flags
}

fn f32_at(node: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes(node[offset..offset + 4].try_into().unwrap())
}

fn i32_at(node: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(node[offset..offset + 4].try_into().unwrap())
}
