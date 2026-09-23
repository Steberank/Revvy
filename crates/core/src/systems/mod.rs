//! Schedule de conducción, el mismo en cliente y server.
//! La integración sigue siendo local: quien simula escribe `VehiclePose`.

use bevy_ecs::prelude::*;

use crate::components::{CarId, Transform, VehiclePose, Velocity};

pub fn drive_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.add_systems(apply_vehicle_pose);
    schedule
}

fn apply_vehicle_pose(
    pose: Res<VehiclePose>,
    mut cars: Query<(&CarId, &mut Transform, &mut Velocity)>,
) {
    for (_id, mut transform, mut velocity) in &mut cars {
        transform.translation = pose.translation;
        transform.rotation = pose.rotation;
        velocity.linear = pose.linear;
        velocity.angular = pose.angular;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Quat, Vec3};

    #[test]
    fn schedule_copies_pose_onto_the_car() {
        let mut world = World::new();
        world.insert_resource(VehiclePose {
            translation: Vec3::new(1.0, 2.0, 3.0),
            rotation: Quat::IDENTITY,
            linear: Vec3::X,
            angular: Vec3::ZERO,
        });
        world.spawn((
            CarId("calcure".into()),
            Transform {
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
            },
            Velocity::default(),
            crate::components::PowerupSlot::Empty,
        ));
        let mut schedule = drive_schedule();
        schedule.run(&mut world);
        let transform = world.query::<&Transform>().single(&world).expect("auto");
        assert_eq!(transform.translation, Vec3::new(1.0, 2.0, 3.0));
    }
}
