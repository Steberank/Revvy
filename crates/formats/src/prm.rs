//! Mesh `.prm` / `.m`.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use crate::binutil::Reader;
use crate::mesh::{self, RawPolygon, RawVertex, VisualMesh};
use crate::FormatError;

#[derive(Clone, Debug)]
pub struct Prm {
    pub vertices: Vec<RawVertex>,
    pub polygons: Vec<RawPolygon>,
}

impl Prm {
    pub fn parse(path: &Path) -> Result<Self, FormatError> {
        let file = File::open(path).map_err(|err| FormatError::io(path, err))?;
        let mut reader = Reader::new(BufReader::new(file));
        let polygon_count = reader.u16()? as usize;
        let vertex_count = reader.u16()? as usize;
        let mut polygons = Vec::with_capacity(polygon_count);
        for _ in 0..polygon_count {
            polygons.push(mesh::read_polygon(&mut reader)?);
        }
        let mut vertices = Vec::with_capacity(vertex_count);
        for _ in 0..vertex_count {
            vertices.push(mesh::read_vertex(&mut reader)?);
        }
        Ok(Self { vertices, polygons })
    }

    pub fn to_meshes(&self, name: &str) -> Result<Vec<VisualMesh>, FormatError> {
        mesh::bake_meshes(name, &self.vertices, &self.polygons, None)
    }
}
