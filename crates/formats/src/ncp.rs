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

#[derive(Clone, Debug)]
pub struct CollisionTri {
    pub positions: [Vec3; 3],
    pub surface: SurfaceType,
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
            push_tri(&mut triangles, corners[0], corners[3], corners[2], surface);
            push_tri(&mut triangles, corners[0], corners[2], corners[1], surface);
        } else {
            push_tri(&mut triangles, corners[0], corners[2], corners[1], surface);
        }
    }
    Ok(triangles)
}

fn push_tri(out: &mut Vec<CollisionTri>, a: Vec3, b: Vec3, c: Vec3, surface: SurfaceType) {
    if !a.is_finite() || !b.is_finite() || !c.is_finite() {
        return;
    }
    out.push(CollisionTri {
        positions: [a, b, c],
        surface,
    });
}

fn intersect(d1: f32, n1: Vec3, d2: f32, n2: Vec3, d3: f32, n3: Vec3) -> Option<Vec3> {
    let det = n1.dot(n2.cross(n3));
    if det.abs() < 1e-8 {
        return None;
    }
    Some((d1 * n2.cross(n3) + d2 * n3.cross(n1) + d3 * n1.cross(n2)) / det)
}
