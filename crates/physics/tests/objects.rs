//! Objetos de la pista en el motor: se apoyan, los autos los empujan, la pelota rebota y
//! los que arrancan dormidos esperan a que algo los toque. Las puertas corredizas siguen su
//! camino y empujan lo que encuentran; el chango se endereza solo.

use std::path::PathBuf;

use glam::{Quat, Vec3};
use revvy_formats::{
    load_car, load_track, LoadedTrack, MotionCue, ObjectKind, ObjectSpawn, SpawnWhen, TrackLoad,
};
use revvy_physics::{Controls, PhysicsWorld};

const DT: f32 = 1.0 / 60.0;

fn content(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../content")
        .join(rel)
}

fn track(level: &str) -> LoadedTrack {
    load_track(
        &content(&format!("levels/{level}")),
        TrackLoad::collision_only(),
    )
    .unwrap()
}

fn world(track: &LoadedTrack) -> PhysicsWorld {
    PhysicsWorld::new(track.asset.collision.as_ref().unwrap())
}

fn run(world: &mut PhysicsWorld, seconds: f32, controls: &[Controls]) {
    for _ in 0..(seconds / DT).round() as usize {
        world.frame(DT, controls);
    }
}

fn kind<'a>(track: &'a LoadedTrack, name: &str) -> (usize, &'a ObjectKind) {
    let kinds = &track.asset.objects.kinds;
    let index = kinds
        .iter()
        .position(|kind| kind.name == name)
        .unwrap_or_else(|| panic!("falta {name}"));
    (index, &kinds[index])
}

/// Una aparición en el mundo: con camino, o suelta con su velocidad.
fn add(world: &mut PhysicsWorld, track: &LoadedTrack, spawn: &ObjectSpawn) -> usize {
    let kind = &track.asset.objects.kinds[spawn.kind];
    match spawn.motion {
        Some(motion) => world.add_moving_object(spawn.kind, kind, spawn.pos, spawn.rot, motion),
        None => world.add_object(spawn.kind, kind, spawn.pos, spawn.rot, spawn.velocity),
    }
    .unwrap()
}

/// Los objetos que aparecen al empezar, con su posición inicial.
fn spawn_start(track: &LoadedTrack, world: &mut PhysicsWorld) -> Vec<(usize, Vec3)> {
    track
        .asset
        .objects
        .spawns
        .iter()
        .filter(|spawn| matches!(spawn.when, SpawnWhen::Start))
        .map(|spawn| (add(world, track, spawn), spawn.pos))
        .collect()
}

fn named(track: &LoadedTrack, world: &PhysicsWorld, index: usize, name: &str) -> bool {
    track.asset.objects.kinds[world.objects()[index].kind()].name == name
}

#[test]
fn nhood1_cones_rest_on_the_street() {
    let nhood1 = track("nhood1");
    let mut world = world(&nhood1);
    let cones = spawn_start(&nhood1, &mut world);
    assert_eq!(cones.len(), 4);
    let upright: Vec<Vec3> = cones
        .iter()
        .map(|&(i, _)| world.objects()[i].pose(1.0).1 * Vec3::Y)
        .collect();
    run(&mut world, 3.0, &[]);
    for (&(i, start), up) in cones.iter().zip(upright) {
        let prop = &world.objects()[i];
        let (pos, rot) = prop.pose(1.0);
        assert!(
            pos.distance(start) < 0.1,
            "el cono {i} se fue de {start} a {pos}"
        );
        // Uno está sobre una pendiente de 8.6°: se compara con cómo lo pusieron.
        assert!((rot * Vec3::Y).dot(up) > 0.99, "el cono {i} se cayó");
        assert!(
            prop.velocity(world.bodies()).length() < 0.05,
            "el cono {i} sigue moviéndose"
        );
    }
}

#[test]
fn the_calcure_knocks_a_cone_away() {
    let arena = track("revvy_arena");
    let nhood1 = track("nhood1");
    let (cone_index, cone) = kind(&nhood1, "cone");
    let mut world = world(&arena);
    let start = arena.asset.layout.start_grid[0].clone();
    world.add_vehicle(
        &load_car(&content("cars/phim_calcure")).unwrap().vehicle,
        start.pos,
        start.yaw,
    );
    let ahead = Quat::from_rotation_y(start.yaw) * Vec3::Z;
    let cone_start = start.pos + ahead * 5.0 + Vec3::Y * (0.1 - start.pos.y);
    let index = world
        .add_object(cone_index, cone, cone_start, Quat::IDENTITY, Vec3::ZERO)
        .unwrap();
    run(&mut world, 1.0, &[Controls::default()]);
    world.take_knocks();
    let throttle = Controls {
        throttle: 1.0,
        ..Controls::default()
    };
    run(&mut world, 3.0, &[throttle]);
    let (pos, _) = world.objects()[index].pose(1.0);
    assert!(
        pos.distance(cone_start) > 1.0,
        "el auto no movió el cono: {pos}"
    );
    let knock = world
        .take_knocks()
        .iter()
        .filter(|(i, _)| *i == index)
        .map(|&(_, knock)| knock)
        .fold(0.0, f32::max);
    let sound = cone.impact_sound.as_ref().unwrap();
    assert!(
        sound.volume(knock).is_some(),
        "el golpe de {knock} m/s no suena"
    );
}

#[test]
fn the_thrown_basketball_bounces_and_settles() {
    let nhood1 = track("nhood1");
    let (ball, _) = kind(&nhood1, "basketball");
    let objects = &nhood1.asset.objects;
    let mut world = world(&nhood1);
    let throws: Vec<usize> = objects
        .spawns
        .iter()
        .filter(|spawn| spawn.kind == ball)
        .map(|spawn| {
            world
                .add_object(
                    spawn.kind,
                    &objects.kinds[spawn.kind],
                    spawn.pos,
                    spawn.rot,
                    spawn.velocity,
                )
                .unwrap()
        })
        .collect();
    assert_eq!(throws.len(), 2);
    let mut bounced = vec![false; throws.len()];
    let mut loudest = vec![0.0f32; throws.len()];
    for _ in 0..(10.0 / DT) as usize {
        let before: Vec<f32> = throws
            .iter()
            .map(|&i| world.objects()[i].velocity(world.bodies()).y)
            .collect();
        run(&mut world, DT, &[]);
        for (slot, &i) in throws.iter().enumerate() {
            let vy = world.objects()[i].velocity(world.bodies()).y;
            // Rebote: venía cayendo rápido y ahora sube.
            if before[slot] < -1.0 && vy > 0.5 {
                bounced[slot] = true;
            }
            let (pos, _) = world.objects()[i].pose(1.0);
            assert!(pos.y > -10.0, "la pelota {slot} atravesó el piso: {pos}");
        }
        for (i, knock) in world.take_knocks() {
            if let Some(slot) = throws.iter().position(|&t| t == i) {
                loudest[slot] = loudest[slot].max(knock);
            }
        }
    }
    assert!(
        bounced.iter().all(|&b| b),
        "alguna pelota no rebotó: {bounced:?}"
    );
    assert!(
        loudest.iter().all(|&knock| knock > 1.5),
        "los botes no suenan: {loudest:?}"
    );
    for &i in &throws {
        assert!(
            world.objects()[i].velocity(world.bodies()).length() < 2.0,
            "la pelota {i} no se frena"
        );
    }
}

#[test]
fn market1_bottles_wait_on_the_shelf_until_touched() {
    let market1 = track("market1");
    let nhood1 = track("nhood1");
    let mut world = world(&market1);
    let shelf = spawn_start(&market1, &mut world);
    assert_eq!(shelf.len(), 17);
    run(&mut world, 2.0, &[]);
    for &(i, start) in &shelf {
        let prop = &world.objects()[i];
        assert!(
            prop.sleeping(world.bodies()),
            "el objeto {i} se despertó solo"
        );
        let moved = prop.pose(1.0).0.distance(start);
        assert!(moved < 0.001, "el objeto {i} se movió solo {moved} m");
    }

    // Una pelota que cae sobre la primera botella la despierta.
    let (ball_index, ball) = kind(&nhood1, "basketball");
    let (bottle, bottle_start) = shelf
        .iter()
        .copied()
        .find(|&(i, _)| market1.asset.objects.kinds[world.objects()[i].kind()].name == "bottle")
        .unwrap();
    world.add_object(
        ball_index,
        ball,
        bottle_start + Vec3::Y * 1.2,
        Quat::IDENTITY,
        Vec3::ZERO,
    );
    run(&mut world, 2.0, &[]);
    assert!(
        world.objects()[bottle].pose(1.0).0.distance(bottle_start) > 0.02,
        "la botella no se movió"
    );
}

#[test]
fn the_arena_objects_come_from_the_track_folder() {
    let arena = track("revvy_arena");
    let mut world = world(&arena);
    let cones: Vec<_> = spawn_start(&arena, &mut world)
        .into_iter()
        .filter(|&(i, _)| named(&arena, &world, i, "Cono"))
        .collect();
    assert_eq!(cones.len(), 3);
    let objects = &arena.asset.objects;
    let throw = objects
        .spawns
        .iter()
        .find(|spawn| matches!(spawn.when, SpawnWhen::Trigger(_)))
        .unwrap();
    let ball = world
        .add_object(
            throw.kind,
            &objects.kinds[throw.kind],
            throw.pos,
            throw.rot,
            throw.velocity,
        )
        .unwrap();
    run(&mut world, 3.0, &[]);
    for &(i, start) in &cones {
        let (pos, rot) = world.objects()[i].pose(1.0);
        assert!(
            pos.distance(start) < 0.05,
            "el cono {i} se fue de {start} a {pos}"
        );
        assert!((rot * Vec3::Y).y > 0.99, "el cono {i} se cayó");
    }
    // La pelota cruza la pista hacia −X, picando: nunca por debajo del piso.
    let (pos, _) = world.objects()[ball].pose(1.0);
    assert!(pos.x < throw.pos.x - 5.0, "la pelota no cruzó: {pos}");
    run(&mut world, 6.0, &[]);
    let (pos, _) = world.objects()[ball].pose(1.0);
    assert!(
        (pos.y - 0.2).abs() < 0.05,
        "la pelota no quedó sobre el piso: {pos}"
    );
}

/// `AI_SliderHandler`: las dos hojas de market2 se abren 2 m, cada una para su lado, y se
/// cierran; suena al cerrar (a los 1.5 s) y al volver a abrir (a los 3 s).
#[test]
fn market2_doors_slide_and_come_back() {
    let market2 = track("market2");
    let mut world = world(&market2);
    let doors: Vec<(usize, Vec3)> = spawn_start(&market2, &mut world)
        .into_iter()
        .filter(|&(i, _)| named(&market2, &world, i, "slider"))
        .collect();
    assert_eq!(doors.len(), 2);
    let mut cues = Vec::new();
    let mut checks = vec![(0.75, 1.0), (1.5, 2.0), (2.25, 1.0), (3.0, 0.0)];
    checks.reverse();
    for frame in 1..=(3.3 / DT).round() as usize {
        run(&mut world, DT, &[]);
        let time = frame as f32 * DT;
        for (i, cue) in world.take_motion_cues() {
            cues.push((i, cue, time));
        }
        if checks
            .last()
            .is_some_and(|&(at, _)| (time - at).abs() < DT / 2.0)
        {
            let (at, open) = checks.pop().unwrap();
            for (&(i, start), side) in doors.iter().zip([-1.0, 1.0]) {
                let (pos, _) = world.objects()[i].pose(1.0);
                let moved = pos - start;
                assert!(
                    (moved.z - side * open).abs() < 0.02,
                    "hoja {i} a los {at} s: {moved}"
                );
                assert!(
                    moved.x.abs() + moved.y.abs() < 1e-3,
                    "la hoja {i} se salió del camino"
                );
            }
        }
    }
    assert!(checks.is_empty());
    for &(door, _) in &doors {
        let heard: Vec<(MotionCue, f32)> = cues
            .iter()
            .filter(|&&(i, _, _)| i == door)
            .map(|&(_, cue, time)| (cue, time))
            .collect();
        assert_eq!(heard.len(), 2, "{heard:?}");
        assert_eq!(heard[0].0, MotionCue::Turn);
        assert!((heard[0].1 - 1.5).abs() < 0.02, "{heard:?}");
        assert_eq!(heard[1].0, MotionCue::Start);
        assert!((heard[1].1 - 3.0).abs() < 0.02, "{heard:?}");
    }
}

/// Las hojas de la arena se abren hacia los costados: la derecha empuja un cono que ya se
/// había dormido y la izquierda al Calcure, sin que nada las frene.
#[test]
fn the_arena_doors_push_what_they_find() {
    let arena = track("revvy_arena");
    let mut world = world(&arena);
    let doors: Vec<(usize, Vec3)> = spawn_start(&arena, &mut world)
        .into_iter()
        .filter(|&(i, _)| named(&arena, &world, i, "Puerta"))
        .collect();
    assert_eq!(doors.len(), 2);
    // Afuera de cada hoja cerrada, que va de x = 0 a ±2: la hoja llega a los 0.6 s.
    let (cone_index, cone) = kind(&arena, "Cono");
    let cone_start = Vec3::new(2.9, 0.1, -8.0);
    let cone = world
        .add_object(cone_index, cone, cone_start, Quat::IDENTITY, Vec3::ZERO)
        .unwrap();
    let calcure = load_car(&content("cars/phim_calcure")).unwrap();
    let car_start = Vec3::new(-2.6, 0.3, -8.0);
    world.add_vehicle(&calcure.vehicle, car_start, 0.0);
    run(&mut world, 0.5, &[Controls::default()]);
    assert!(
        world.objects()[cone].sleeping(world.bodies()),
        "el cono no se durmió"
    );
    run(&mut world, 1.5, &[Controls::default()]);
    let (door_pos, _) = world.objects()[doors[1].0].pose(1.0);
    assert!(
        (door_pos.x - 3.2).abs() < 0.02,
        "la hoja derecha se frenó: {door_pos}"
    );
    // El canto de la hoja mide 8 cm: lo que empuja termina resbalando hacia un costado.
    let (cone_pos, _) = world.objects()[cone].pose(1.0);
    assert!(
        cone_pos.x - cone_start.x > 0.5,
        "la hoja no empujó el cono: {cone_pos}"
    );
    assert!(cone_pos.y > -0.1, "el cono atravesó el piso: {cone_pos}");
    let (car_pos, _) = world.vehicle(0).pose(1.0);
    assert!(
        car_start.x - car_pos.x > 0.5,
        "la hoja no empujó al Calcure: {car_pos}"
    );
}

/// `TrolleyAIHandler`: el chango dado vuelta se endereza solo; sin eso queda panza arriba.
#[test]
fn the_trolley_rights_itself() {
    let arena = track("revvy_arena");
    let mut world = world(&arena);
    let trolley = load_car(&content("cars/trolley")).unwrap();
    let upside_down = Quat::from_rotation_z(std::f32::consts::PI);
    for (x, righting) in [(10.0, Some(0.5)), (20.0, None)] {
        let index = world.add_vehicle(&trolley.vehicle, Vec3::new(x, 1.2, 0.0), 0.0);
        world.place_vehicle(index, Vec3::new(x, 1.2, 0.0), upside_down);
        world.set_self_righting(index, righting);
    }
    run(&mut world, 3.0, &[]);
    let up = |i: usize| world.vehicle(i).pose(1.0).1 * Vec3::Y;
    assert!(up(0).y > 0.95, "el chango no se enderezó: {}", up(0));
    assert!(
        up(1).y < -0.9,
        "sin enderezarse solo queda panza arriba: {}",
        up(1)
    );
}

/// El chango no tiene conductor: queda quieto hasta que el Calcure lo empuja.
#[test]
fn the_calcure_pushes_the_trolley() {
    let arena = track("revvy_arena");
    let mut world = world(&arena);
    let start = arena.asset.layout.start_grid[0].clone();
    let calcure = load_car(&content("cars/phim_calcure")).unwrap();
    world.add_vehicle(&calcure.vehicle, start.pos, start.yaw);
    let trolley = load_car(&content("cars/trolley")).unwrap();
    let ahead = Quat::from_rotation_y(start.yaw) * Vec3::Z;
    let spot = start.pos + ahead * 3.0 + Vec3::Y * 0.3;
    let index = world.add_vehicle(&trolley.vehicle, spot, start.yaw);
    world.set_self_righting(index, Some(0.5));
    run(&mut world, 1.0, &[Controls::default()]);
    let (parked, _) = world.vehicle(index).pose(1.0);
    run(&mut world, 1.0, &[Controls::default()]);
    let still = world.vehicle(index).pose(1.0).0.distance(parked);
    assert!(still < 0.01, "el chango se mueve solo: {still} m");
    let throttle = Controls {
        throttle: 1.0,
        ..Controls::default()
    };
    run(&mut world, 3.0, &[throttle]);
    let (pos, rot) = world.vehicle(index).pose(1.0);
    assert!(
        (pos - parked).dot(ahead) > 0.5,
        "el Calcure no empujó al chango: {parked} → {pos}"
    );
    assert!((rot * Vec3::Y).y > 0.9, "el chango quedó volcado");
}
