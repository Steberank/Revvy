//! Componentes compartidos entre cliente y server.

use bevy_ecs::prelude::*;
use glam::{Quat, Vec3};

#[derive(Component, Clone, Debug)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
}

#[derive(Component, Clone, Debug, Default)]
pub struct Velocity {
    pub linear: Vec3,
    pub angular: Vec3,
}

/// Un solo slot. En esta fase queda vacío.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub enum PowerupSlot {
    #[default]
    Empty,
    Occupied {
        kind: String,
    },
}

#[derive(Component, Clone, Debug)]
pub struct CarId(pub String);

/// Pose que escribe la física local. El schedule la copia a las entidades.
#[derive(Resource, Clone, Debug, Default)]
pub struct VehiclePose {
    pub translation: Vec3,
    pub rotation: Quat,
    pub linear: Vec3,
    pub angular: Vec3,
}
