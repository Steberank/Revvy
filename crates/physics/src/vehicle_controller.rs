//! Auto controlado por el jugador: la física portada de Re-Volt (`crate::revolt`) y la
//! conversión entre el espacio de Re-Volt (unidades de 5 mm, Y abajo) y el de Revvy
//! (m, Y arriba).
//!
//! Negar X e Y es un giro de 180° alrededor de Z (`revvy_formats` hace lo mismo con
//! las mallas), así que una matriz de Re-Volt se lleva a Revvy conjugándola con
//! `diag(-1, -1, 1)`.

use glam::{Mat3, Mat4, Vec3};

use crate::revolt::math::Mat;

pub use crate::revolt::{Car, CollWorld, Controls, FollowCamera, FrameReport, SfxState, Simulation};

/// El mismo factor que usa `revvy-formats` para las mallas.
pub use revvy_formats::REVOLT_TO_METERS;

/// Punto del espacio de Re-Volt a Revvy.
pub fn to_revvy_point(p: Vec3) -> Vec3 {
    Vec3::new(-p.x, -p.y, p.z) * REVOLT_TO_METERS
}

/// Dirección del espacio de Re-Volt a Revvy (sin escala).
pub fn to_revvy_dir(v: Vec3) -> Vec3 {
    Vec3::new(-v.x, -v.y, v.z)
}

/// Punto de Revvy al espacio de Re-Volt.
pub fn to_revolt_point(p: Vec3) -> Vec3 {
    Vec3::new(-p.x, -p.y, p.z) / REVOLT_TO_METERS
}

/// Dirección de Revvy al espacio de Re-Volt.
pub fn to_revolt_dir(v: Vec3) -> Vec3 {
    Vec3::new(-v.x, -v.y, v.z)
}

/// Rotación de un `MAT` de Re-Volt en Revvy: `C · [R U L] · C` con `C = diag(-1, -1, 1)`.
/// Las filas R, U, L son columnas y la conjugación niega las dos primeras.
pub fn to_revvy_rotation(m: &Mat) -> Mat3 {
    Mat3::from_cols(-to_revvy_dir(m.r), -to_revvy_dir(m.u), to_revvy_dir(m.l))
}

/// Transformación de un modelo de Re-Volt (`DrawModel(model, mat, pos)`) para dibujar
/// mallas que `revvy_formats` ya convirtió a Revvy.
pub fn model_matrix(m: &Mat, pos: Vec3) -> Mat4 {
    Mat4::from_mat3_translation(to_revvy_rotation(m), to_revvy_point(pos))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::revolt::math::{rotation_y, vec_mul_mat};

    #[test]
    fn model_matrix_matches_converting_the_transformed_point() {
        let m = rotation_y(0.7);
        let pos = Vec3::new(-1500.0, 90.0, 2000.0);
        let local = Vec3::new(24.8, -1.0, 45.4);
        let revolt = vec_mul_mat(local, &m) + pos;
        let revvy_local = to_revvy_point(local);
        let drawn = model_matrix(&m, pos).transform_point3(revvy_local);
        assert!((drawn - to_revvy_point(revolt)).length() < 1e-4, "{drawn:?}");
    }

    #[test]
    fn points_round_trip() {
        let p = Vec3::new(1.0, -2.0, 3.0);
        assert!((to_revolt_point(to_revvy_point(p)) - p).length() < 1e-4);
    }
}
