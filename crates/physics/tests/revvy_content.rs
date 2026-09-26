//! Autos de Re-Volt y propios, en pistas de Re-Volt y `.glb`, en el mismo motor.

use std::path::PathBuf;

use glam::Vec3;
use revvy_formats::{load_car, load_track, TrackLoad};
use revvy_physics::{Controls, PhysicsWorld};

const DT: f32 = 1.0 / 60.0;

fn content(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content").join(rel)
}

/// La pista y los autos en los primeros puestos de la grilla.
fn world(level: &str, cars: &[&str]) -> PhysicsWorld {
    let track = load_track(&content(level), TrackLoad::collision_only()).expect("pista");
    let mut world = PhysicsWorld::new(track.asset.collision.as_ref().expect("colisión"));
    for (slot, car) in track.asset.layout.start_grid.iter().zip(cars) {
        let car = load_car(&content(car)).expect("auto");
        world.add_vehicle(&car.vehicle, slot.pos, slot.yaw);
    }
    world
}

fn run(world: &mut PhysicsWorld, seconds: f32, controls: &[Controls]) {
    for _ in 0..(seconds / DT).round() as usize {
        world.frame(DT, controls);
    }
}

fn position(world: &PhysicsWorld, i: usize) -> Vec3 {
    world.vehicle(i).pose(1.0).0
}

#[test]
fn a_revolt_car_and_an_own_car_share_nhood1() {
    let mut world = world("levels/nhood1", &["cars/phim_calcure", "cars/revvy_buggy"]);
    run(&mut world, 2.0, &[]);
    for i in 0..2 {
        assert_eq!(world.vehicle(i).wheels_in_contact(), 4, "el auto {i} no apoya las cuatro ruedas");
    }

    // Un buggy propio 1.5 m detrás del Calcure lo lleva por delante.
    let track = load_track(&content("levels/nhood1"), TrackLoad::collision_only()).unwrap();
    let mut world = PhysicsWorld::new(track.asset.collision.as_ref().unwrap());
    let slot = &track.asset.layout.start_grid[0];
    let forward = glam::Quat::from_rotation_y(slot.yaw) * Vec3::Z;
    world.add_vehicle(&load_car(&content("cars/phim_calcure")).unwrap().vehicle, slot.pos, slot.yaw);
    world.add_vehicle(&load_car(&content("cars/revvy_buggy")).unwrap().vehicle, slot.pos - forward * 1.5, slot.yaw);
    run(&mut world, 1.0, &[]);
    let calcure = position(&world, 0);
    let push = Controls { steer: 0.0, throttle: 1.0, reset: false };
    run(&mut world, 1.5, &[Controls::default(), push]);
    let moved = (position(&world, 0) - calcure).dot(forward);
    assert!(moved > 0.3, "el buggy no empujó al Calcure: se movió {moved} m");
}

#[test]
fn both_cars_drive_on_the_glb_arena() {
    let mut world = world("levels/revvy_arena", &["cars/phim_calcure", "cars/revvy_buggy"]);
    run(&mut world, 1.5, &[]);
    let start = [position(&world, 0), position(&world, 1)];
    for i in 0..2 {
        assert_eq!(world.vehicle(i).wheels_in_contact(), 4, "el auto {i} no apoya las cuatro ruedas en la arena");
    }
    let throttle = Controls { steer: 0.0, throttle: 1.0, reset: false };
    run(&mut world, 2.0, &[throttle, throttle]);
    for i in 0..2 {
        let moved = position(&world, i) - start[i];
        assert!(moved.z > 5.0, "el auto {i} no avanzó hacia +Z: {moved:?}");
    }
}

#[test]
fn ice_holds_less_than_road() {
    // El mismo auto dobla en el asfalto y en el hielo de la arena. En el hielo la
    // velocidad cambia mucho menos de dirección: el auto se va de largo.
    let track = load_track(&content("levels/revvy_arena"), TrackLoad::collision_only()).expect("arena");
    let car = load_car(&content("cars/phim_calcure")).expect("calcure");
    let mut world = PhysicsWorld::new(track.asset.collision.as_ref().unwrap());
    world.add_vehicle(&car.vehicle, Vec3::new(-50.0, 0.3, -50.0), 0.0);
    world.add_vehicle(&car.vehicle, Vec3::new(30.0, 0.3, 12.0), 0.0);
    run(&mut world, 1.0, &[]);
    let throttle = Controls { steer: 0.0, throttle: 1.0, reset: false };
    run(&mut world, 1.0, &[throttle, throttle]);
    let course = |w: &PhysicsWorld, i: usize| {
        let v = w.vehicle(i).velocity(w.bodies());
        v.x.atan2(v.z)
    };
    let before = [course(&world, 0), course(&world, 1)];
    let turn = Controls { steer: -1.0, throttle: 1.0, reset: false };
    run(&mut world, 0.6, &[turn, turn]);
    let road = course(&world, 0) - before[0];
    let ice = course(&world, 1) - before[1];
    assert!(road > 0.0, "no dobla en asfalto: {road}");
    assert!(ice < road * 0.7, "el hielo agarra como el asfalto: road {:.1}° ice {:.1}°", road.to_degrees(), ice.to_degrees());
}

/// El piso para reaparecer: la arena está en y = 0 y nhood1, bajo la largada.
#[test]
fn ground_below_finds_the_track() {
    let arena = world("levels/revvy_arena", &[]);
    let ground = arena
        .ground_below(Vec3::new(3.0, 2.0, -12.0), 5.0)
        .expect("piso de la arena");
    assert!(
        ground.distance(Vec3::new(3.0, 0.0, -12.0)) < 1e-4,
        "{ground}"
    );
    assert_eq!(
        arena.ground_below(Vec3::new(3.0, 2.0, -12.0), 1.0),
        None,
        "más lejos que depth"
    );

    let nhood1 = world("levels/nhood1", &[]);
    let dir = content("levels/nhood1");
    let track = load_track(&dir, TrackLoad::collision_only()).unwrap();
    let start = track.asset.layout.start_grid[0].pos;
    let ground = nhood1
        .ground_below(start + Vec3::Y * 0.5, 5.0)
        .expect("piso de nhood1");
    assert!(
        (ground.y - start.y).abs() < 0.2,
        "la largada queda a {} m del piso",
        start.y - ground.y
    );
}
