//! Mundo `.w`: meshes estáticos, bigcubes, animaciones de textura y colores de entorno.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use crate::binutil::Reader;
use crate::mesh::{self, RawPolygon, RawVertex, VisualMesh};
use crate::FormatError;

#[derive(Clone, Debug)]
pub struct WorldMesh {
    pub vertices: Vec<RawVertex>,
    pub polygons: Vec<RawPolygon>,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct World {
    pub meshes: Vec<WorldMesh>,
    pub bigcube_count: i32,
    pub animation_count: u32,
    pub env_colors: usize,
}

impl World {
    pub fn parse(path: &Path) -> Result<Self, FormatError> {
        let file = File::open(path).map_err(|err| FormatError::io(path, err))?;
        let mut reader = Reader::new(BufReader::new(file));
        let mesh_count = reader.i32()?;
        if mesh_count < 0 {
            return Err(FormatError::parse(path, "mesh_count negativo"));
        }
        let mut meshes = Vec::with_capacity(mesh_count as usize);
        let mut env_colors = 0usize;
        for _ in 0..mesh_count {
            let mesh = read_mesh(&mut reader, &mut env_colors)?;
            meshes.push(mesh);
        }

        let bigcube_count = reader.i32()?;
        if bigcube_count < 0 {
            return Err(FormatError::parse(path, "bigcube_count negativo"));
        }
        for _ in 0..bigcube_count {
            let _center = reader.v3()?;
            let _size = reader.f32()?;
            let mesh_indices = reader.i32()?;
            if mesh_indices < 0 {
                return Err(FormatError::parse(path, "índices de bigcube negativos"));
            }
            reader.skip(mesh_indices as usize * 4)?;
        }

        let animation_count = match reader.u32() {
            Ok(count) => count,
            Err(FormatError::Io(err)) if err.kind() == std::io::ErrorKind::UnexpectedEof => 0,
            Err(err) => return Err(err),
        };
        for _ in 0..animation_count {
            let frames = reader.u32()?;
            // textura u32 + delay f32 + 4 UV
            reader.skip(frames as usize * (4 + 4 + 4 * 8))?;
        }

        for _ in 0..env_colors {
            reader.skip(4)?;
        }

        let rest = reader.rest()?;
        if !rest.is_empty() {
            return Err(FormatError::parse(
                path,
                format!("quedaron {} bytes sin leer", rest.len()),
            ));
        }

        Ok(Self {
            meshes,
            bigcube_count,
            animation_count,
            env_colors,
        })
    }

    pub fn to_meshes(&self) -> Result<Vec<VisualMesh>, FormatError> {
        let mut out = Vec::new();
        for (index, mesh) in self.meshes.iter().enumerate() {
            out.extend(mesh::bake_meshes(
                &format!("world-{index}"),
                &mesh.vertices,
                &mesh.polygons,
                None,
            )?);
        }
        Ok(out)
    }
}

fn read_mesh<R: std::io::Read>(
    reader: &mut Reader<R>,
    env_colors: &mut usize,
) -> Result<WorldMesh, FormatError> {
    let _center = reader.v3()?;
    let _radius = reader.f32()?;
    reader.skip(24)?;
    let polygon_count = reader.u16()? as usize;
    let vertex_count = reader.u16()? as usize;
    let mut polygons = Vec::with_capacity(polygon_count);
    for _ in 0..polygon_count {
        let polygon = mesh::read_polygon(reader)?;
        if polygon.is_env() {
            *env_colors += 1;
        }
        polygons.push(polygon);
    }
    let mut vertices = Vec::with_capacity(vertex_count);
    for _ in 0..vertex_count {
        vertices.push(mesh::read_vertex(reader)?);
    }
    Ok(WorldMesh { vertices, polygons })
}
