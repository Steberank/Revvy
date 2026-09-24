//! AI nodes `.fan`, como los lee `LoadAiNodes` (`rvsource/Xbox/Src/ainode.cpp`). En PC el
//! archivo es little-endian.
//!
//! - Cabecera: `short AiSingleNodeNum` y `short AiLinkNodeNum`. Los nodos link los arma
//!   `SetupNewMethodLinkNodes` en memoria y no tienen datos en el archivo.
//! - `AiSingleNodeNum` veces `FILE_AINODE` (`editai.h`), de 76 bytes.
//! - `long AiStartNode` y `REAL AiNodeTotalDist`. RVGL agrega otro `long`, el nodo final
//!   de las pistas sprint (`ENDPOS`). Las arenas de batalla traen solo la cabecera en cero.
//!
//! `FILE_AINODE`, en bytes desde el inicio del nodo:
//!
//! | offset | campo |
//! | --- | --- |
//! | 0 | `char Priority` (`AIN_TYPE`), `char StartNode` |
//! | 2 | `char flags[2]`: `AIN_LF_WALL_LEFT` 0x01, `AIN_LF_WALL_RIGHT` 0x02 |
//! | 4 | `REAL RacingLine, FinishDist, OvertakingLine, fpad` |
//! | 20 | `long RacingLineSpeed, CentreSpeed` |
//! | 28 | `long Prev[2]`, después `Next[2]` (`-1` no conecta) |
//! | 44 | `Node[2]`, cada uno `long Speed` y `VEC Pos` |
//!
//! El loader invierte los extremos: el `Node[1]` del archivo es el verde (`Node[0]` en el
//! juego), a la izquierda del sentido de carrera, y el `Node[0]` es el rojo, a la derecha.
//! `RacingLine` y `OvertakingLine` van de verde (0) a rojo (1). `Next` sigue el sentido de
//! carrera: `FinishDist`, lo que falta hasta la meta, baja.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use glam::Vec3;

use crate::axes;
use crate::binutil::Reader;
use crate::layout::{links2, AiFlags, AiNode};
use crate::FormatError;

/// `sizeof(FILE_AINODE)`.
const NODE_SIZE: usize = 76;

/// `AIN_LF_WALL_LEFT` y `AIN_LF_WALL_RIGHT`, en `flags[0]`.
const WALL_LEFT: u8 = 0x01;
const WALL_RIGHT: u8 = 0x02;

// `AIN_TYPE` de `ainode.h`, la `Priority` del nodo. `RACINGLINE` (0), `TURBOLINE` (8) y
// `SHORTCUT` (10) son la ruta normal.
const AIN_TYPE_PICKUP: u8 = 1;
const AIN_TYPE_STAIRS: u8 = 2;
const AIN_TYPE_BUMPY: u8 = 3;
const AIN_TYPE_SLOWDOWN_25: u8 = 4;
const AIN_TYPE_SOFTSUSPENSION: u8 = 5;
const AIN_TYPE_JUMPWALL: u8 = 6;
const AIN_TYPE_TITLESCR_SLOWDOWN: u8 = 7;
const AIN_TYPE_LONGPICKUP: u8 = 9;
const AIN_TYPE_LONGCUT: u8 = 11;
const AIN_TYPE_BARRELBLOCK: u8 = 12;
const AIN_TYPE_OFFTHROTTLE: u8 = 13;
const AIN_TYPE_OFFTHROTTLEPETROL: u8 = 14;
const AIN_TYPE_WILDERNESS: u8 = 15;
const AIN_TYPE_SLOWDOWN_15: u8 = 16;
const AIN_TYPE_SLOWDOWN_20: u8 = 17;
const AIN_TYPE_SLOWDOWN_30: u8 = 18;

pub fn parse(path: &Path) -> Result<Vec<AiNode>, FormatError> {
    let file = File::open(path).map_err(|err| FormatError::io(path, err))?;
    let mut reader = Reader::new(BufReader::new(file));
    let single = reader.i16()?;
    let _link = reader.i16()?;
    if single < 0 {
        return Err(FormatError::parse(path, "cantidad de AI nodes negativa"));
    }
    let count = single as usize;
    let body = reader.rest()?;
    // Después de los nodos van el inicio y la distancia total, y en RVGL el nodo final.
    let fits = match body.len().checked_sub(count * NODE_SIZE) {
        Some(8 | 12) => true,
        Some(0) => count == 0,
        _ => false,
    };
    if !fits {
        return Err(FormatError::parse(
            path,
            format!(
                "tamaño de .fan no cierra: {count} nodos en {} bytes",
                body.len() + 4
            ),
        ));
    }

    let mut nodes: Vec<AiNode> = body
        .as_chunks::<NODE_SIZE>()
        .0
        .iter()
        .take(count)
        .enumerate()
        .map(|(id, node)| read_node(id as u32, node))
        .collect();
    // Algunas pistas custom apuntan a nodos que no están en el archivo: esos links no van.
    for node in &mut nodes {
        node.prev.retain(|&id| (id as usize) < count);
        node.next.retain(|&id| (id as usize) < count);
    }
    Ok(nodes)
}

fn read_node(id: u32, node: &[u8; NODE_SIZE]) -> AiNode {
    let priority = node[0];
    let walls = node[2];
    let racing_t = f32_at(node, 4);
    let overtaking_t = f32_at(node, 12);
    let prev = links2([i32_at(node, 28), i32_at(node, 32)]);
    let next = links2([i32_at(node, 36), i32_at(node, 40)]);
    // Cada extremo es `Speed` y después `Pos`: `Node[0]` del archivo es el rojo y
    // `Node[1]` el verde.
    let right = pos_at(node, 48);
    let left = pos_at(node, 64);
    let (mut flags, speed_limit) = priority_flags(priority);
    flags.wall_left = walls & WALL_LEFT != 0;
    flags.wall_right = walls & WALL_RIGHT != 0;
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
        speed_limit,
    }
}

/// Lo que hace la IA de `ai_car.cpp` con cada `Priority`. El límite va en m/s.
fn priority_flags(priority: u8) -> (AiFlags, Option<f32>) {
    let mph = |mph: f32| Some(mph * axes::MPH2OGU_SPEED * axes::REVOLT_TO_METERS);
    let slowdown = AiFlags {
        slowdown: true,
        ..AiFlags::default()
    };
    let pickup_route = AiFlags {
        racing: false,
        pickup_route: true,
        ..AiFlags::default()
    };
    let careful = AiFlags {
        racing: false,
        careful: true,
        ..AiFlags::default()
    };
    match priority {
        // Frena por encima de esa velocidad.
        AIN_TYPE_SLOWDOWN_15 => (slowdown, mph(15.0)),
        AIN_TYPE_SLOWDOWN_20 => (slowdown, mph(20.0)),
        AIN_TYPE_SLOWDOWN_25 => (slowdown, mph(25.0)),
        AIN_TYPE_SLOWDOWN_30 => (slowdown, mph(30.0)),
        // Frena por encima del 25 % de la velocidad máxima del auto.
        AIN_TYPE_TITLESCR_SLOWDOWN => (slowdown, None),
        // Suelta el acelerador por encima de 20 mph (`PETROL`, solo en los autos glow).
        AIN_TYPE_OFFTHROTTLE | AIN_TYPE_OFFTHROTTLEPETROL => (slowdown, mph(20.0)),
        // Desvío a los rayitos, según `pickupBias`.
        AIN_TYPE_PICKUP | AIN_TYPE_LONGPICKUP => (pickup_route, None),
        // Rutas que se eligen según la suspensión o que restan puntos: `BARRELBLOCK` nunca
        // se toma si hay otra.
        AIN_TYPE_BUMPY
        | AIN_TYPE_SOFTSUSPENSION
        | AIN_TYPE_STAIRS
        | AIN_TYPE_JUMPWALL
        | AIN_TYPE_LONGCUT
        | AIN_TYPE_WILDERNESS
        | AIN_TYPE_BARRELBLOCK => (careful, None),
        // La ruta normal, y los valores desconocidos como el `default` de `ai_car.cpp`.
        _ => (AiFlags::default(), None),
    }
}

fn f32_at(node: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes(node[offset..offset + 4].try_into().unwrap())
}

fn i32_at(node: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(node[offset..offset + 4].try_into().unwrap())
}

fn pos_at(node: &[u8], offset: usize) -> Vec3 {
    axes::position([
        f32_at(node, offset),
        f32_at(node, offset + 4),
        f32_at(node, offset + 8),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_fields_follow_file_ainode() {
        let mut bytes = [0u8; NODE_SIZE];
        bytes[0] = AIN_TYPE_SLOWDOWN_20;
        bytes[2] = WALL_RIGHT;
        let mut put =
            |offset: usize, value: [u8; 4]| bytes[offset..offset + 4].copy_from_slice(&value);
        put(4, 0.25f32.to_le_bytes());
        put(8, 900.0f32.to_le_bytes());
        put(12, 0.75f32.to_le_bytes());
        put(28, 7i32.to_le_bytes());
        put(32, (-1i32).to_le_bytes());
        put(36, 8i32.to_le_bytes());
        put(40, 9i32.to_le_bytes());
        // Rojo en +X de Re-Volt (la derecha mirando a +Z) y verde en −X.
        put(44, 30i32.to_le_bytes());
        put(48, 200.0f32.to_le_bytes());
        put(60, 30i32.to_le_bytes());
        put(64, (-200.0f32).to_le_bytes());

        let node = read_node(3, &bytes);
        assert_eq!(node.racing_t, 0.25);
        assert_eq!(node.overtaking_t, 0.75);
        assert_eq!(node.prev, [7]);
        assert_eq!(node.next, [8, 9]);
        assert!(node.left.distance(Vec3::X) < 1e-6, "verde: {:?}", node.left);
        assert!(
            node.right.distance(-Vec3::X) < 1e-6,
            "rojo: {:?}",
            node.right
        );
        assert!(node.flags.racing && node.flags.slowdown);
        assert!(node.flags.wall_right && !node.flags.wall_left);
        let limit = node.speed_limit.expect("SLOWDOWN_20");
        assert!((limit - 20.0 * 0.44704).abs() < 0.01, "{limit} m/s");
    }
}
