use std::path::PathBuf;

use revvy_formats::{load_car, load_track, track_title, CarStat, Track, TrackLoad};

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

#[test]
fn nhood1_loads_meshes_zones_and_nodes() {
    let dir = repo("../../content/levels/nhood1");
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

    // La carrera va en el orden de las zonas: la zona 1 queda adelante de la largada y la
    // primera curva dobla a la derecha.
    let start = &asset.layout.start_grid[0];
    let forward = glam::Quat::from_rotation_y(start.yaw) * glam::Vec3::Z;
    let right = forward.cross(glam::Vec3::Y);
    let zone = |id: i32| {
        asset
            .layout
            .zones
            .iter()
            .find(|zone| zone.id == id)
            .map(|zone| zone.center - start.pos)
            .unwrap_or_else(|| panic!("falta la zona {id}"))
    };
    assert!(zone(1).dot(forward) > 10.0, "el auto no mira la pista: adelante={forward:?}");
    assert!(zone(2).dot(right) > 10.0, "la primera curva de nhood1 no es a la derecha");

    // El segundo puesto de la grilla de a dos va a la derecha y un poco atrás.
    let second = asset.layout.start_grid[1].pos - start.pos;
    assert!((second.dot(right) - 1.28).abs() < 0.01, "puesto 2 al costado: {second:?}");
    assert!((second.dot(forward) + 0.2).abs() < 0.01, "puesto 2 atrás: {second:?}");

    // Sonidos del nivel: el banco de nhood1 y sus objetos, ya en metros.
    let bank = asset.sounds.bank.as_ref().expect("banco de nhood1");
    assert_eq!(bank.folder, "hood");
    assert!(asset.sounds.emitters.len() >= 5, "emisores {}", asset.sounds.emitters.len());
    assert!(visual.color_key && visual.sky.is_some());
    // `FOGCOLOR 80 144 192` del `.inf`: el fondo donde no hay cielo.
    assert_eq!(visual.background, Some([80, 144, 192]));

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
fn nhood1_ai_nodes_follow_the_race() {
    let dir = repo("../../content/levels/nhood1");
    let track = load_track(&dir, TrackLoad::collision_only()).expect("nhood1");
    let layout = &track.asset().layout;
    let nodes = &layout.ai_nodes;
    let center = |id: usize| (nodes[id].left + nodes[id].right) * 0.5;

    for node in nodes {
        for &next in &node.next {
            let next = &nodes[next as usize];
            assert!(next.prev.contains(&(node.id as i32)), "{} no vuelve a {}", next.id, node.id);
            // El extremo verde queda a la izquierda del sentido de carrera.
            let left = glam::Vec3::Y.cross(center(next.id as usize) - center(node.id as usize));
            assert!((node.left - node.right).dot(left) > 0.0, "nodo {} al revés", node.id);
        }
    }

    // Desde el nodo más cercano a la largada, `next` recorre las zonas en orden: primero la
    // 1, que queda adelante, y al final la 19, que queda atrás. Las zonas se superponen
    // porque la pista va y vuelve por las mismas calles.
    let start = layout.start_grid[0].pos;
    let first = (0..nodes.len())
        .min_by(|&a, &b| center(a).distance(start).total_cmp(&center(b).distance(start)))
        .unwrap();
    let zones_at = |point: glam::Vec3| -> Vec<i32> {
        layout
            .zones
            .iter()
            .filter(|zone| {
                let local = zone.rotation.inverse() * (point - zone.center);
                local.abs().cmple(zone.half_extents).all()
            })
            .map(|zone| zone.id)
            .collect()
    };
    let mut zone = 0;
    let mut id = first;
    let mut length = 0.0;
    for _ in 0..nodes.len() {
        let inside = zones_at(center(id));
        if !inside.is_empty() && !inside.contains(&zone) {
            assert!(inside.contains(&(zone + 1)), "el nodo {id} salta de la zona {zone} a {inside:?}");
            zone += 1;
        }
        let next = nodes[id].next[0] as usize;
        length += center(id).distance(center(next));
        id = next;
        if id == first {
            break;
        }
    }
    assert_eq!(id, first, "la vuelta por next no cierra");
    let last = layout.zones.iter().map(|zone| zone.id).max().unwrap();
    assert_eq!(zone, last, "la vuelta no pasa por todas las zonas");
    // `AiNodeTotalDist` de nhood1 es 148003 unidades: 740 m.
    assert!((length - 740.0).abs() < 10.0, "vuelta de {length} m");
}

#[test]
fn track_titles_come_from_inf_and_track_toml() {
    let title = |rel: &str| track_title(&repo(rel));
    assert_eq!(title("../../content/levels/nhood1").as_deref(), Some("Toys in the Hood 1"));
    assert_eq!(title("../../content/levels/revvy_arena").as_deref(), Some("Revvy Arena"));
    assert_eq!(title("../../content/levels/no_existe"), None);
}

#[test]
fn pici_loads_ai_nodes_and_zones() {
    let dir = repo("../../REVOLT/levels/pici");
    let track = load_track(&dir, TrackLoad::collision_only()).expect("pici");
    assert_eq!(track.asset().layout.ai_nodes.len(), 10);
    assert!(!track.asset().layout.zones.is_empty());
}

#[test]
fn collision_only_skips_visual() {
    let dir = repo("../../content/levels/nhood1");
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
    assert!(car.wheels.iter().all(|wheel| !wheel.is_empty()));
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
fn a_broken_glb_track_and_mixed_folders_fail() {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("revvy-format-mixed");
    let _ = std::fs::remove_dir_all(&root);
    let glb = root.join("harbor");
    std::fs::create_dir_all(&glb).unwrap();
    std::fs::write(glb.join("track.toml"), "format = \"revvy-glb-v1\"\n").unwrap();
    std::fs::write(glb.join("visual.glb"), b"glb").unwrap();
    assert!(load_track(&glb, TrackLoad::default()).is_err());

    let mixed = root.join("mixed");
    std::fs::create_dir_all(&mixed).unwrap();
    std::fs::write(mixed.join("mixed.w"), b"w").unwrap();
    std::fs::write(mixed.join("visual.glb"), b"glb").unwrap();
    let err = load_track(&mixed, TrackLoad::default()).unwrap_err();
    assert!(err.to_string().contains("mezcla"), "{err}");
}

#[test]
fn a_revvy_car_never_loads_with_revolt_parameters() {
    // El auto propio de prueba, con un `parameters.txt` de Re-Volt al lado.
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("revvy_car_with_parameters");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for file in ["car.toml", "body.glb", "collision.glb"] {
        std::fs::copy(repo("../../content/cars/revvy_buggy").join(file), dir.join(file)).unwrap();
    }
    std::fs::copy(
        repo("../../content/cars/phim_calcure/parameters.txt"),
        dir.join("parameters.txt"),
    )
    .unwrap();
    let car = load_car(&dir).expect("auto propio");
    assert!(car.revolt.is_none(), "se cargó como auto de Re-Volt");
    assert_eq!(car.name, "Revvy Buggy");
    assert_eq!(car.vehicle.mass, 1.6);
}

#[test]
fn own_car_loads_meshes_wheels_and_hull() {
    let car = load_car(&repo("../../content/cars/revvy_buggy")).expect("revvy_buggy");
    assert!(!car.body.is_empty());
    assert!(car.wheels.iter().all(|wheel| !wheel.is_empty()));
    // La rueda se dibuja centrada en su buje.
    let hub = car.wheels[0][0].positions.iter().copied().reduce(|a, b| a + b).unwrap()
        / car.wheels[0][0].positions.len() as f32;
    assert!(hub.length() < 0.01, "rueda corrida del buje: {hub:?}");
    assert_eq!(car.vehicle.chassis.spheres.len(), 5);
    assert_eq!(car.vehicle.chassis.hulls.len(), 1);
    assert!(car.vehicle.wheels[2].powered && car.vehicle.wheels[0].steered);
}

#[test]
fn calcure_translates_to_revvy_units() {
    let car = load_car(&repo("../../content/cars/phim_calcure")).expect("calcure");
    let v = &car.vehicle;
    // 5 mm por unidad: gravedad 2200 → 11 m/s², 50 mph de Re-Volt → 22.4 m/s.
    assert!((v.gravity - 11.0).abs() < 1e-4);
    assert!((v.top_speed - 22.36).abs() < 0.01, "{}", v.top_speed);
    assert!((v.wheels[0].radius - 0.055).abs() < 1e-5);
    // La delantera izquierda queda del lado +X (izquierda) y adelante.
    assert!(v.wheels[0].offset[0] > 0.1 && v.wheels[0].offset[2] > 0.2);
    assert!(v.wheels[1].offset[0] < -0.1);
    // El casco convexo mide lo que el auto: 30 cm de ancho y 68 cm de largo.
    let points: Vec<[f32; 3]> = v.chassis.hulls.iter().flatten().copied().collect();
    let width = points.iter().map(|p| p[0]).fold(f32::MIN, f32::max) - points.iter().map(|p| p[0]).fold(f32::MAX, f32::min);
    let length = points.iter().map(|p| p[2]).fold(f32::MIN, f32::max) - points.iter().map(|p| p[2]).fold(f32::MAX, f32::min);
    assert!((width - 0.30).abs() < 0.02 && (length - 0.68).abs() < 0.03, "{width} × {length}");
    assert!(!v.chassis.spheres.is_empty());
    assert!(car.revolt.is_some());
}

#[test]
fn glb_arena_loads_visual_collision_surfaces_and_grid() {
    let track = load_track(&repo("../../content/levels/revvy_arena"), TrackLoad::default()).expect("arena");
    let asset = track.asset();
    let visual = asset.visual.as_ref().unwrap();
    assert!(!visual.meshes.is_empty() && !visual.color_key);
    assert_eq!(visual.background, None, "una .glb usa el fondo de Revvy");
    let collision = asset.collision.as_ref().unwrap();
    let surfaces: std::collections::BTreeSet<&str> =
        collision.triangles.iter().map(|tri| tri.surface.name()).collect();
    for surface in ["Road", "Ice", "Grass", "Dirt", "Pebbles", "Wood", "Stone"] {
        assert!(surfaces.contains(surface), "falta {surface}: {surfaces:?}");
    }
    // El piso mira hacia arriba.
    let floor = collision.triangles.iter().find(|tri| tri.surface.name() == "Road").unwrap();
    let [a, b, c] = floor.positions;
    assert!((b - a).cross(c - a).normalize().y > 0.99);
    assert_eq!(asset.layout.start_grid.len(), 4);
    assert!(asset.legacy.is_none());
}
