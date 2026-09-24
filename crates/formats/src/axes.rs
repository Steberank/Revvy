//! Re-Volt es diestro con Y hacia abajo, +X a la derecha y +Z adelante
//! (`DownVec`, `LookVec` en `Geom.cpp`). El espacio interno es Y-up.
//! Negar solo Y lo vuelve zurdo y el mapa sale espejado: la primera curva
//! de nhood1 queda a la izquierda. Negar X e Y es un giro, y la derecha sigue
//! siendo la derecha. Una unidad de Re-Volt mide 5 mm.

use glam::{Mat3, Quat, Vec3};

/// Una unidad de Re-Volt en metros. El velocímetro de `units.h` convierte a km/h con
/// `OGU2KPH_SPEED` 0.018 y a mph con `OGU2MPH_SPEED` 0.01118: las dos dan 0.005 m/s
/// por unidad/s. Así la gravedad de Re-Volt (2200) es 11 m/s².
pub const REVOLT_TO_METERS: f32 = 0.005;

/// `MPH2OGU_SPEED`: unidades de Re-Volt por segundo en una milla por hora.
pub const MPH2OGU_SPEED: f32 = 1.0 / 0.01118;

pub fn position(revolt: [f32; 3]) -> Vec3 {
    Vec3::new(
        -revolt[0] * REVOLT_TO_METERS,
        -revolt[1] * REVOLT_TO_METERS,
        revolt[2] * REVOLT_TO_METERS,
    )
}

pub fn direction(revolt: [f32; 3]) -> Vec3 {
    Vec3::new(-revolt[0], -revolt[1], revolt[2])
}

/// `rows` es la matriz 3×3 tal como está en el archivo (tres filas).
/// Se transpone y se conjuga con el mismo cambio de ejes que `position`.
pub fn rotation(rows: [[f32; 3]; 3]) -> Quat {
    let mut m = [[0.0; 3]; 3];
    for r in 0..3 {
        for c in 0..3 {
            m[r][c] = rows[c][r];
        }
    }
    let sign = |axis: usize| if axis == 2 { 1.0 } else { -1.0 };
    for r in 0..3 {
        for c in 0..3 {
            m[r][c] *= sign(r) * sign(c);
        }
    }
    let mat = Mat3::from_cols(
        Vec3::new(m[0][0], m[1][0], m[2][0]),
        Vec3::new(m[0][1], m[1][1], m[2][1]),
        Vec3::new(m[0][2], m[1][2], m[2][2]),
    );
    Quat::from_mat3(&mat).normalize()
}

/// Yaw de Revvy para la matriz `RotMatrixY(-turns)` con la que Re-Volt ubica un auto.
/// Con el giro de ejes el ángulo queda igual, así que el yaw es `-turns · τ`: en nhood1
/// (`STARTROT` 0.25) el auto mira hacia −X, hacia la zona 1 de la pista.
pub fn yaw_from_turns(turns: f32) -> f32 {
    -turns * std::f32::consts::TAU
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converted_axes_stay_right_handed() {
        let x = position([1.0, 0.0, 0.0]);
        let y = position([0.0, 1.0, 0.0]);
        let z = position([0.0, 0.0, 1.0]);
        assert!(x.dot(y.cross(z)) > 0.0, "un solo eje negado espeja el mapa");
    }
}
