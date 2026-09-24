//! Mundo de colisión: `LoadNewCollPolys`, `BuildInstanceCollPolys`, `LoadGridInfo`,
//! `InitCollGrid`, `PosToCollGrid` y `LineOfSight`.

use glam::Vec3;
use revvy_formats::{LegacyLevel, NcpPoly};

use super::coll::{self, CollPoly, CAMERA_ONLY, OBJECT_ONLY};
use super::math::{BBox, Mat, Plane};
use super::material::material_index;
use super::units::{COLLGRID_EXPAND, COLL_EPSILON, LARGEDIST};

/// `INSTANCE_NO_OBJECT_COLLISION` / `INSTANCE_NO_CAMERA_COLLISION`.
const INSTANCE_NO_OBJECT_COLLISION: u8 = 32;
const INSTANCE_NO_CAMERA_COLLISION: u8 = 64;

#[derive(Clone, Debug)]
pub struct CollGridData {
    pub x_start: f32,
    pub z_start: f32,
    pub x_num: f32,
    pub z_num: f32,
    pub grid_size: f32,
}

#[derive(Clone, Debug)]
pub struct CollWorld {
    /// Primero los del `.ncp` del mundo y después los de las instancias.
    pub polys: Vec<CollPoly>,
    pub n_world_polys: usize,
    pub grid: CollGridData,
    /// Índices de cada celda, ordenados por `BBox.YMin` como `InitCollGrid`.
    pub cells: Vec<Vec<u32>>,
}

/// `LoadNewCollPolys`: la caja del polígono crece `COLL_EPSILON`.
fn load_poly(raw: &NcpPoly) -> CollPoly {
    let mut bbox = BBox::from_array(raw.bbox);
    bbox.expand(COLL_EPSILON);
    CollPoly {
        kind: raw.kind,
        material: material_index(raw.material),
        plane: Plane::from_array(raw.plane),
        edges: raw.edges.map(Plane::from_array),
        bbox,
    }
}

impl CollWorld {
    pub fn new(level: &LegacyLevel) -> Self {
        let mut polys: Vec<CollPoly> = level.world.polys.iter().map(load_poly).collect();
        let n_world_polys = polys.len();

        // `BuildInstanceCollPolys`. Con las instancias encendidas no se filtra por prioridad.
        for (instance, model_polys) in &level.instances {
            let rot = Mat::from_rows(instance.matrix);
            let pos = Vec3::from(instance.position);
            for raw in model_polys {
                let mut poly = load_poly(raw).rot_trans(&rot, pos);
                if instance.flag & INSTANCE_NO_OBJECT_COLLISION != 0 {
                    poly.kind |= CAMERA_ONLY;
                }
                if instance.flag & INSTANCE_NO_CAMERA_COLLISION != 0 {
                    poly.kind |= OBJECT_ONLY;
                }
                polys.push(poly);
            }
        }

        let (grid, mut cells) = match &level.world.grid {
            Some(grid) => {
                let data = CollGridData {
                    x_start: grid.x_start,
                    z_start: grid.z_start,
                    x_num: grid.x_num,
                    z_num: grid.z_num,
                    grid_size: grid.grid_size,
                };
                let x_count = grid.x_num.round().max(0.0) as usize;
                let mut cells = Vec::with_capacity(grid.cells.len());
                for (index, world_cell) in grid.cells.iter().enumerate() {
                    let (xi, zi) = if x_count == 0 {
                        (0, 0)
                    } else {
                        (index % x_count, index / x_count)
                    };
                    let x1 = grid.x_start + xi as f32 * grid.grid_size;
                    let z1 = grid.z_start + zi as f32 * grid.grid_size;
                    let cell_box = BBox {
                        min: Vec3::new(x1 - COLLGRID_EXPAND, -LARGEDIST, z1 - COLLGRID_EXPAND),
                        max: Vec3::new(
                            x1 + grid.grid_size + COLLGRID_EXPAND,
                            LARGEDIST,
                            z1 + grid.grid_size + COLLGRID_EXPAND,
                        ),
                    };
                    let mut cell = world_cell.clone();
                    for (i, poly) in polys.iter().enumerate().skip(n_world_polys) {
                        if poly.bbox.overlaps(&cell_box) {
                            cell.push(i as u32);
                        }
                    }
                    cells.push(cell);
                }
                (data, cells)
            }
            None => (
                CollGridData {
                    x_start: 0.0,
                    z_start: 0.0,
                    x_num: 0.0,
                    z_num: 0.0,
                    grid_size: LARGEDIST,
                },
                vec![(0..n_world_polys as u32).collect()],
            ),
        };

        // `InitCollGrid`: de abajo… de menor YMin a mayor (el burbujeo es estable).
        for cell in &mut cells {
            cell.sort_by(|a, b| {
                polys[*a as usize]
                    .bbox
                    .min
                    .y
                    .partial_cmp(&polys[*b as usize].bbox.min.y)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        Self {
            polys,
            n_world_polys,
            grid,
            cells,
        }
    }

    fn gridded(&self) -> bool {
        !(self.grid.x_num == 0.0 && self.grid.z_num == 0.0)
    }

    fn x_count(&self) -> i64 {
        (self.grid.x_num + 0.5) as i64
    }

    fn z_count(&self) -> i64 {
        (self.grid.z_num + 0.5) as i64
    }

    /// `PosToCollGridCoords`: (celda o −1, x, z). La división trunca hacia cero, como `Int`.
    pub fn grid_coords(&self, pos: Vec3) -> (i64, i64, i64) {
        if !self.gridded() {
            return (0, 0, 0);
        }
        let off_x = ((pos.x - self.grid.x_start) / self.grid.grid_size) as i64;
        let off_z = ((pos.z - self.grid.z_start) / self.grid.grid_size) as i64;
        if off_x < 0 || off_x >= self.x_count() || off_z < 0 || off_z >= self.z_count() {
            return (-1, off_x, off_z);
        }
        (off_x + self.x_count() * off_z, off_x, off_z)
    }

    /// `PosToCollGrid`.
    pub fn grid_for(&self, pos: Vec3) -> Option<&[u32]> {
        let (num, _, _) = self.grid_coords(pos);
        if num < 0 {
            return None;
        }
        self.cells.get(num as usize).map(Vec::as_slice)
    }

    /// `LineOfSight`: recorre las celdas entre los dos puntos.
    pub fn line_of_sight(&self, src: Vec3, dest: Vec3) -> bool {
        let dr = dest - src;
        let (mut grid_num, mut x_grid, mut z_grid) = self.grid_coords(src);
        let (end_grid_num, _, _) = self.grid_coords(dest);
        let mut last_grid = false;
        loop {
            if x_grid >= self.x_count().max(1) || z_grid >= self.z_count().max(1) || x_grid < 0 || z_grid < 0 {
                return true;
            }
            let Some(cell) = (grid_num >= 0).then(|| self.cells.get(grid_num as usize)).flatten() else {
                return true;
            };
            for &index in cell {
                let poly = &self.polys[index as usize];
                if poly.camera_only() || poly.object_only() {
                    continue;
                }
                let Some((t, _)) = coll::line_plane_intersect(src, dest, &poly.plane) else {
                    continue;
                };
                if coll::point_in_coll_poly_bounds(src + dr * t, poly) {
                    return false;
                }
            }
            if grid_num == end_grid_num || !self.gridded() {
                last_grid = true;
            } else {
                let mut min_t = 2.0;
                let (mut dx, mut dz) = (0, 0);
                let size = self.grid.grid_size;
                let face_x = if dr.x < 0.0 {
                    Plane {
                        n: Vec3::X,
                        d: -(self.grid.x_start + x_grid as f32 * size),
                    }
                } else {
                    Plane {
                        n: -Vec3::X,
                        d: self.grid.x_start + (x_grid + 1) as f32 * size,
                    }
                };
                if let Some((t, _)) = coll::line_plane_intersect(src, dest, &face_x) {
                    if t < min_t {
                        min_t = t;
                        dx = if dr.x < 0.0 { -1 } else { 1 };
                        dz = 0;
                    }
                }
                let face_z = if dr.z < 0.0 {
                    Plane {
                        n: Vec3::Z,
                        d: -(self.grid.z_start + z_grid as f32 * size),
                    }
                } else {
                    Plane {
                        n: -Vec3::Z,
                        d: self.grid.z_start + (z_grid + 1) as f32 * size,
                    }
                };
                if let Some((t, _)) = coll::line_plane_intersect(src, dest, &face_z) {
                    if t < min_t {
                        dx = 0;
                        dz = if dr.z < 0.0 { -1 } else { 1 };
                    }
                }
                if dx == 0 && dz == 0 {
                    return true;
                }
                x_grid += dx;
                z_grid += dz;
                if x_grid as f32 >= self.grid.x_num || z_grid as f32 >= self.grid.z_num {
                    grid_num = -1;
                } else {
                    grid_num += dx + self.x_count() * dz;
                }
            }
            if grid_num == -1 || last_grid {
                return true;
            }
        }
    }
}
