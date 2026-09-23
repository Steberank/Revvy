//! Polígonos y vértices compartidos por `.prm` y `.w`.

use glam::Vec3;

use crate::axes;
use crate::binutil::Reader;
use crate::FormatError;

pub const FACE_QUAD: i16 = 0x001;
pub const FACE_ENV: i16 = 0x800;

#[derive(Clone, Debug)]
pub struct RawVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

#[derive(Clone, Debug)]
pub struct RawPolygon {
    pub flags: i16,
    pub texture: i16,
    pub indices: [u16; 4],
    pub colors: [[u8; 4]; 4],
    pub uvs: [[f32; 2]; 4],
}

impl RawPolygon {
    pub fn is_quad(&self) -> bool {
        self.flags & FACE_QUAD != 0
    }

    pub fn is_env(&self) -> bool {
        self.flags & FACE_ENV != 0
    }
}

pub fn read_polygon<R: std::io::Read>(r: &mut Reader<R>) -> Result<RawPolygon, FormatError> {
    let flags = r.i16()?;
    let texture = r.i16()?;
    let indices = [r.u16()?, r.u16()?, r.u16()?, r.u16()?];
    let mut colors = [[0; 4]; 4];
    for color in &mut colors {
        let b = r.u8()?;
        let g = r.u8()?;
        let red = r.u8()?;
        let alpha_byte = r.u8()?;
        *color = [red, g, b, 255u8.wrapping_sub(alpha_byte)];
    }
    let mut uvs = [[0.0; 2]; 4];
    for uv in &mut uvs {
        *uv = [r.f32()?, r.f32()?];
    }
    Ok(RawPolygon {
        flags,
        texture,
        indices,
        colors,
        uvs,
    })
}

pub fn read_vertex<R: std::io::Read>(r: &mut Reader<R>) -> Result<RawVertex, FormatError> {
    Ok(RawVertex {
        position: r.v3()?,
        normal: r.v3()?,
    })
}

#[derive(Clone, Debug)]
pub struct VisualMesh {
    pub name: String,
    pub texture_page: i16,
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<[f32; 2]>,
    pub colors: Vec<[u8; 4]>,
    pub indices: Vec<u32>,
}

pub fn bake_meshes(
    name: &str,
    vertices: &[RawVertex],
    polygons: &[RawPolygon],
    transform: Option<(&[[f32; 3]; 3], [f32; 3])>,
) -> Result<Vec<VisualMesh>, FormatError> {
    let mut pages: Vec<i16> = polygons.iter().map(|p| p.texture).collect();
    pages.sort_unstable();
    pages.dedup();

    let mut meshes = Vec::new();
    for page in pages {
        let mut mesh = VisualMesh {
            name: name.to_string(),
            texture_page: page,
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            colors: Vec::new(),
            indices: Vec::new(),
        };
        for poly in polygons.iter().filter(|p| p.texture == page) {
            let corners = if poly.is_quad() { 4 } else { 3 };
            let base = mesh.positions.len() as u32;
            for corner in 0..corners {
                let index = poly.indices[corner] as usize;
                let vertex = vertices.get(index).ok_or_else(|| FormatError::Parse {
                    path: name.to_string(),
                    message: format!("índice de vértice {index} fuera de rango"),
                })?;
                let (position, normal) = match transform {
                    Some((matrix, translation)) => {
                        let local = mul_rows(*matrix, vertex.position);
                        let world = [
                            local[0] + translation[0],
                            local[1] + translation[1],
                            local[2] + translation[2],
                        ];
                        (
                            axes::position(world),
                            axes::direction(mul_rows(*matrix, vertex.normal)).normalize_or_zero(),
                        )
                    }
                    None => (
                        axes::position(vertex.position),
                        axes::direction(vertex.normal).normalize_or_zero(),
                    ),
                };
                mesh.positions.push(position);
                mesh.normals.push(normal);
                mesh.uvs.push(poly.uvs[corner]);
                mesh.colors.push(poly.colors[corner]);
            }
            // Negar X e Y conserva el sentido de la cara.
            if poly.is_quad() {
                mesh.indices
                    .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
            } else {
                mesh.indices.extend([base, base + 1, base + 2]);
            }
        }
        if !mesh.indices.is_empty() {
            meshes.push(mesh);
        }
    }
    Ok(meshes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vertex() -> RawVertex {
        RawVertex {
            position: [0.0, 0.0, 0.0],
            normal: [0.0, -1.0, 0.0],
        }
    }

    fn tri(colors: [[u8; 4]; 4]) -> RawPolygon {
        RawPolygon {
            flags: 0,
            texture: 0,
            indices: [0, 0, 0, 0],
            colors,
            uvs: [[0.0, 0.0]; 4],
        }
    }

    #[test]
    fn black_gouraud_faces_stay_in_the_mesh() {
        let polys = [tri([[0, 0, 0, 255]; 4]), tri([[255, 255, 255, 255]; 4])];
        let meshes = bake_meshes("t", &[vertex()], &polys, None).unwrap();
        let tris = meshes.iter().map(|m| m.indices.len() / 3).sum::<usize>();
        assert_eq!(tris, 2);
    }
}

fn mul_rows(rows: [[f32; 3]; 3], p: [f32; 3]) -> [f32; 3] {
    [
        rows[0][0] * p[0] + rows[1][0] * p[1] + rows[2][0] * p[2],
        rows[0][1] * p[0] + rows[1][1] * p[1] + rows[2][1] * p[2],
        rows[0][2] * p[0] + rows[1][2] * p[1] + rows[2][2] * p[2],
    ]
}
