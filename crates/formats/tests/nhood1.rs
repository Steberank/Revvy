use std::path::PathBuf;

use revvy_formats::{load_car, load_track, CarStat, Track, TrackLoad};

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

#[test]
fn nhood1_loads_meshes_zones_and_nodes() {
    let dir = repo("../../ServerREVOLT/levels/nhood1");
    let track = load_track(&dir, TrackLoad::default()).expect("nhood1");
    assert_eq!(track.id(), "nhood1");

    let asset = track.asset();
    let visual = asset.visual.as_ref().expect("visual");
    assert!(visual.meshes.len() > 77, "meshes {}", visual.meshes.len());
    assert!(visual.textures.len() >= 10, "bmp {}", visual.textures.len());
    let tris = visual
        .meshes
        .iter()
        .map(|m| m.indices.len() / 3)
        .sum::<usize>();
    assert!(tris > 1000, "tris {tris}");

    let collision = asset.collision.as_ref().expect("collision");
    assert!(
        collision.triangles.len() > 3000,
        "colis {} (faltan los .ncp de las instancias)",
        collision.triangles.len()
    );
    assert!(collision
        .triangles
        .iter()
        .all(|tri| tri.positions.iter().all(|p| p.is_finite())));
    let max_edge = collision
        .triangles
        .iter()
        .map(|tri| {
            let [a, b, c] = tri.positions;
            (a - b).length().max((b - c).length()).max((c - a).length())
        })
        .fold(0.0f32, f32::max);
    assert!(
        max_edge < 220.0,
        "arista de colisión fuera de escala: {max_edge}"
    );

    assert_eq!(asset.layout.zones.len(), 22);
    assert_eq!(asset.layout.pos_nodes.len(), 44);
    assert_eq!(asset.layout.ai_nodes.len(), 214);
    assert!(!asset.layout.pickups.is_empty());
    assert!(!asset.layout.start_grid.is_empty());
    assert!(asset
        .layout
        .ai_nodes
        .iter()
        .all(|n| n.left.is_finite() && n.right.is_finite()));
    assert!(asset.layout.ai_nodes.iter().any(|n| !n.next.is_empty()));

    let start = &asset.layout.start_grid[0];
    let forward = glam::Quat::from_rotation_y(start.yaw) * glam::Vec3::Z;
    let nodes = &asset.layout.ai_nodes;
    let center = |node: &revvy_formats::layout::AiNode| (node.left + node.right) * 0.5;
    let mut id = nodes
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            center(a)
                .distance_squared(start.pos)
                .partial_cmp(&center(b).distance_squared(start.pos))
                .unwrap()
        })
        .map(|(index, _)| index)
        .unwrap();
    let mut traveled = 0.0;
    let mut here = center(&nodes[id]);
    for _ in 0..40 {
        let Some(next) = nodes[id].next.first().copied() else {
            break;
        };
        if next < 0 {
            break;
        }
        let next = next as usize;
        let there = center(&nodes[next]);
        traveled += there.distance(here);
        here = there;
        id = next;
        if traveled > 30.0 {
            break;
        }
    }
    let right = glam::Vec3::Y.cross(forward);
    let ahead = here - start.pos;
    assert!(
        ahead.dot(forward) > 5.0,
        "el auto no mira la pista: adelante={forward:?} hacia={ahead:?}"
    );
    assert!(
        ahead.dot(right) > 2.0,
        "la primera curva de nhood1 no es a la derecha: lateral={}",
        ahead.dot(right)
    );

    // El `.fin` guarda 8 caracteres: WHITEPOS es whitepost, BARRIERP es barrierpole.
    for name in ["whitepos", "barrierp", "ramp1", "bin"] {
        let mesh = visual
            .meshes
            .iter()
            .find(|mesh| mesh.name.eq_ignore_ascii_case(name))
            .unwrap_or_else(|| panic!("falta el modelo {name}"));
        let center = mesh.positions.iter().copied().reduce(|a, b| a + b).unwrap()
            / mesh.positions.len() as f32;
        let nearest = collision
            .triangles
            .iter()
            .flat_map(|tri| tri.positions)
            .map(|point| point.distance(center))
            .fold(f32::MAX, f32::min);
        assert!(
            nearest < 4.0,
            "el .ncp de {name} no coincide con la malla en {center:?}, distancia {nearest}"
        );
    }
}

#[test]
fn pici_fan_header_v256_loads() {
    let dir = repo("../../REVOLT/levels/pici");
    let track = load_track(&dir, TrackLoad::collision_only()).expect("pici");
    assert_eq!(track.asset().layout.ai_nodes.len(), 10);
    assert!(!track.asset().layout.zones.is_empty());
}

#[test]
fn collision_only_skips_visual() {
    let dir = repo("../../ServerREVOLT/levels/nhood1");
    let track = load_track(&dir, TrackLoad::collision_only()).unwrap();
    assert!(track.asset().visual.is_none());
    assert!(track.asset().collision.as_ref().unwrap().triangles.len() > 500);
}

#[test]
fn car_prm_and_parameters_become_car_def() {
    let dir = repo("../../REVOLT/cars/lib16d_maverick");
    let car = load_car(&dir).expect("maverick");
    assert_eq!(car.name, "Maverick");
    assert!(!car.body.is_empty());
    assert!(!car.wheels.is_empty());
    assert!(car.stat(CarStat::Grip).unwrap() > 0.0);
    assert!(car.stat(CarStat::Mass).unwrap() > 0.0);
    assert!(car.stat(CarStat::Engine).is_some());
    assert!(car.stat(CarStat::Steer).is_some());
}

#[test]
fn unknown_car_key_is_kept() {
    let params = revvy_formats::parse_car_text("Name \"Test\"\nNotAStat 3\n");
    assert!(params.unknown.iter().any(|k| k == "notastat"));
    assert_eq!(params.keys.get("notastat").map(String::as_str), Some("3"));
}

#[test]
fn glb_is_not_implemented_and_mixed_folders_fail() {
    let root = std::env::temp_dir().join("revvy-format-phase1");
    let _ = std::fs::remove_dir_all(&root);
    let glb = root.join("harbor");
    std::fs::create_dir_all(&glb).unwrap();
    std::fs::write(glb.join("track.toml"), "format = \"revvy-glb-v1\"\n").unwrap();
    std::fs::write(glb.join("visual.glb"), b"glb").unwrap();
    let err = load_track(&glb, TrackLoad::default()).unwrap_err();
    assert!(err.to_string().contains("no está implementado"), "{err}");

    let mixed = root.join("mixed");
    std::fs::create_dir_all(&mixed).unwrap();
    std::fs::write(mixed.join("mixed.w"), b"w").unwrap();
    std::fs::write(mixed.join("visual.glb"), b"glb").unwrap();
    let err = load_track(&mixed, TrackLoad::default()).unwrap_err();
    assert!(err.to_string().contains("mezcla"), "{err}");
}
