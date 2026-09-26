//! La carrera sobre las pistas de `content/`: un auto que sigue la línea de carrera de la IA
//! (`.fan`), o el camino de POS nodes en la arena propia, da las vueltas de la sala, sin
//! contramano ni reposiciones.

use std::path::PathBuf;

use glam::Vec3;
use revvy_core::race::{CarPose, Race, RaceEvent, RaceTrack};
use revvy_core::rules::GameplayRules;
use revvy_formats::layout::TrackLayout;
use revvy_formats::{load_track, TrackLoad};

const STEP: f32 = 0.5;
const SPEED: f32 = 20.0;

fn layout(level: &str) -> (TrackLayout, revvy_formats::Collision) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../content/levels")
        .join(level);
    let track = load_track(&dir, TrackLoad::collision_only()).unwrap();
    (track.asset.layout, track.asset.collision.unwrap())
}

/// La línea de carrera por `next[0]` desde el nodo más cercano a la largada: el tramo
/// hasta entrar en la vuelta y la vuelta. En market1 y market2 el grafo tiene ramas y la
/// largada no está sobre la vuelta de `next[0]`.
fn racing_line(layout: &TrackLayout) -> (Vec<Vec3>, Vec<Vec3>) {
    let nodes = &layout.ai_nodes;
    let point = |i: usize| nodes[i].left.lerp(nodes[i].right, nodes[i].racing_t);
    let start = layout.start_grid[0].pos;
    let mut id = (0..nodes.len())
        .min_by(|&a, &b| {
            point(a)
                .distance(start)
                .total_cmp(&point(b).distance(start))
        })
        .unwrap();
    let mut order: Vec<usize> = Vec::new();
    while !order.contains(&id) {
        order.push(id);
        id = nodes[id].next[0] as usize;
    }
    let lap_start = order.iter().position(|&node| node == id).unwrap();
    let points = |ids: &[usize]| ids.iter().map(|&i| point(i)).collect();
    (points(&order[..lap_start]), points(&order[lap_start..]))
}

/// La arena no tiene nodos de IA: el camino de POS nodes, que ya está en orden.
fn pos_path(layout: &TrackLayout) -> (Vec<Vec3>, Vec<Vec3>) {
    let line = layout.pos_nodes.iter().map(|node| node.position).collect();
    (Vec::new(), line)
}

fn run_laps(level: &str, laps: u32) {
    let (layout, collision) = layout(level);
    let (lead, line) = if layout.ai_nodes.is_empty() {
        pos_path(&layout)
    } else {
        racing_line(&layout)
    };
    let start = layout.start_grid[0].pos;
    let forward = (lead.first().unwrap_or(&line[0]) - start).normalize();
    let rules = GameplayRules {
        laps,
        ..GameplayRules::default()
    };
    let track = RaceTrack::new(&layout, Some(&collision));
    assert!(track.has_laps(), "{level} sin zonas o camino");
    let mut race = Race::new(
        track,
        &rules,
        &[CarPose {
            pos: start,
            forward,
        }],
    );

    // De la largada a la línea de carrera, y después vueltas enteras y un poco más.
    let mut path = vec![start];
    path.extend(&lead);
    for _ in 0..=laps {
        path.extend(&line);
    }
    path.extend(&line[..line.len() / 10]);
    let mut events = Vec::new();
    let mut wrong_way = 0;
    for pair in path.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let forward = (b - a).normalize_or_zero();
        let steps = ((b - a).length() / STEP).ceil().max(1.0) as usize;
        for i in 1..=steps {
            let pos = a.lerp(b, i as f32 / steps as f32);
            let dt = (b - a).length() / steps as f32 / SPEED;
            events.extend(race.update(dt, &[CarPose { pos, forward }]));
            if race.racer(0).wrong_way() {
                wrong_way += 1;
            }
        }
    }
    let lap_times: Vec<f64> = events
        .iter()
        .filter_map(|event| match *event {
            RaceEvent::Lap { time, .. } => Some(time),
            _ => None,
        })
        .collect();
    assert_eq!(lap_times.len(), laps as usize, "{level}: {events:?}");
    assert!(
        events
            .iter()
            .any(|event| matches!(event, RaceEvent::Finished { .. })),
        "{level} no terminó"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, RaceEvent::Respawn { .. })),
        "{level}: {events:?}"
    );
    assert_eq!(
        wrong_way, 0,
        "{level}: contramano yendo por la línea de carrera"
    );
    // Cada vuelta es la línea de carrera entera, a velocidad constante.
    let line_length: f32 = line
        .windows(2)
        .map(|pair| pair[0].distance(pair[1]))
        .sum::<f32>()
        + line[line.len() - 1].distance(line[0]);
    for time in lap_times {
        let expected = f64::from(line_length / SPEED);
        assert!(
            (time - expected).abs() < 0.5,
            "{level}: vuelta de {time} s, la línea da {expected} s"
        );
    }
}

#[test]
fn nhood1_counts_laps_along_the_racing_line() {
    run_laps("nhood1", 2);
}

#[test]
fn market1_counts_laps_along_the_racing_line() {
    run_laps("market1", 2);
}

#[test]
fn market2_counts_laps_along_the_racing_line() {
    run_laps("market2", 2);
}

#[test]
fn the_glb_arena_counts_laps_along_its_course() {
    run_laps("revvy_arena", 2);
}

/// Una pista de RVGL con sus zonas, POS nodes y triggers de reposición.
#[test]
fn wildland_counts_laps_along_the_racing_line() {
    run_laps("wildland", 2);
}
