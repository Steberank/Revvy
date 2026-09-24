//! El Calcure en nhood1 con el port de referencia de Re-Volt.

use std::path::PathBuf;

use glam::Vec3;
use revvy_formats::{load_car, load_track, TrackLoad};
use revvy_physics::revolt::units::OGU2MPH_SPEED;
use revvy_physics::revolt::{Car, CollWorld, Controls, Simulation};

fn content(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content").join(rel)
}

fn simulation() -> Simulation {
    let track = load_track(&content("levels/nhood1"), TrackLoad::collision_only()).expect("nhood1");
    let legacy = track.asset.legacy.expect("datos nativos del nivel");
    let car_def = load_car(&content("cars/phim_calcure")).expect("calcure");
    let start = Car::start_grid(legacy.start_pos, legacy.start_rot, legacy.start_grid_type);
    let revolt = car_def.revolt.as_ref().expect("auto de Re-Volt");
    let car = Car::new(&revolt.info, &revolt.hull_spheres);
    Simulation::new(CollWorld::new(&legacy), car, start)
}

fn run(sim: &mut Simulation, seconds: f32, controls: Controls) {
    let frames = (seconds * 60.0) as usize;
    for _ in 0..frames {
        sim.frame(1.0 / 60.0, controls);
    }
}

fn wheels_on_floor(sim: &Simulation) -> usize {
    sim.car.wheels.iter().filter(|w| w.in_contact()).count()
}

#[test]
fn calcure_settles_on_its_wheels_at_the_start_line() {
    let mut sim = simulation();
    let start_y = sim.car.body.centre.pos.y;
    run(&mut sim, 3.0, Controls::default());
    let body = &sim.car.body;
    assert!(body.centre.pos.is_finite());
    assert!(
        (body.centre.pos.y - start_y).abs() < 30.0,
        "el auto se fue: y {} → {}",
        start_y,
        body.centre.pos.y
    );
    assert!(body.centre.vel.length() < 20.0, "no se asentó: vel {:?}", body.centre.vel);
    assert_eq!(wheels_on_floor(&sim), 4, "no apoya las cuatro ruedas");
    assert!(body.centre.wmatrix.u.y > 0.95, "no quedó derecho: up {:?}", body.centre.wmatrix.u);
}

#[test]
fn full_throttle_drives_forward_and_spins_the_wheels() {
    let mut sim = simulation();
    run(&mut sim, 1.0, Controls::default());
    let start = sim.car.body.centre.pos;
    let forward = sim.car.body.centre.wmatrix.l;
    run(
        &mut sim,
        2.0,
        Controls {
            dy: -127.0,
            ..Controls::default()
        },
    );
    let body = &sim.car.body;
    let moved = body.centre.pos - start;
    let mph = body.centre.vel.length() * OGU2MPH_SPEED;
    assert!(moved.dot(forward) > 300.0, "avanzó {} unidades", moved.dot(forward));
    assert!(mph > 10.0, "velocidad {mph} mph");
    assert!(sim.car.revs > 0.0, "revs {}", sim.car.revs);
    let spin = sim.car.wheels[2].ang_vel;
    assert!(spin > 0.0, "la rueda trasera no gira hacia adelante: {spin}");
    assert!(body.centre.wmatrix.u.y > 0.8, "se dio vuelta: up {:?}", body.centre.wmatrix.u);
}

#[test]
fn steering_left_turns_the_car_left() {
    let mut sim = simulation();
    run(&mut sim, 1.0, Controls::default());
    run(
        &mut sim,
        1.0,
        Controls {
            dy: -127.0,
            ..Controls::default()
        },
    );
    let before = sim.car.body.centre.wmatrix.l;
    run(
        &mut sim,
        0.6,
        Controls {
            dx: -127.0,
            dy: -127.0,
            ..Controls::default()
        },
    );
    let after = sim.car.body.centre.wmatrix.l;
    // En Re-Volt Y apunta abajo: mirando desde arriba, `before × after` hacia −Y es giro a la izquierda.
    let turn = before.cross(after).y;
    assert!(turn < -0.05, "no dobló a la izquierda: {turn} (antes {before:?} después {after:?})");
    assert!(sim.car.wheels[0].turn_angle > 0.1, "la rueda delantera no gira: {}", sim.car.wheels[0].turn_angle);
    let _ = Vec3::ZERO;
}
