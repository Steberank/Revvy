//! POS nodes `.pan`. El archivo guarda 4 links; `-1` no es conexión.
//! El tipo nuevo no impone ese tope.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use crate::axes;
use crate::binutil::Reader;
use crate::layout::{links, PosNode};
use crate::FormatError;

pub struct PanFile {
    pub start_node: u32,
    pub total_distance: f32,
    pub nodes: Vec<PosNode>,
}

pub fn parse(path: &Path) -> Result<PanFile, FormatError> {
    let file = File::open(path).map_err(|err| FormatError::io(path, err))?;
    let mut reader = Reader::new(BufReader::new(file));
    let count = reader.i32()?;
    if count < 0 {
        return Err(FormatError::parse(path, "cantidad de POS negativa"));
    }
    let start_node = reader.i32()?;
    let total_distance = reader.f32()? * axes::REVOLT_TO_METERS;
    let mut nodes = Vec::with_capacity(count as usize);
    for id in 0..count {
        let position = axes::position(reader.v3()?);
        let distance = reader.f32()? * axes::REVOLT_TO_METERS;
        let prev = [reader.i32()?, reader.i32()?, reader.i32()?, reader.i32()?];
        let next = [reader.i32()?, reader.i32()?, reader.i32()?, reader.i32()?];
        nodes.push(PosNode {
            id: id as u32,
            position,
            distance,
            prev: links(prev),
            next: links(next),
        });
    }
    Ok(PanFile {
        start_node: start_node.max(0) as u32,
        total_distance,
        nodes,
    })
}
