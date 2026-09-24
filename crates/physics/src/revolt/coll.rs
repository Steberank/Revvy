//! Polígonos de colisión y pruebas de `newcoll.cpp` (versión PC final).

use glam::Vec3;

use super::math::{approx_equal, rot_trans_bbox, rot_trans_plane, sign, BBox, Mat, Plane};
use super::units::{COLL_EPSILON, SMALL_REAL};

pub const QUAD: u32 = 0x1;
pub const TWOSIDED: u32 = 0x2;
pub const OBJECT_ONLY: u32 = 0x4;
pub const CAMERA_ONLY: u32 = 0x8;
pub const NON_PLANAR: u32 = 0x10;
pub const NO_SKID: u32 = 0x20;

/// `NEWCOLLPOLY`.
#[derive(Clone, Debug)]
pub struct CollPoly {
    pub kind: u32,
    pub material: usize,
    pub plane: Plane,
    pub edges: [Plane; 4],
    pub bbox: BBox,
}

impl CollPoly {
    #[inline]
    pub fn is_quad(&self) -> bool {
        self.kind & QUAD != 0
    }

    #[inline]
    pub fn sides(&self) -> usize {
        if self.is_quad() {
            4
        } else {
            3
        }
    }

    #[inline]
    pub fn camera_only(&self) -> bool {
        self.kind & CAMERA_ONLY != 0
    }

    #[inline]
    pub fn object_only(&self) -> bool {
        self.kind & OBJECT_ONLY != 0
    }

    /// `RotTransPlane` sobre el plano y los bordes, y `RotTransBBox` (`BuildInstanceCollPolys`).
    pub fn rot_trans(&self, rot: &Mat, pos: Vec3) -> CollPoly {
        let mut edges = self.edges;
        for edge in edges.iter_mut().take(self.sides()) {
            *edge = rot_trans_plane(edge, rot, pos);
        }
        CollPoly {
            kind: self.kind,
            material: self.material,
            plane: rot_trans_plane(&self.plane, rot, pos),
            edges,
            bbox: rot_trans_bbox(&self.bbox, rot, pos),
        }
    }
}

/// Resultado de `SphereCollPoly`.
#[derive(Clone, Copy, Debug)]
pub struct SphereHit {
    /// Normal del contacto. En un borde es la dirección borde → centro, no la de la cara.
    pub plane: Plane,
    /// Punto de la esfera que toca, relativo al centro.
    pub rel_pos: Vec3,
    pub world_pos: Vec3,
    pub depth: f32,
    pub time: f32,
}

/// `SphereCollPoly` (PC): cara, o un solo borde. Un vértice no cuenta como choque.
pub fn sphere_coll_poly(old_pos: Vec3, new_pos: Vec3, radius: f32, poly: &CollPoly) -> Option<SphereHit> {
    let new_dist = poly.plane.dist(new_pos);
    if new_dist - radius > COLL_EPSILON {
        return None;
    }
    let old_dist = poly.plane.dist(old_pos);
    if old_dist < -(radius + COLL_EPSILON) {
        return None;
    }

    let sides = poly.sides();
    let mut dist = [0.0f32; 4];
    let mut inside = [false; 4];
    let mut out_count = 0;
    for i in 0..sides {
        dist[i] = poly.edges[i].dist(new_pos);
        inside[i] = dist[i] < 0.0;
        if !inside[i] {
            out_count += 1;
        }
    }

    if out_count == 0 {
        let depth = new_dist - radius;
        let time = if approx_equal(old_dist, new_dist) {
            0.0
        } else {
            ((old_dist - new_dist + depth) / (old_dist - new_dist)).clamp(0.0, 1.0)
        };
        let plane = poly.plane;
        return Some(SphereHit {
            plane,
            rel_pos: plane.n * -radius,
            world_pos: new_pos - plane.n * new_dist,
            depth,
            time,
        });
    }

    if out_count == 1 {
        let mut normal = Vec3::ZERO;
        let mut d_len = 0.0;
        let mut world_pos = Vec3::ZERO;
        let mut depth = 0.0;
        let mut time = 0.0;
        for i in 0..sides {
            if inside[i] {
                continue;
            }
            normal = poly.edges[i].n * -dist[i] - poly.plane.n * new_dist;
            d_len = normal.length();
            if d_len > radius {
                return None;
            }
            world_pos = normal + new_pos;
            depth = d_len - radius;
            time = if approx_equal(old_dist, new_dist) {
                0.0
            } else {
                old_dist / (old_dist - new_dist)
            };
            break;
        }
        if !poly.bbox.contains(world_pos) {
            return None;
        }
        for i in 0..sides {
            if inside[i] && poly.edges[i].dist(world_pos) > 0.0 {
                return None;
            }
        }
        let plane = if d_len > SMALL_REAL {
            Plane {
                n: normal / -d_len,
                d: poly.plane.d,
            }
        } else {
            poly.plane
        };
        return Some(SphereHit {
            plane,
            rel_pos: plane.n * -radius,
            world_pos,
            depth,
            time,
        });
    }

    None
}

/// `ModifyShift`: por componente, el mismo signo se queda con el mayor; distinto signo se suma.
pub fn modify_shift(shift: &mut Vec3, shift_mag: f32, normal: Vec3) {
    for i in 0..3 {
        let new_shift = shift_mag * normal[i];
        if sign(shift[i]) == sign(new_shift) {
            if shift[i].abs() < new_shift.abs() {
                shift[i] = new_shift;
            }
        } else {
            shift[i] += new_shift;
        }
    }
}

/// `LinePlaneIntersect`: `(t, depth)` si el segmento cruza el plano (con tolerancia).
pub fn line_plane_intersect(start: Vec3, end: Vec3, plane: &Plane) -> Option<(f32, f32)> {
    let s = plane.dist(start);
    let e = plane.dist(end);
    if sign(s) == sign(e) && s.abs() > COLL_EPSILON && e.abs() > COLL_EPSILON {
        return None;
    }
    let t = if approx_equal(s, e) { 0.0 } else { s / (s - e) };
    Some((t, e))
}

/// `PointInCollPolyBounds`.
pub fn point_in_coll_poly_bounds(p: Vec3, poly: &CollPoly) -> bool {
    poly.edges
        .iter()
        .take(poly.sides())
        .all(|edge| edge.dist(p) <= COLL_EPSILON)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cuadrado de 200×200 en y = 0 con la normal hacia arriba (−Y en Re-Volt).
    pub(crate) fn floor_quad() -> CollPoly {
        let up = Vec3::new(0.0, -1.0, 0.0);
        let edge = |n: Vec3, d: f32| Plane { n, d };
        CollPoly {
            kind: QUAD,
            material: 0,
            plane: Plane { n: up, d: 0.0 },
            edges: [
                edge(Vec3::new(-1.0, 0.0, 0.0), -100.0),
                edge(Vec3::new(1.0, 0.0, 0.0), -100.0),
                edge(Vec3::new(0.0, 0.0, -1.0), -100.0),
                edge(Vec3::new(0.0, 0.0, 1.0), -100.0),
            ],
            bbox: {
                let mut bbox = BBox {
                    min: Vec3::new(-100.0, 0.0, -100.0),
                    max: Vec3::new(100.0, 0.0, 100.0),
                };
                bbox.expand(COLL_EPSILON);
                bbox
            },
        }
    }

    #[test]
    fn sphere_resting_on_a_face_reports_its_depth() {
        let poly = floor_quad();
        let hit = sphere_coll_poly(Vec3::new(0.0, -12.0, 0.0), Vec3::new(0.0, -9.0, 0.0), 10.0, &poly)
            .expect("toca el piso");
        assert!((hit.depth + 1.0).abs() < 1e-5, "{hit:?}");
        assert_eq!(hit.plane.n, Vec3::new(0.0, -1.0, 0.0));
        assert!((hit.world_pos - Vec3::ZERO).length() < 1e-5);
    }

    #[test]
    fn sphere_far_above_does_not_collide() {
        let poly = floor_quad();
        assert!(sphere_coll_poly(Vec3::new(0.0, -50.0, 0.0), Vec3::new(0.0, -40.0, 0.0), 10.0, &poly).is_none());
    }

    #[test]
    fn shift_keeps_the_largest_push_per_axis() {
        let mut shift = Vec3::ZERO;
        modify_shift(&mut shift, 3.0, Vec3::new(0.0, -1.0, 0.0));
        modify_shift(&mut shift, 1.0, Vec3::new(0.0, -1.0, 0.0));
        assert_eq!(shift, Vec3::new(0.0, -3.0, 0.0));
        modify_shift(&mut shift, 2.0, Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(shift, Vec3::new(0.0, -1.0, 0.0));
    }
}
