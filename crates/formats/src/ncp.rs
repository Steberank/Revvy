//! Colisión `.ncp`. Cada poliedro se reconstruye como triángulos con `SurfaceType`.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use glam::Vec3;

use crate::axes;
use crate::binutil::Reader;
use crate::layout::SurfaceType;
use crate::FormatError;

const NCP_QUAD: u32 = 1;
/// `OBJECT_ONLY` y `CAMERA_ONLY` del tipo de polígono.
const NCP_OBJECT_ONLY: u32 = 4;
const NCP_CAMERA_ONLY: u32 = 8;

#[derive(Clone, Debug)]
pub struct CollisionTri {
    /// Antihorario visto desde el frente: el lado de donde se choca.
    pub positions: [Vec3; 3],
    pub surface: SurfaceType,
    /// Solo frena la cámara: los autos lo atraviesan.
    pub camera_only: bool,
    /// Solo lo tocan los autos: la cámara lo atraviesa.
    pub object_only: bool,
}

pub fn parse(path: &Path) -> Result<Vec<CollisionTri>, FormatError> {
    let file = File::open(path).map_err(|err| FormatError::io(path, err))?;
    if file.metadata().map(|meta| meta.len()).unwrap_or(0) < 2 {
        return Ok(Vec::new());
    }
    let mut reader = Reader::new(BufReader::new(file));
    let count = reader.u16()? as usize;
    let mut triangles = Vec::new();
    for _ in 0..count {
        let kind = reader.u32()?;
        let material = reader.u32()?;
        let surface = SurfaceType::from_revolt(material);
        let flags = (kind & NCP_CAMERA_ONLY != 0, kind & NCP_OBJECT_ONLY != 0);
        let mut normals = [Vec3::ZERO; 5];
        let mut distances = [0.0; 5];
        for plane in 0..5 {
            let normal = axes::direction(reader.v3()?);
            normals[plane] = if normal.length_squared() > 0.0 {
                normal.normalize()
            } else {
                normal
            };
            // El addon niega la distancia. El eje Y ya quedó en el normal.
            distances[plane] = -reader.f32()? * axes::REVOLT_TO_METERS;
        }
        let xlo = reader.f32()?;
        let xhi = reader.f32()?;
        let ylo = reader.f32()?;
        let yhi = reader.f32()?;
        let zlo = reader.f32()?;
        let zhi = reader.f32()?;
        let scale = axes::REVOLT_TO_METERS;
        let x0 = (-xlo * scale).min(-xhi * scale);
        let x1 = (-xlo * scale).max(-xhi * scale);
        let y0 = (-ylo * scale).min(-yhi * scale);
        let y1 = (-ylo * scale).max(-yhi * scale);
        let bounds_min = Vec3::new(x0, y0, zlo.min(zhi) * scale);
        let bounds_max = Vec3::new(x1, y1, zlo.max(zhi) * scale);

        let quad = kind & NCP_QUAD != 0;
        let corners = if quad {
            vec![
                intersect(
                    distances[0],
                    normals[0],
                    distances[1],
                    normals[1],
                    distances[2],
                    normals[2],
                ),
                intersect(
                    distances[0],
                    normals[0],
                    distances[2],
                    normals[2],
                    distances[3],
                    normals[3],
                ),
                intersect(
                    distances[0],
                    normals[0],
                    distances[3],
                    normals[3],
                    distances[4],
                    normals[4],
                ),
                intersect(
                    distances[0],
                    normals[0],
                    distances[4],
                    normals[4],
                    distances[1],
                    normals[1],
                ),
            ]
        } else {
            vec![
                intersect(
                    distances[0],
                    normals[0],
                    distances[1],
                    normals[1],
                    distances[2],
                    normals[2],
                ),
                intersect(
                    distances[0],
                    normals[0],
                    distances[2],
                    normals[2],
                    distances[3],
                    normals[3],
                ),
                intersect(
                    distances[0],
                    normals[0],
                    distances[3],
                    normals[3],
                    distances[1],
                    normals[1],
                ),
            ]
        };
        if corners.iter().any(|c| c.is_none()) {
            continue;
        }
        let corners: Vec<Vec3> = corners.into_iter().flatten().collect();
        // Una intersección mal condicionada sale del bbox y se vuelve un muro invisible.
        let margin = 0.2;
        if corners.iter().any(|corner| {
            corner.cmplt(bounds_min - Vec3::splat(margin)).any()
                || corner.cmpgt(bounds_max + Vec3::splat(margin)).any()
        }) {
            continue;
        }
        if quad {
            push_tri(&mut triangles, [corners[0], corners[3], corners[2]], normals[0], surface, flags);
            push_tri(&mut triangles, [corners[0], corners[2], corners[1]], normals[0], surface, flags);
        } else {
            push_tri(&mut triangles, [corners[0], corners[2], corners[1]], normals[0], surface, flags);
        }
    }
    Ok(triangles)
}

/// `NEWCOLLPOLY` tal cual está en el archivo: espacio de Re-Volt, sin girar
/// ejes ni pasar a metros. La física portada de Re-Volt trabaja con esto.
#[derive(Clone, Debug)]
pub struct NcpPoly {
    pub kind: u32,
    pub material: u32,
    /// Normal (a, b, c) y d: la distancia de un punto es `n·p + d`.
    pub plane: [f32; 4],
    pub edges: [[f32; 4]; 4],
    /// XMin, XMax, YMin, YMax, ZMin, ZMax.
    pub bbox: [f32; 6],
}

/// `COLLGRID_DATA` y la lista de polígonos de cada celda, como la guarda el `.ncp` del mundo.
#[derive(Clone, Debug)]
pub struct NcpGrid {
    pub x_start: f32,
    pub z_start: f32,
    pub x_num: f32,
    pub z_num: f32,
    pub grid_size: f32,
    pub cells: Vec<Vec<u32>>,
}

#[derive(Clone, Debug, Default)]
pub struct NcpFile {
    pub polys: Vec<NcpPoly>,
    pub grid: Option<NcpGrid>,
}

/// `LoadNewCollPolys` + `LoadGridInfo`. El `.ncp` de una instancia no trae grilla.
pub fn parse_native(path: &Path) -> Result<NcpFile, FormatError> {
    let bytes = std::fs::read(path).map_err(|err| FormatError::io(path, err))?;
    if bytes.len() < 2 {
        return Ok(NcpFile::default());
    }
    let mut reader = Reader::new(bytes.as_slice());
    let count = reader.u16()? as usize;
    let mut polys = Vec::with_capacity(count);
    for _ in 0..count {
        let kind = reader.u32()?;
        let material = reader.u32()?;
        let mut plane = [0.0; 4];
        for value in &mut plane {
            *value = reader.f32()?;
        }
        let mut edges = [[0.0; 4]; 4];
        for edge in &mut edges {
            for value in edge.iter_mut() {
                *value = reader.f32()?;
            }
        }
        let mut bbox = [0.0; 6];
        for value in &mut bbox {
            *value = reader.f32()?;
        }
        polys.push(NcpPoly {
            kind,
            material,
            plane,
            edges,
            bbox,
        });
    }
    let consumed = 2 + count * 112;
    let grid = if bytes.len() >= consumed + 20 {
        Some(parse_grid(path, &mut reader, count)?)
    } else {
        None
    };
    Ok(NcpFile { polys, grid })
}

fn parse_grid(path: &Path, reader: &mut Reader<&[u8]>, polys: usize) -> Result<NcpGrid, FormatError> {
    let x_start = reader.f32()?;
    let z_start = reader.f32()?;
    let x_num = reader.f32()?;
    let z_num = reader.f32()?;
    let grid_size = reader.f32()?;
    let cell_count = (x_num.round() as i64 * z_num.round() as i64).max(0) as usize;
    let mut cells = Vec::with_capacity(cell_count);
    for _ in 0..cell_count {
        let n = reader.i32()?;
        if n < 0 {
            return Err(FormatError::parse(path, "celda de grilla con cantidad negativa"));
        }
        let mut cell = Vec::with_capacity(n as usize);
        for _ in 0..n {
            let index = reader.i32()?;
            if index < 0 || index as usize >= polys {
                return Err(FormatError::parse(path, format!("índice de grilla {index} fuera de rango")));
            }
            cell.push(index as u32);
        }
        cells.push(cell);
    }
    Ok(NcpGrid {
        x_start,
        z_start,
        x_num,
        z_num,
        grid_size,
        cells,
    })
}

/// El frente del triángulo es el lado del plano del polígono: el orden de los vértices
/// queda antihorario visto desde ahí, como en una `.glb`.
fn push_tri(out: &mut Vec<CollisionTri>, [a, b, mut c]: [Vec3; 3], front: Vec3, surface: SurfaceType, flags: (bool, bool)) {
    if !a.is_finite() || !b.is_finite() || !c.is_finite() {
        return;
    }
    let mut b = b;
    if (b - a).cross(c - a).dot(front) < 0.0 {
        std::mem::swap(&mut b, &mut c);
    }
    out.push(CollisionTri {
        positions: [a, b, c],
        surface,
        camera_only: flags.0,
        object_only: flags.1,
    });
}

fn intersect(d1: f32, n1: Vec3, d2: f32, n2: Vec3, d3: f32, n3: Vec3) -> Option<Vec3> {
    let det = n1.dot(n2.cross(n3));
    if det.abs() < 1e-8 {
        return None;
    }
    Some((d1 * n2.cross(n3) + d2 * n3.cross(n1) + d3 * n1.cross(n2)) / det)
}
