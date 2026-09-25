//! Motor de física de Revvy: Rapier con el vehículo de Revvy encima, la cámara de
//! persecución y las superficies. `revolt` es el port de Re-Volt que sirve de referencia
//! de manejo en los tests; el motor no lo usa.

pub mod camera;
pub mod collision_events;
pub mod force_field;
pub mod jump;
pub mod objects;
pub mod revolt;
pub mod surfaces;
pub mod vehicle_controller;
pub mod world;

pub use camera::ChaseCamera;
pub use objects::Prop;
pub use vehicle_controller::{Controls, Vehicle, VehicleSound};
pub use world::{PhysicsWorld, SphereHit, Viewer, GRAVITY, TICK};
