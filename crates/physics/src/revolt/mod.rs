//! Port de la física de Re-Volt, versión retail de PC (`rvsource/Xbox/Src`, ramas
//! `_PC`, modo Simulación): `newcoll.cpp`, `body.cpp`, `particle.cpp`, `car.cpp`,
//! `wheel.cpp`, `field.cpp` (gravedad), `control.cpp`, `move.cpp` y `camera.cpp`.
//!
//! Todo corre en el espacio de Re-Volt: una unidad mide 5 mm, Y apunta hacia abajo y
//! las matrices son tres filas (right, up, look) en coordenadas de mundo. La conversión
//! a Revvy (Y arriba, metros) se hace afuera, en `vehicle_controller`.

pub mod body;
pub mod camera;
pub mod car;
pub mod coll;
pub mod conjgrad;
pub mod level;
pub mod material;
pub mod math;
pub mod units;

pub use camera::FollowCamera;
pub use car::{Car, Controls, FrameReport, SfxState, Simulation};
pub use level::CollWorld;
pub use math::Mat;
