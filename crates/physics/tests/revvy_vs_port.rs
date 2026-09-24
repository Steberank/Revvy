//! El motor de Revvy (Rapier + vehículo de Revvy) contra el port de referencia de
//! Re-Volt, con el mismo auto, la misma pista y los mismos mandos.

use std::path::PathBuf;

use glam::{Quat, Vec3};
use revvy_formats::{load_car, load_track, CarDef, TrackLoad};
use revvy_physics::revolt::convert::{to_revvy_dir, to_revvy_point};
use revvy_physics::revolt::{self, Car, CollWorld, Simulation};
use revvy_physics::{Controls, PhysicsWorld};

const DT: f32 = 1.0 / 60.0;

fn content(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content").join(rel)
}

struct Pair {
    port: Simulation,
    revvy: PhysicsWorld,
}

fn pair() -> Pair {
    let track = load_track(&content("levels/nhood1"), TrackLoad::collision_only()).expect("nhood1");
    let legacy = track.asset.legacy.clone().expect("datos de Re-Volt");
    let car: CarDef = load_car(&content("cars/phim_calcure")).expect("calcure");
    let raw = car.revolt.as_ref().expect("auto de Re-Volt");
    let start = Car::start_grid(legacy.start_pos, legacy.start_rot, legacy.start_grid_type);
    let port = Simulation::new(CollWorld::new(&legacy), Car::new(&raw.info, &raw.hull_spheres), start);
    let mut revvy = PhysicsWorld::new(track.asset.collision.as_ref().expect("colisión"));
    let slot = &track.asset.layout.start_grid[0];
    revvy.add_vehicle(&car.vehicle, slot.pos, slot.yaw);
    Pair { port, revvy }
}

impl Pair {
    fn run(&mut self, seconds: f32, steer: f32, throttle: f32) {
        let port_controls = revolt::Controls {
            dx: steer * 127.0,
            dy: -throttle * 127.0,
            reset: false,
        };
        let controls = [Controls { steer, throttle, reset: false }];
        for _ in 0..(seconds / DT).round() as usize {
            self.port.frame(DT, port_controls);
            self.revvy.frame(DT, &controls);
        }
    }

    fn port_pose(&self) -> (Vec3, Vec3, Vec3) {
        let body = &self.port.car.body;
        (
            to_revvy_point(body.centre.pos),
            to_revvy_dir(body.centre.vel) * revvy_formats::REVOLT_TO_METERS,
            to_revvy_dir(body.centre.wmatrix.l),
        )
    }

    fn revvy_pose(&self) -> (Vec3, Vec3, Vec3) {
        let vehicle = self.revvy.vehicle(0);
        let (pos, rot): (Vec3, Quat) = vehicle.pose(1.0);
        (pos, vehicle.velocity(self.revvy.bodies()), rot * Vec3::Z)
    }
}

fn yaw(forward: Vec3) -> f32 {
    forward.x.atan2(forward.z)
}

fn mph(v: Vec3) -> f32 {
    v.length() * 2.23694
}

#[test]
fn settles_like_the_port() {
    let mut pair = pair();
    pair.run(2.0, 0.0, 0.0);
    let (port, _, _) = pair.port_pose();
    let (revvy, _, _) = pair.revvy_pose();
    assert!((port.y - revvy.y).abs() < 0.005, "altura: port {} revvy {}", port.y, revvy.y);
    let scale = revvy_formats::REVOLT_TO_METERS;
    for (w, travel) in pair.port.car.wheels.iter().zip(pair.revvy.vehicle(0).suspension()) {
        assert!((w.pos * scale - travel).abs() < 0.002, "suspensión: port {} revvy {}", w.pos * scale, travel);
    }
    assert_eq!(pair.revvy.vehicle(0).wheels_in_contact(), 4);
}

#[test]
fn accelerates_like_the_port() {
    let mut pair = pair();
    pair.run(1.0, 0.0, 0.0);
    for _ in 0..4 {
        pair.run(0.5, 0.0, 1.0);
        let (_, port, _) = pair.port_pose();
        let (_, revvy, _) = pair.revvy_pose();
        assert!((mph(port) - mph(revvy)).abs() < 0.5, "mph: port {} revvy {}", mph(port), mph(revvy));
    }
    let spin = pair.revvy.vehicle(0).wheel_spin();
    assert!(spin[2] > 50.0 && spin[3] > 50.0, "las ruedas de atrás no giran: {spin:?}");
}

#[test]
fn steers_like_the_port() {
    let mut pair = pair();
    pair.run(1.0, 0.0, 0.0);
    pair.run(1.0, 0.0, 1.0);
    let (_, _, port0) = pair.port_pose();
    let (_, _, revvy0) = pair.revvy_pose();
    for (seconds, tolerance) in [(0.25, 0.12), (0.5, 0.05)] {
        pair.run(0.25, -1.0, 1.0);
        let (_, _, port) = pair.port_pose();
        let (_, _, revvy) = pair.revvy_pose();
        let (port_yaw, revvy_yaw) = (yaw(port) - yaw(port0), yaw(revvy) - yaw(revvy0));
        assert!(revvy_yaw > 0.0, "{seconds} s: no dobla a la izquierda");
        assert!(
            (port_yaw - revvy_yaw).abs() < port_yaw.abs() * tolerance,
            "{seconds} s: port {:.1}° revvy {:.1}°",
            port_yaw.to_degrees(),
            revvy_yaw.to_degrees()
        );
    }
}
