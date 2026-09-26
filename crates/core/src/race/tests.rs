//! La carrera sobre una vuelta cuadrada de 160 m: sale de la meta en (0, 0, 5) hacia +Z,
//! dobla hacia −X, baja por −Z y vuelve por +X. Una zona por lado y un POS node cada 10 m.

use glam::{Quat, Vec3};
use revvy_formats::layout::{KillVolume, PosNode, SurfaceType, TrackLayout, TrackZone};
use revvy_formats::{Collision, CollisionTri};

use super::*;

const LAP: f32 = 160.0;
const STEP: f32 = 0.5;
const SPEED: f32 = 10.0;

/// El punto de la vuelta a `s` metros de la meta, siguiendo la carrera.
fn along(s: f32) -> Vec3 {
    let s = s.rem_euclid(LAP);
    if s < 35.0 {
        Vec3::new(0.0, 0.0, 5.0 + s)
    } else if s < 75.0 {
        Vec3::new(-(s - 35.0), 0.0, 40.0)
    } else if s < 115.0 {
        Vec3::new(-40.0, 0.0, 40.0 - (s - 75.0))
    } else if s < 155.0 {
        Vec3::new(-40.0 + (s - 115.0), 0.0, 0.0)
    } else {
        Vec3::new(0.0, 0.0, s - 155.0)
    }
}

/// En `s`, mirando hacia donde sigue la carrera (o para atrás).
fn pose(s: f32, backwards: bool) -> CarPose {
    let forward = (along(s + 0.25) - along(s - 0.25)).normalize();
    CarPose {
        pos: along(s),
        forward: if backwards { -forward } else { forward },
    }
}

fn square() -> TrackLayout {
    let zone = |id, (x0, z0): (f32, f32), (x1, z1): (f32, f32)| TrackZone {
        id,
        center: Vec3::new((x0 + x1) / 2.0, 0.0, (z0 + z1) / 2.0),
        rotation: Quat::IDENTITY,
        half_extents: Vec3::new((x1 - x0) / 2.0, 5.0, (z1 - z0) / 2.0),
    };
    let count = 16;
    TrackLayout {
        start_node: 0,
        total_distance: LAP,
        zones: vec![
            zone(0, (-5.0, 5.0), (5.0, 45.0)),
            zone(1, (-45.0, 35.0), (5.0, 45.0)),
            zone(2, (-45.0, -5.0), (-35.0, 45.0)),
            zone(3, (-45.0, -5.0), (5.0, 5.0)),
        ],
        pos_nodes: (0..count)
            .map(|k| {
                let s = k as f32 * 10.0;
                PosNode {
                    id: k,
                    position: along(s),
                    distance: if k == 0 { 0.0 } else { LAP - s },
                    prev: vec![((k + count - 1) % count) as i32],
                    next: vec![((k + 1) % count) as i32],
                }
            })
            .collect(),
        ..TrackLayout::default()
    }
}

fn rules(laps: u32) -> GameplayRules {
    GameplayRules {
        laps,
        ..GameplayRules::default()
    }
}

/// Una carrera con un auto parado 3 m antes de la meta, como en la grilla de Re-Volt.
fn race(layout: &TrackLayout, laps: u32) -> Race {
    Race::new(
        RaceTrack::new(layout, None),
        &rules(laps),
        &[pose(-3.0, false)],
    )
}

/// Maneja el auto de `from` a `to` metros (hacia atrás si `to` es menor) a 10 m/s.
fn drive(race: &mut Race, from: f32, to: f32, backwards: bool) -> Vec<RaceEvent> {
    let steps = ((to - from).abs() / STEP).round() as usize;
    let dir = (to - from).signum();
    let mut events = Vec::new();
    for i in 1..=steps {
        let s = from + dir * STEP * i as f32;
        events.extend(race.update(STEP / SPEED, &[pose(s, backwards)]));
    }
    events
}

#[test]
fn a_lap_counts_each_time_the_car_crosses_the_line() {
    let layout = square();
    let mut race = race(&layout, 2);
    let events = drive(&mut race, -3.0, 2.0 * LAP + 10.0, false);
    let laps: Vec<(u32, f64)> = events
        .iter()
        .filter_map(|event| match *event {
            RaceEvent::Lap { lap, time, .. } => Some((lap, time)),
            _ => None,
        })
        .collect();
    assert_eq!(laps.len(), 2, "{events:?}");
    for (i, &(lap, time)) in laps.iter().enumerate() {
        assert_eq!(lap, i as u32 + 1);
        assert!((time - 16.0).abs() < 1e-3, "vuelta de {time} s");
    }
    // Cruzó la línea a los 0.3 s y dio dos vueltas de 16 s.
    let finished = events
        .iter()
        .find_map(|event| match *event {
            RaceEvent::Finished { time, .. } => Some(time),
            _ => None,
        })
        .expect("terminó");
    assert!((finished - 32.3).abs() < 1e-3, "{finished}");
    assert_eq!(race.results(), vec![(0, finished)]);
    assert_eq!(race.racer(0).best_lap(), race.racer(0).last_lap());
    assert!(!events
        .iter()
        .any(|event| matches!(event, RaceEvent::Respawn { .. })));
}

#[test]
fn going_backwards_is_wrong_way_after_a_second() {
    let layout = square();
    let mut race = race(&layout, 3);
    drive(&mut race, -3.0, 20.0, false);
    assert!(!race.racer(0).wrong_way());
    // Media vuelta y para atrás: el aviso aparece recién pasado un segundo.
    drive(&mut race, 20.0, 15.5, true);
    assert!(!race.racer(0).wrong_way(), "a los 0.45 s todavía no");
    drive(&mut race, 15.5, 9.0, true);
    assert!(race.racer(0).wrong_way());
    drive(&mut race, 9.0, 20.0, false);
    assert!(!race.racer(0).wrong_way());
}

#[test]
fn backing_over_the_line_does_not_count_a_lap() {
    let layout = square();
    let mut race = race(&layout, 3);
    let mut events = drive(&mut race, -3.0, 10.0, false);
    events.extend(drive(&mut race, 10.0, -10.0, false));
    events.extend(drive(&mut race, -10.0, LAP + 10.0, false));
    let laps: Vec<u32> = events
        .iter()
        .filter_map(|event| match *event {
            RaceEvent::Lap { lap, .. } => Some(lap),
            _ => None,
        })
        .collect();
    assert_eq!(laps, [1], "solo la vuelta completa");
}

#[test]
fn a_shortcut_does_not_count_a_lap() {
    let layout = square();
    let mut race = race(&layout, 3);
    let mut events = drive(&mut race, -3.0, 20.0, false);
    // De la zona 0 a la 2 sin pasar por la 1.
    events.extend(drive(&mut race, 100.0, LAP + 10.0, false));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, RaceEvent::Lap { .. })),
        "{events:?}"
    );
    assert_eq!(race.racer(0).laps(), 0);
}

#[test]
fn leaving_the_zones_respawns_at_the_last_good_node() {
    let layout = square();
    let mut race = race(&layout, 3);
    drive(&mut race, -3.0, 21.0, false);
    // Afuera de todas las zonas: a los 1.5 s hay que reposicionarlo.
    let outside = CarPose {
        pos: Vec3::new(20.0, 0.0, 25.0),
        forward: Vec3::X,
    };
    let mut when = None;
    for frame in 1..=40 {
        let events = race.update(0.05, &[outside]);
        if events.contains(&RaceEvent::Respawn {
            car: 0,
            reason: RespawnReason::OffTrack,
        }) {
            when = Some(frame as f32 * 0.05);
            break;
        }
    }
    let when = when.expect("se reposicionó");
    assert!((when - 1.5).abs() < 0.051, "a los {when} s");
    let (pos, yaw) = race.respawn_pose(0).unwrap();
    assert_eq!(
        pos,
        along(20.0),
        "el POS node más cercano cuando estaba en la pista"
    );
    assert!(yaw.abs() < 1e-5, "mirando hacia +Z: {yaw}");
}

#[test]
fn kill_volumes_and_the_world_edge_respawn_at_once() {
    let mut layout = square();
    layout.kill_volumes.push(KillVolume {
        center: Vec3::new(0.0, 0.0, 25.0),
        rotation: Quat::IDENTITY,
        half_extents: Vec3::new(5.0, 1.0, 2.0),
    });
    let floor = Collision {
        triangles: vec![CollisionTri {
            positions: [
                Vec3::new(-50.0, 0.0, -10.0),
                Vec3::new(10.0, 0.0, -10.0),
                Vec3::new(-50.0, 0.0, 50.0),
            ],
            surface: SurfaceType::Road,
            camera_only: false,
            object_only: false,
        }],
        surfaces: Vec::new(),
    };
    let track = RaceTrack::new(&layout, Some(&floor));
    let mut race = Race::new(track, &rules(3), &[pose(-3.0, false)]);
    let events = drive(&mut race, -3.0, 20.0, false);
    assert!(events.contains(&RaceEvent::Respawn {
        car: 0,
        reason: RespawnReason::KillVolume,
    }));
    // Después de reaparecer tiene un segundo sin volver a hacerlo solo.
    let falling = CarPose {
        pos: Vec3::new(0.0, -30.0, 25.0),
        forward: Vec3::Z,
    };
    let events: Vec<RaceEvent> = (0..10).flat_map(|_| race.update(0.2, &[falling])).collect();
    assert_eq!(
        events,
        [RaceEvent::Respawn {
            car: 0,
            reason: RespawnReason::OutOfWorld,
        }]
    );
}

#[test]
fn standings_go_by_finish_time_then_laps_then_distance() {
    let layout = square();
    let track = RaceTrack::new(&layout, None);
    let start = [pose(-3.0, false), pose(-4.0, false), pose(-5.0, false)];
    let mut race = Race::new(track, &rules(1), &start);
    // El auto 2 da la vuelta; el 1 va por los 80 m y el 0 por los 50.
    let steps = ((LAP + 10.0) / STEP) as usize;
    for i in 0..=steps {
        let s = |from: f32, to: f32| (from + i as f32 * STEP).min(to);
        let cars = [
            pose(s(-3.0, 50.0), false),
            pose(s(-4.0, 80.0), false),
            pose(s(-5.0, LAP + 5.0), false),
        ];
        race.update(STEP / SPEED, &cars);
    }
    assert_eq!(race.standings(), vec![2, 1, 0]);
    assert_eq!(race.position(0), 3);
    assert_eq!(race.results().len(), 1);
}

#[test]
fn without_zones_there_are_no_laps() {
    let layout = TrackLayout::default();
    let mut race = race(&layout, 3);
    assert!(!race.has_laps());
    let events = race.update(1.0, &[pose(50.0, false)]);
    assert!(events.is_empty());
    assert_eq!(race.respawn_pose(0), None);
}
