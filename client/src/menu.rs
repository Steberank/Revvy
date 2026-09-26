//! Vista del menú: la pista de fondo, una cámara que la recorre despacio por la racing
//! line y el estado de la UI del menú.

use std::path::{Path, PathBuf};

use glam::Vec3;
use revvy_core::rules::GameplayRules;
use revvy_formats::layout::TrackLayout;
use revvy_formats::{load_bmp, load_car, load_track, track_title, TrackLoad, VisualMesh};

use crate::config::ClientConfig;
use crate::drive::resolve_content;
use crate::render::CameraView;
use crate::ui::menu::{MenuState, TrackEntry};

/// Velocidad de la cámara del fondo, en m/s.
const FLYBY_SPEED: f32 = 6.0;
/// Altura del ojo sobre la racing line. En el túnel de nhood1 el techo queda 1.6 m arriba
/// de la línea.
const FLYBY_HEIGHT: f32 = 1.0;
/// La cámara mira hacia el punto de la racing line que está esta distancia más adelante.
const FLYBY_LOOK_AHEAD: f32 = 8.0;
/// Qué tan rápido gira la mirada hacia ese punto (1/s): suaviza las esquinas entre nodos.
/// El ojo no se suaviza: va siempre sobre la racing line, así no corta por las paredes.
const FLYBY_SMOOTHING: f32 = 1.5;
/// La mirada baja un poco: 1 m cada 12.
const FLYBY_TILT: f32 = 1.0 / 12.0;
/// Sin AI nodes gira alrededor de la largada: radio, altura y velocidad (rad/s).
const ORBIT_RADIUS: f32 = 30.0;
const ORBIT_HEIGHT: f32 = 12.0;
const ORBIT_SPEED: f32 = 0.08;

pub struct MenuView {
    track_meshes: Vec<VisualMesh>,
    track_textures: Vec<(i16, image::RgbaImage)>,
    color_key: bool,
    sky: Option<[image::RgbaImage; 6]>,
    background: Option<[u8; 3]>,
    camera: Flyby,
    pub state: MenuState,
}

impl MenuView {
    pub fn load(config: &ClientConfig, rules: &GameplayRules) -> anyhow::Result<Self> {
        let content = config.content_dir();
        let level_dir = resolve_content(&content.join("levels"), &config.level);
        tracing::info!(pista = %level_dir.display(), "fondo del menú");
        let track = load_track(&level_dir, TrackLoad::visual_only())?;
        let visual = track.asset.visual.as_ref();

        let car_dir = resolve_content(&content.join("cars"), &config.car);
        let car_name = match load_car(&car_dir) {
            Ok(car) => car.name,
            Err(err) => {
                tracing::warn!(%err, auto = %car_dir.display(), "auto del jugador");
                config.car.clone()
            }
        };
        let tracks = find_tracks(&content.join("levels"))
            .into_iter()
            .map(|(id, title)| track_entry(&content, &id, &title))
            .collect();
        let mut state = MenuState::new(&config.player_name, &config.car, &car_name, tracks);
        // La sala arranca con las vueltas de las reglas por defecto.
        state.settings.laps = rules.laps;
        Ok(Self {
            track_meshes: visual.map(|v| v.meshes.clone()).unwrap_or_default(),
            track_textures: visual.map(|v| v.textures.clone()).unwrap_or_default(),
            color_key: visual.is_some_and(|v| v.color_key),
            sky: visual.and_then(|v| v.sky.clone()),
            background: visual.and_then(|v| v.background),
            camera: Flyby::new(&track.asset.layout),
            state,
        })
    }

    pub fn track_meshes(&self) -> &[VisualMesh] {
        &self.track_meshes
    }

    pub fn track_textures(&self) -> &[(i16, image::RgbaImage)] {
        &self.track_textures
    }

    pub fn color_key(&self) -> bool {
        self.color_key
    }

    pub fn sky(&self) -> Option<&[image::RgbaImage; 6]> {
        self.sky.as_ref()
    }

    /// El fondo donde no hay cielo.
    pub fn background(&self) -> Option<[u8; 3]> {
        self.background
    }

    pub fn step(&mut self, dt: f32) {
        self.camera.step(dt);
    }

    pub fn camera(&self) -> CameraView {
        self.camera.view()
    }
}

/// Las pistas de `levels/`: cada carpeta con su `.inf` (Re-Volt) o con un `track.toml`
/// `revvy-glb-v1`, ordenadas por título.
fn find_tracks(levels: &Path) -> Vec<(String, String)> {
    let Ok(entries) = std::fs::read_dir(levels) else {
        tracing::warn!(carpeta = %levels.display(), "no se pudo leer la carpeta de pistas");
        return Vec::new();
    };
    let mut tracks: Vec<(String, String)> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            let title = track_title(&entry.path())?;
            Some((entry.file_name().to_string_lossy().into_owned(), title))
        })
        .collect();
    tracks.sort_by(|a, b| {
        a.1.to_lowercase()
            .cmp(&b.1.to_lowercase())
            .then_with(|| a.0.cmp(&b.0))
    });
    tracks
}

/// Una pista de la lista con su portada: `gfx/<id>.bmp` en las de Re-Volt, `preview.png`
/// en la carpeta de las `.glb`.
fn track_entry(content: &Path, id: &str, title: &str) -> TrackEntry {
    let candidates = [
        (content.join("gfx"), format!("{id}.bmp")),
        (content.join("levels").join(id), "preview.png".to_string()),
    ];
    let path = candidates
        .iter()
        .find_map(|(dir, name)| find_file(dir, name));
    let cover = path.and_then(|path| match load_bmp(&path) {
        Ok(image) => {
            let size = [image.width() as usize, image.height() as usize];
            Some(egui::ColorImage::from_rgba_unmultiplied(
                size,
                image.as_raw(),
            ))
        }
        Err(err) => {
            tracing::warn!(%err, "portada de la pista");
            None
        }
    });
    if cover.is_none() {
        tracing::info!(pista = id, "pista sin portada");
    }
    TrackEntry::new(id, title, cover)
}

/// `name` dentro de `dir` sin distinguir mayúsculas, como guarda Re-Volt los `.bmp`.
fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|file| file.to_str())
                    .is_some_and(|file| file.eq_ignore_ascii_case(name))
        })
}

/// Cámara del fondo del menú.
enum Flyby {
    /// Recorre la racing line, que es cerrada: el último punto repite el primero.
    Path {
        points: Vec<Vec3>,
        /// Distancia recorrida hasta cada punto.
        distances: Vec<f32>,
        travelled: f32,
        /// Hacia dónde mira, suavizado.
        look: Vec3,
    },
    /// Pista sin AI nodes: gira alrededor de la largada.
    Orbit { center: Vec3, angle: f32 },
}

impl Flyby {
    fn new(layout: &TrackLayout) -> Self {
        let mut points = racing_line(layout);
        if let Some(&first) = points.first() {
            points.push(first);
        }
        let mut distances = vec![0.0];
        for pair in points.windows(2) {
            distances.push(distances[distances.len() - 1] + pair[0].distance(pair[1]));
        }
        if points.len() < 4 || distances[distances.len() - 1] < 1.0 {
            let center = layout
                .start_grid
                .first()
                .map_or(Vec3::ZERO, |slot| slot.pos);
            return Self::Orbit { center, angle: 0.0 };
        }
        let look = look_ahead(&points, &distances, 0.0);
        Self::Path {
            points,
            distances,
            travelled: 0.0,
            look,
        }
    }

    fn step(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        match self {
            Self::Path {
                points,
                distances,
                travelled,
                look,
            } => {
                *travelled += FLYBY_SPEED * dt;
                let want = look_ahead(points, distances, *travelled);
                let follow = 1.0 - (-FLYBY_SMOOTHING * dt).exp();
                *look = look.lerp(want, follow).try_normalize().unwrap_or(want);
            }
            Self::Orbit { angle, .. } => *angle += ORBIT_SPEED * dt,
        }
    }

    fn view(&self) -> CameraView {
        match self {
            Self::Path {
                points,
                distances,
                travelled,
                look,
            } => {
                let eye = along(points, distances, *travelled) + Vec3::Y * FLYBY_HEIGHT;
                CameraView {
                    eye,
                    target: eye + *look - Vec3::Y * FLYBY_TILT,
                }
            }
            Self::Orbit { center, angle } => CameraView {
                eye: *center
                    + Vec3::new(
                        angle.cos() * ORBIT_RADIUS,
                        ORBIT_HEIGHT,
                        angle.sin() * ORBIT_RADIUS,
                    ),
                target: *center + Vec3::Y,
            },
        }
    }
}

/// Hacia dónde sigue la racing line desde `travelled`, como dirección unitaria.
fn look_ahead(points: &[Vec3], distances: &[f32], travelled: f32) -> Vec3 {
    let here = along(points, distances, travelled);
    let ahead = along(points, distances, travelled + FLYBY_LOOK_AHEAD);
    (ahead - here).try_normalize().unwrap_or(Vec3::Z)
}

/// El punto a `s` metros del inicio de un recorrido cerrado, dando vueltas.
fn along(points: &[Vec3], distances: &[f32], s: f32) -> Vec3 {
    let total = distances[distances.len() - 1];
    let s = s.rem_euclid(total);
    let i = distances
        .partition_point(|&d| d <= s)
        .clamp(1, points.len() - 1);
    let (from, to) = (distances[i - 1], distances[i]);
    let t = if to > from {
        (s - from) / (to - from)
    } else {
        0.0
    };
    points[i - 1].lerp(points[i], t)
}

/// La racing line desde el nodo más cercano a la largada, siguiendo `next` (la rama de
/// carrera cuando hay dos) hasta volver al primero.
fn racing_line(layout: &TrackLayout) -> Vec<Vec3> {
    let nodes = &layout.ai_nodes;
    if nodes.is_empty() {
        return Vec::new();
    }
    let point = |i: usize| nodes[i].left.lerp(nodes[i].right, nodes[i].racing_t);
    let start = layout
        .start_grid
        .first()
        .map_or_else(|| point(0), |slot| slot.pos);
    let first = (0..nodes.len())
        .min_by(|&a, &b| {
            point(a)
                .distance(start)
                .total_cmp(&point(b).distance(start))
        })
        .unwrap_or(0);
    let mut points = Vec::new();
    let mut id = first;
    for _ in 0..nodes.len() {
        points.push(point(id));
        let next = &nodes[id].next;
        let Some(&chosen) = next
            .iter()
            .find(|&&n| nodes[n as usize].flags.racing)
            .or_else(|| next.first())
        else {
            break;
        };
        id = chosen as usize;
        if id == first {
            break;
        }
    }
    points
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use revvy_formats::Track;

    #[test]
    fn every_track_folder_is_listed_by_title() {
        let root = std::env::temp_dir().join(format!("revvy-pistas-{}", std::process::id()));
        let levels = root.join("levels");
        for dir in ["vieja", "arena", "vacia"] {
            std::fs::create_dir_all(levels.join(dir)).unwrap();
        }
        std::fs::write(levels.join("vieja/vieja.inf"), "NAME\t'Zeta'\n").unwrap();
        let manifest = "name = \"Alfa\"\nformat = \"revvy-glb-v1\"\n";
        std::fs::write(levels.join("arena/track.toml"), manifest).unwrap();
        std::fs::write(levels.join("suelto.txt"), "").unwrap();
        let found = find_tracks(&levels);
        std::fs::remove_dir_all(&root).unwrap();
        let expected = [("arena", "Alfa"), ("vieja", "Zeta")]
            .map(|(id, title)| (id.to_string(), title.to_string()));
        assert_eq!(found, expected, "sin la carpeta vacía ni el archivo suelto");

        let content = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../content/levels");
        let ids: Vec<String> = find_tracks(&content)
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert!(
            ids.contains(&"nhood1".to_string()) && ids.contains(&"revvy_arena".to_string()),
            "{ids:?}"
        );
    }

    #[test]
    fn along_wraps_around_the_loop() {
        let points = [
            Vec3::ZERO,
            Vec3::X * 10.0,
            Vec3::new(10.0, 0.0, 10.0),
            Vec3::ZERO,
        ];
        let distances = [0.0, 10.0, 20.0, 20.0 + 200f32.sqrt()];
        assert!((along(&points, &distances, 5.0) - Vec3::X * 5.0).length() < 1e-5);
        assert!((along(&points, &distances, 15.0) - Vec3::new(10.0, 0.0, 5.0)).length() < 1e-5);
        let lap = distances[3];
        assert!((along(&points, &distances, lap + 5.0) - Vec3::X * 5.0).length() < 1e-4);
    }

    #[test]
    fn the_menu_camera_laps_nhood1_on_the_racing_line() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../content/levels/nhood1");
        let track = load_track(&dir, TrackLoad::collision_only()).expect("nhood1");
        let line = racing_line(&track.asset().layout);
        let lap: f32 = line
            .windows(2)
            .map(|pair| pair[0].distance(pair[1]))
            .sum::<f32>()
            + line[line.len() - 1].distance(line[0]);
        // La vuelta de nhood1 mide unos 740 m (`AiNodeTotalDist`).
        assert!((700.0..800.0).contains(&lap), "vuelta de {lap} m");
        let start = track.asset().layout.start_grid[0].pos;
        assert!(line[0].distance(start) < 5.0, "arranca en la largada");
        assert!(matches!(
            Flyby::new(&track.asset().layout),
            Flyby::Path { .. }
        ));
    }

    /// El ojo va siempre por el aire libre: con piso debajo y, si hay techo, por debajo del
    /// techo. Cubre el túnel del final de la vuelta, donde el techo queda a 1.6 m de la
    /// racing line.
    #[test]
    fn the_menu_camera_never_goes_inside_nhood1() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../content/levels/nhood1");
        let track = load_track(&dir, TrackLoad::collision_only()).expect("nhood1");
        let collision = track.asset().collision.as_ref().expect("colisión");
        let mut camera = Flyby::new(&track.asset().layout);
        let Flyby::Path { distances, .. } = &camera else {
            panic!("nhood1 tiene racing line");
        };
        let lap_secs = distances[distances.len() - 1] / FLYBY_SPEED;
        let mut secs = 0.0;
        while secs < lap_secs {
            for _ in 0..15 {
                camera.step(1.0 / 60.0);
            }
            secs += 0.25;
            let eye = camera.view().eye;
            // La superficie más cercana debajo y encima del ojo, con su cara (+ arriba).
            let (mut below, mut above) = (None::<(f32, f32)>, None::<(f32, f32)>);
            for tri in &collision.triangles {
                let Some(height) = height_over(eye, &tri.positions) else {
                    continue;
                };
                let [a, b, c] = tri.positions;
                let facing = (b - a).cross(c - a).y;
                let (slot, distance) = if height <= 0.0 {
                    (&mut below, -height)
                } else {
                    (&mut above, height)
                };
                if slot.is_none_or(|(nearest, _)| distance < nearest) {
                    *slot = Some((distance, facing));
                }
            }
            let (floor, facing) =
                below.unwrap_or_else(|| panic!("sin piso bajo el ojo a los {secs} s: {eye:?}"));
            assert!(
                facing > 0.0 && floor < 3.0,
                "el ojo no está sobre el piso a los {secs} s: {eye:?}"
            );
            if let Some((ceiling, facing)) = above {
                assert!(
                    facing < 0.0 && ceiling > 0.3,
                    "el ojo toca un techo a los {secs} s: {eye:?}"
                );
            }
        }
    }

    /// Altura del triángulo en la vertical de `point`, relativa a él, si la vertical lo cruza.
    fn height_over(point: Vec3, tri: &[Vec3; 3]) -> Option<f32> {
        let [a, b, c] = *tri;
        let (ab, ac, ap) = (b - a, c - a, point - a);
        let den = ab.x * ac.z - ac.x * ab.z;
        if den.abs() < 1e-9 {
            return None;
        }
        let u = (ap.x * ac.z - ac.x * ap.z) / den;
        let v = (ab.x * ap.z - ap.x * ab.z) / den;
        (u >= 0.0 && v >= 0.0 && u + v <= 1.0).then(|| a.y + u * ab.y + v * ac.y - point.y)
    }
}
