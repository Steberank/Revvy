//! `Geom.cpp` / `Geom.h`: vectores, matrices por filas y cuaterniones de Re-Volt.
//!
//! Un `MAT` son tres filas: right, up y look del objeto en coordenadas de mundo.
//! `VecMulMat` lleva de local a mundo y `MatMulVec` de mundo a local. Los nombres
//! siguen a los del fuente para que el port se pueda leer al lado del original.

use glam::Vec3;

use super::units::{SIMILAR_REAL, SMALL_REAL};

/// `Sign` de `Util.h`: 0 para 0, a diferencia de `f32::signum`.
#[inline]
pub fn sign(x: f32) -> i32 {
    if x == 0.0 {
        0
    } else if x < 0.0 {
        -1
    } else {
        1
    }
}

/// `ApproxEqual` de `Util.h`.
#[inline]
pub fn approx_equal(a: f32, b: f32) -> bool {
    (a - b).abs() < SIMILAR_REAL
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat {
    pub r: Vec3,
    pub u: Vec3,
    pub l: Vec3,
}

pub const IDENTITY: Mat = Mat {
    r: Vec3::X,
    u: Vec3::Y,
    l: Vec3::Z,
};

pub const ZERO_MAT: Mat = Mat {
    r: Vec3::ZERO,
    u: Vec3::ZERO,
    l: Vec3::ZERO,
};

impl Mat {
    pub fn from_rows(rows: [[f32; 3]; 3]) -> Self {
        Self {
            r: Vec3::from(rows[0]),
            u: Vec3::from(rows[1]),
            l: Vec3::from(rows[2]),
        }
    }

    #[inline]
    pub fn row(&self, i: usize) -> Vec3 {
        match i {
            0 => self.r,
            1 => self.u,
            _ => self.l,
        }
    }

    #[inline]
    pub fn row_mut(&mut self, i: usize) -> &mut Vec3 {
        match i {
            0 => &mut self.r,
            1 => &mut self.u,
            _ => &mut self.l,
        }
    }

    /// `m.m[i * 3 + j]`.
    #[inline]
    pub fn get(&self, i: usize, j: usize) -> f32 {
        self.row(i)[j]
    }
}

/// `VecMulMat`: `v.x * R + v.y * U + v.z * L`.
#[inline]
pub fn vec_mul_mat(v: Vec3, m: &Mat) -> Vec3 {
    m.r * v.x + m.u * v.y + m.l * v.z
}

/// `MatMulVec`: proyecta sobre cada fila.
#[inline]
pub fn mat_mul_vec(m: &Mat, v: Vec3) -> Vec3 {
    Vec3::new(m.r.dot(v), m.u.dot(v), m.l.dot(v))
}

/// `MatMulMat`: producto usual de matrices por filas.
pub fn mat_mul_mat(a: &Mat, b: &Mat) -> Mat {
    Mat {
        r: vec_mul_mat(a.r, b),
        u: vec_mul_mat(a.u, b),
        l: vec_mul_mat(a.l, b),
    }
}

/// `TransMatMulMat`: `aᵀ · b`.
pub fn trans_mat_mul_mat(a: &Mat, b: &Mat) -> Mat {
    let mut out = ZERO_MAT;
    for i in 0..3 {
        let mut row = Vec3::ZERO;
        for j in 0..3 {
            row[j] = a.get(0, i) * b.get(0, j) + a.get(1, i) * b.get(1, j) + a.get(2, i) * b.get(2, j);
        }
        *out.row_mut(i) = row;
    }
    out
}

/// `GetFrameInertia`: `Tᵀ · I⁻¹ · T`.
pub fn get_frame_inertia(body_inv_inertia: &Mat, transform: &Mat) -> Mat {
    let tmp = mat_mul_mat(body_inv_inertia, transform);
    trans_mat_mul_mat(transform, &tmp)
}

/// `InvertMat`: Gauss-Jordan con pivote parcial. Una matriz singular queda en cero.
pub fn invert_mat(mat: &Mat) -> Mat {
    let mut a = [mat.r, mat.u, mat.l];
    let mut b = [Vec3::X, Vec3::Y, Vec3::Z];
    for j in 0..3 {
        let mut pivot = j;
        for i in (j + 1)..3 {
            if a[i][j].abs() > a[pivot][j].abs() {
                pivot = i;
            }
        }
        a.swap(pivot, j);
        b.swap(pivot, j);
        if a[j][j] == 0.0 {
            return ZERO_MAT;
        }
        let d = a[j][j];
        b[j] /= d;
        a[j] /= d;
        for i in 0..3 {
            if i != j {
                let t = a[i][j];
                b[i] -= b[j] * t;
                a[i] -= a[j] * t;
            }
        }
    }
    Mat {
        r: b[0],
        u: b[1],
        l: b[2],
    }
}

/// `VecCrossMat`.
pub fn vec_cross_mat(v: Vec3, m: &Mat) -> Mat {
    Mat {
        r: Vec3::new(
            v.y * m.get(2, 0) - v.z * m.get(1, 0),
            v.y * m.get(2, 1) - v.z * m.get(1, 1),
            v.y * m.get(2, 2) - v.z * m.get(1, 2),
        ),
        u: Vec3::new(
            v.z * m.get(0, 0) - v.x * m.get(2, 0),
            v.z * m.get(0, 1) - v.x * m.get(2, 1),
            v.z * m.get(0, 2) - v.x * m.get(2, 2),
        ),
        l: Vec3::new(
            v.x * m.get(1, 0) - v.y * m.get(0, 0),
            v.x * m.get(1, 1) - v.y * m.get(0, 1),
            v.x * m.get(1, 2) - v.y * m.get(0, 2),
        ),
    }
}

/// `MatCrossVec`.
pub fn mat_cross_vec(m: &Mat, v: Vec3) -> Mat {
    let row = |i: usize| {
        Vec3::new(
            -v.y * m.get(i, 2) + v.z * m.get(i, 1),
            -v.z * m.get(i, 0) + v.x * m.get(i, 2),
            -v.x * m.get(i, 1) + v.y * m.get(i, 0),
        )
    };
    Mat {
        r: row(0),
        u: row(1),
        l: row(2),
    }
}

/// `RotationX`.
pub fn rotation_x(rot: f32) -> Mat {
    let (s, c) = rot.sin_cos();
    Mat {
        r: Vec3::new(1.0, 0.0, 0.0),
        u: Vec3::new(0.0, c, -s),
        l: Vec3::new(0.0, s, c),
    }
}

/// `RotationY`.
pub fn rotation_y(rot: f32) -> Mat {
    let (s, c) = rot.sin_cos();
    Mat {
        r: Vec3::new(c, 0.0, s),
        u: Vec3::new(0.0, 1.0, 0.0),
        l: Vec3::new(-s, 0.0, c),
    }
}

/// `RotMatrixY`: el ángulo está en vueltas.
pub fn rot_matrix_y(turns: f32) -> Mat {
    rotation_y(turns * std::f32::consts::TAU)
}

/// `BuildLookMatrixForward`: sin roll, el right queda horizontal.
pub fn build_look_matrix_forward(pos: Vec3, look: Vec3) -> Mat {
    let l = (look - pos).normalize_or_zero();
    let r = Vec3::new(l.z, 0.0, -l.x).normalize_or_zero();
    let u = l.cross(r);
    Mat { r, u, l }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    pub n: Vec3,
    pub d: f32,
}

impl Plane {
    pub fn from_array(v: [f32; 4]) -> Self {
        Self {
            n: Vec3::new(v[0], v[1], v[2]),
            d: v[3],
        }
    }

    /// `VecDotPlane`.
    #[inline]
    pub fn dist(&self, p: Vec3) -> f32 {
        p.dot(self.n) + self.d
    }
}

/// `RotTransPlane`.
pub fn rot_trans_plane(plane: &Plane, rot: &Mat, dr: Vec3) -> Plane {
    let n = vec_mul_mat(plane.n, rot);
    Plane {
        n,
        d: plane.d - dr.dot(n),
    }
}

/// `QUATERNION`: `v` es (VX, VY, VZ) y `s` el escalar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quat {
    pub v: Vec3,
    pub s: f32,
}

impl Quat {
    pub const IDENTITY: Self = Self {
        v: Vec3::ZERO,
        s: 1.0,
    };

    pub fn dot(&self, other: &Quat) -> f32 {
        self.v.dot(other.v) + self.s * other.s
    }

    /// `NormalizeQuat`: no toca un cuaternión casi nulo.
    pub fn normalize(&mut self) {
        let len = self.dot(self).sqrt();
        if len > SMALL_REAL {
            let inv = 1.0 / len;
            self.v *= inv;
            self.s *= inv;
        }
    }

    pub fn negate(&mut self) {
        self.v = -self.v;
        self.s = -self.s;
    }
}

/// `VecMulQuat`: el vector como cuaternión puro por la izquierda.
pub fn vec_mul_quat(v: Vec3, q: &Quat) -> Quat {
    Quat {
        v: Vec3::new(
            v.x * q.s + v.y * q.v.z - v.z * q.v.y,
            v.y * q.s + v.z * q.v.x - v.x * q.v.z,
            v.z * q.s + v.x * q.v.y - v.y * q.v.x,
        ),
        s: -v.x * q.v.x - v.y * q.v.y - v.z * q.v.z,
    }
}

/// `QuatToMat`.
pub fn quat_to_mat(q: &Quat) -> Mat {
    let (x, y, z, s) = (q.v.x, q.v.y, q.v.z, q.s);
    let xx = 1.0 - 2.0 * (y * y + z * z);
    let yy = 1.0 - 2.0 * (x * x + z * z);
    let zz = 1.0 - 2.0 * (x * x + y * y);
    let yx = 2.0 * (x * y - s * z);
    let xy = 2.0 * (x * y + s * z);
    let zx = 2.0 * (x * z + s * y);
    let xz = 2.0 * (x * z - s * y);
    let zy = 2.0 * (y * z - s * x);
    let yz = 2.0 * (y * z + s * x);
    Mat {
        r: Vec3::new(xx, xy, xz),
        u: Vec3::new(yx, yy, yz),
        l: Vec3::new(zx, zy, zz),
    }
}

/// `MatToQuat`.
pub fn mat_to_quat(m: &Mat) -> Quat {
    let tr = m.get(0, 0) + m.get(1, 1) + m.get(2, 2);
    if tr >= 0.0 {
        let s = (tr + 1.0).sqrt();
        let w = 0.5 * s;
        let s = 0.5 / s;
        return Quat {
            v: Vec3::new(
                (m.get(1, 2) - m.get(2, 1)) * s,
                (m.get(2, 0) - m.get(0, 2)) * s,
                (m.get(0, 1) - m.get(1, 0)) * s,
            ),
            s: w,
        };
    }
    let mut i = 0;
    if m.get(1, 1) > m.get(0, 0) {
        i = 1;
    }
    if m.get(2, 2) > m.get(i, i) {
        i = 2;
    }
    match i {
        0 => {
            let s = ((m.get(0, 0) - (m.get(1, 1) + m.get(2, 2))) + 1.0).sqrt();
            let x = 0.5 * s;
            let s = 0.5 / s;
            Quat {
                v: Vec3::new(
                    x,
                    (m.get(1, 0) + m.get(0, 1)) * s,
                    (m.get(0, 2) + m.get(2, 0)) * s,
                ),
                s: (m.get(1, 2) - m.get(2, 1)) * s,
            }
        }
        1 => {
            let s = ((m.get(1, 1) - (m.get(2, 2) + m.get(0, 0))) + 1.0).sqrt();
            let y = 0.5 * s;
            let s = 0.5 / s;
            Quat {
                v: Vec3::new(
                    (m.get(1, 0) + m.get(0, 1)) * s,
                    y,
                    (m.get(2, 1) + m.get(1, 2)) * s,
                ),
                s: (m.get(2, 0) - m.get(0, 2)) * s,
            }
        }
        _ => {
            let s = ((m.get(2, 2) - (m.get(0, 0) + m.get(1, 1))) + 1.0).sqrt();
            let z = 0.5 * s;
            let s = 0.5 / s;
            Quat {
                v: Vec3::new(
                    (m.get(0, 2) + m.get(2, 0)) * s,
                    (m.get(2, 1) + m.get(1, 2)) * s,
                    z,
                ),
                s: (m.get(0, 1) - m.get(1, 0)) * s,
            }
        }
    }
}

/// `SLerpQuat`. `acos` fuera de [-1, 1] vale 0, como el `_matherr` del fuente.
pub fn slerp_quat(q0: &Quat, q1: &Quat, t: f32) -> Quat {
    let dot = q0.dot(q1);
    let theta = if (-1.0..=1.0).contains(&dot) {
        dot.acos()
    } else {
        0.0
    };
    let (a, b) = if theta.abs() < 0.01 {
        (1.0 - t, t)
    } else {
        let sin_t = theta.sin();
        (((1.0 - t) * theta).sin() / sin_t, (t * theta).sin() / sin_t)
    };
    Quat {
        v: q0.v * a + q1.v * b,
        s: q0.s * a + q1.s * b,
    }
}

/// `GoodWrap`.
pub fn good_wrap(value: f32, min: f32, max: f32) -> f32 {
    let range = max - min;
    if value < min {
        let n = ((min - value) / range) as i32;
        value + range * (n + 1) as f32
    } else if value > max {
        let n = ((value - max) / range) as i32;
        value - range * (n + 1) as f32
    } else {
        value
    }
}

/// `BBOX`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BBox {
    pub min: Vec3,
    pub max: Vec3,
}

impl BBox {
    /// Caja inválida para ir sumando puntos (`LARGEDIST` al revés).
    pub const EMPTY: Self = Self {
        min: Vec3::splat(super::units::LARGEDIST),
        max: Vec3::splat(-super::units::LARGEDIST),
    };

    pub fn from_array(v: [f32; 6]) -> Self {
        Self {
            min: Vec3::new(v[0], v[2], v[4]),
            max: Vec3::new(v[1], v[3], v[5]),
        }
    }

    /// `BBTestXZY` / `BBTestYXZ`: el orden de las pruebas no cambia el resultado.
    #[inline]
    pub fn overlaps(&self, other: &BBox) -> bool {
        !(self.min.x > other.max.x
            || self.max.x < other.min.x
            || self.min.z > other.max.z
            || self.max.z < other.min.z
            || self.min.y > other.max.y
            || self.max.y < other.min.y)
    }

    /// `PointInBBox`.
    #[inline]
    pub fn contains(&self, p: Vec3) -> bool {
        !(self.min.x > p.x
            || self.max.x < p.x
            || self.min.z > p.z
            || self.max.z < p.z
            || self.min.y > p.y
            || self.max.y < p.y)
    }

    /// `AddPosRadToBBox`.
    pub fn add_pos_rad(&mut self, p: Vec3, radius: f32) {
        self.min = self.min.min(p - Vec3::splat(radius));
        self.max = self.max.max(p + Vec3::splat(radius));
    }

    /// `AddPointToBBox`.
    pub fn add_point(&mut self, p: Vec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }

    /// `ExpandBBox`.
    pub fn expand(&mut self, delta: f32) {
        self.min -= Vec3::splat(delta);
        self.max += Vec3::splat(delta);
    }
}

/// `RotTransBBox`: las ocho esquinas al marco nuevo y vuelta a una caja alineada.
pub fn rot_trans_bbox(src: &BBox, mat: &Mat, pos: Vec3) -> BBox {
    let mut out = BBox::EMPTY;
    for i in 0..8 {
        let p = Vec3::new(
            if i & 4 != 0 { src.max.x } else { src.min.x },
            if i & 2 != 0 { src.max.y } else { src.min.y },
            if i & 1 != 0 { src.max.z } else { src.min.z },
        );
        out.add_point(vec_mul_mat(p, mat) + pos);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-4
    }

    #[test]
    fn quat_round_trips_through_matrix() {
        let m = mat_mul_mat(&rotation_x(0.3), &rotation_y(-1.1));
        let q = mat_to_quat(&m);
        let back = quat_to_mat(&q);
        assert!(close(m.r, back.r) && close(m.u, back.u) && close(m.l, back.l), "{m:?} {back:?}");
    }

    #[test]
    fn inverse_times_matrix_is_identity() {
        let m = Mat::from_rows([[2000.0, 0.0, 0.0], [0.0, 2480.0, 0.0], [0.0, 0.0, 705.0]]);
        let p = mat_mul_mat(&m, &invert_mat(&m));
        assert!(close(p.r, Vec3::X) && close(p.u, Vec3::Y) && close(p.l, Vec3::Z));
    }

    #[test]
    fn frame_inertia_of_identity_is_the_body_inertia() {
        let inv = Mat::from_rows([[1.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 3.0]]);
        assert_eq!(get_frame_inertia(&inv, &IDENTITY), inv);
    }

    #[test]
    fn wrap_keeps_the_angle_in_range() {
        let tau = std::f32::consts::TAU;
        assert!((good_wrap(7.0, 0.0, tau) - (7.0 - tau)).abs() < 1e-5);
        assert!((good_wrap(-1.0, 0.0, tau) - (tau - 1.0)).abs() < 1e-5);
    }
}
