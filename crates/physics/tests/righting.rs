//! Enderezar (R) funciona cada vez que el auto queda dado vuelta.

use std::path::PathBuf;

use glam::{Quat, Vec3};
use revvy_formats::{load_car, load_track, TrackLoad};
use revvy_physics::{Controls, PhysicsWorld};

const DT: f32 = 1.0 / 60.0;

fn content(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content").join(rel)
}

fn run(world: &mut PhysicsWorld, seconds: f32, controls: Controls) {
    for _ in 0..(seconds / DT).round() as usize {
        world.frame(DT, &[controls]);
    }
}

fn up_y(world: &PhysicsWorld) -> f32 {
    (world.vehicle(0).pose(1.0).1 * Vec3::Y).y
}

#[test]
fn righting_works_every_time() {
    for car in ["cars/phim_calcure", "cars/revvy_buggy"] {
        let track = load_track(&content("levels/revvy_arena"), TrackLoad::collision_only()).unwrap();
        let mut world = PhysicsWorld::new(track.asset.collision.as_ref().unwrap());
        world.add_vehicle(&load_car(&content(car)).unwrap().vehicle, Vec3::new(0.0, 0.3, 0.0), 0.0);
        run(&mut world, 1.0, Controls::default());
        for round in 1..=3 {
            let pos = world.vehicle(0).pose(1.0).0 + Vec3::Y * 0.4;
            world.place_vehicle(0, pos, Quat::from_rotation_z(std::f32::consts::PI));
            run(&mut world, 1.5, Controls::default());
            assert!(up_y(&world) < -0.5, "{car}, vuelta {round}: no quedó dado vuelta ({})", up_y(&world));
            run(&mut world, 0.1, Controls { reset: true, ..Controls::default() });
            run(&mut world, 2.5, Controls::default());
            assert!(up_y(&world) > 0.95, "{car}, vuelta {round}: R no lo enderezó ({})", up_y(&world));
        }
    }
}
