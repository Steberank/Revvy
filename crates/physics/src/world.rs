//! Motor de física de Revvy: Rapier para cuerpos rígidos, choques y consultas, con el
//! vehículo de Revvy encima (`vehicle_controller`). Todo en metros, con Y arriba.
//!
//! La pista entra como triángulos con su superficie (`Collision`), venga de un `.ncp`
//! de Re-Volt traducido o de una `.glb`: el motor no distingue una de otra. Los autos
//! entran con sus `VehicleParams`, sean de Re-Volt o propios.

use glam::Vec3;
use rapier3d::parry::bounding_volume::Aabb;
use rapier3d::parry::query::{Ray, RayCast};
use rapier3d::parry::shape::{TriMesh, TriMeshFlags};
use rapier3d::prelude::*;
use revvy_formats::{Collision, SurfaceType, VehicleParams};

use crate::surfaces;
use crate::vehicle_controller::{CarMaterial, Controls, Vehicle};

/// Gravedad del mundo (m/s²): la de Re-Volt, 2200 unidades de 5 mm por segundo².
pub const GRAVITY: f32 = 11.0;
/// Paso fijo del motor (s). Re-Volt a 60 cuadros por segundo da tres pasos de este largo.
pub const TICK: f32 = 1.0 / 180.0;
/// Los mandos se leen cada tantos pasos: 60 veces por segundo, como Re-Volt a 60
/// cuadros. El volante de Re-Volt da su primer paso desde el centro ×4 por cuadro, así
/// que la frecuencia de lectura es parte del manejo.
const CONTROL_TICKS: u32 = 3;
/// Un frame no simula más de esto (`UpdateTimeFactor` corta en 10/72 s).
const MAX_FRAME: f32 = 10.0 / 72.0;
/// `COLL_EPSILON` (2 unidades de Re-Volt): tolerancia de los contactos de esfera.
const COLL_EPSILON: f32 = 0.01;
/// `SMALL_REAL` de Re-Volt, en metros.
const SMALL: f32 = 5e-8;

pub(crate) const GROUP_WORLD: Group = Group::GROUP_1;
pub(crate) const GROUP_CAMERA_ONLY: Group = Group::GROUP_2;
pub(crate) const GROUP_OBJECT_ONLY: Group = Group::GROUP_3;
pub(crate) const GROUP_CAR_SKIN: Group = Group::GROUP_4;
pub(crate) const GROUP_CAR_HULL: Group = Group::GROUP_5;

/// `user_data` de los colliders: tipo en la parte alta, índice en la baja.
const TAG_TRACK: u128 = 1 << 64;
pub(crate) const TAG_CAR: u128 = 2 << 64;
const TAG_MASK: u128 = !0u128 << 64;

/// Quién consulta la pista: las ruedas y los autos, o la cámara.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Viewer {
    Objects,
    Camera,
}

/// Una malla de la pista en Rapier y la superficie de cada triángulo.
pub(crate) struct TrackMesh {
    pub collider: ColliderHandle,
    pub surfaces: Vec<SurfaceType>,
    pub camera: bool,
    pub objects: bool,
}

/// Contacto de una esfera contra un triángulo de la pista, como `SphereCollPoly`: la
/// cara o un solo borde. Un vértice no cuenta.
#[derive(Clone, Copy, Debug)]
pub struct SphereHit {
    /// Sale de la superficie hacia el centro de la esfera.
    pub normal: Vec3,
    /// Normal del triángulo.
    pub face_normal: Vec3,
    /// Del centro de la esfera al punto de contacto.
    pub rel_pos: Vec3,
    /// Punto de contacto sobre la superficie.
    pub world_pos: Vec3,
    /// Negativa cuando la esfera entra en la superficie.
    pub depth: f32,
    pub surface: SurfaceType,
}

pub struct PhysicsWorld {
    bodies: RigidBodySet,
    colliders: ColliderSet,
    pipeline: PhysicsPipeline,
    integration: IntegrationParameters,
    islands: IslandManager,
    broad_phase: BroadPhaseBvh,
    narrow_phase: NarrowPhase,
    impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    ccd: CCDSolver,
    track: Vec<TrackMesh>,
    vehicles: Vec<Vehicle>,
    accumulator: f32,
    ticks: u32,
}

impl PhysicsWorld {
    /// La pista va en hasta tres mallas: la común, la que solo frena la cámara y la que
    /// la cámara atraviesa.
    pub fn new(collision: &Collision) -> Self {
        let mut colliders = ColliderSet::new();
        let mut track = Vec::new();
        for (camera, objects) in [(true, true), (true, false), (false, true)] {
            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            let mut surfaces = Vec::new();
            for tri in &collision.triangles {
                let wants = match (tri.camera_only, tri.object_only) {
                    (true, _) => camera && !objects,
                    (false, true) => objects && !camera,
                    (false, false) => camera && objects,
                };
                let [a, b, c] = tri.positions;
                if !wants || (b - a).cross(c - a).length_squared() < 1e-12 {
                    continue;
                }
                let base = vertices.len() as u32;
                vertices.extend([a, b, c]);
                indices.push([base, base + 1, base + 2]);
                surfaces.push(tri.surface);
            }
            if indices.is_empty() {
                continue;
            }
            let (member, filter) = match (camera, objects) {
                (true, true) => (GROUP_WORLD, GROUP_CAR_SKIN),
                (true, false) => (GROUP_CAMERA_ONLY, Group::NONE),
                _ => (GROUP_OBJECT_ONLY, GROUP_CAR_SKIN),
            };
            let shape = build_trimesh(vertices, indices, surfaces.len());
            let collider = ColliderBuilder::new(SharedShape::new(shape))
                .collision_groups(InteractionGroups::new(member, filter, InteractionTestMode::And))
                .user_data(TAG_TRACK | track.len() as u128)
                .build();
            track.push(TrackMesh {
                collider: colliders.insert(collider),
                surfaces,
                camera,
                objects,
            });
        }
        Self {
            bodies: RigidBodySet::new(),
            colliders,
            pipeline: PhysicsPipeline::new(),
            // Sin clusters, cada contacto del chasis con la pista es de un triángulo: el hook
            // de superficies lee su material y los contactos quedan en `manifolds`.
            integration: IntegrationParameters {
                dt: TICK,
                contact_clustering: false,
                ..IntegrationParameters::default()
            },
            islands: IslandManager::new(),
            broad_phase: BroadPhaseBvh::new(),
            narrow_phase: NarrowPhase::new(),
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            ccd: CCDSolver::new(),
            track,
            vehicles: Vec::new(),
            accumulator: 0.0,
            ticks: 0,
        }
    }

    /// Agrega un auto con su centro de masa en `pos`, mirando con `yaw` sobre +Y.
    pub fn add_vehicle(&mut self, params: &VehicleParams, pos: Vec3, yaw: f32) -> usize {
        let index = self.vehicles.len();
        let vehicle = Vehicle::spawn(params, index, pos, yaw, &mut self.bodies, &mut self.colliders);
        self.vehicles.push(vehicle);
        index
    }

    /// Un frame: pasos fijos de `TICK` hasta consumir el tiempo. `controls[i]` maneja el
    /// auto `i`; los que no tienen mandos quedan quietos.
    pub fn frame(&mut self, dt: f32, controls: &[Controls]) {
        for (i, vehicle) in self.vehicles.iter_mut().enumerate() {
            vehicle.begin_frame(&controls.get(i).copied().unwrap_or_default());
        }
        self.accumulator += dt.clamp(0.0, MAX_FRAME);
        while self.accumulator >= TICK {
            self.tick(controls);
            self.accumulator -= TICK;
        }
    }

    /// Cuánto del próximo paso pasó ya (0–1), para interpolar el dibujo.
    pub fn alpha(&self) -> f32 {
        self.accumulator / TICK
    }

    fn tick(&mut self, controls: &[Controls]) {
        let control_dt = (self.ticks % CONTROL_TICKS == 0).then_some(TICK * CONTROL_TICKS as f32);
        self.ticks = self.ticks.wrapping_add(1);
        for (i, vehicle) in self.vehicles.iter_mut().enumerate() {
            let controls = controls.get(i).copied().unwrap_or_default();
            vehicle.pre_step(&controls, control_dt, &mut self.bodies, &self.colliders, &self.track, TICK);
        }
        let materials: Vec<CarMaterial> = self.vehicles.iter().map(Vehicle::material).collect();
        let hooks = SurfaceHooks {
            track: &self.track,
            cars: &materials,
        };
        self.pipeline.step(
            Vec3::new(0.0, -GRAVITY, 0.0),
            &self.integration,
            &mut self.islands,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.ccd,
            &hooks,
            &(),
        );
        for vehicle in &mut self.vehicles {
            vehicle.post_step(&mut self.bodies, &mut self.colliders, &self.narrow_phase, &self.track, TICK);
        }
    }

    /// Pone un auto en una pose, quieto. Sirve para respawnear.
    pub fn place_vehicle(&mut self, index: usize, pos: Vec3, rot: glam::Quat) {
        self.vehicles[index].place(&mut self.bodies, pos, rot);
    }

    pub fn vehicles(&self) -> &[Vehicle] {
        &self.vehicles
    }

    pub fn vehicle(&self, index: usize) -> &Vehicle {
        &self.vehicles[index]
    }

    pub fn bodies(&self) -> &RigidBodySet {
        &self.bodies
    }

    /// Contactos de una esfera que va de `old` a `new` contra la pista.
    pub fn sphere_hits(&self, old: Vec3, new: Vec3, radius: f32, viewer: Viewer) -> Vec<SphereHit> {
        let mut hits = Vec::new();
        track_sphere_hits(&self.colliders, &self.track, old, new, radius, viewer, &mut hits);
        hits
    }

    /// `LineOfSight`: nada de la pista que vea la cámara corta el segmento.
    pub fn line_of_sight(&self, from: Vec3, to: Vec3) -> bool {
        let delta = to - from;
        let length = delta.length();
        if length < SMALL {
            return true;
        }
        let ray = Ray::new(from, delta / length);
        !self
            .track
            .iter()
            .filter(|mesh| mesh.camera)
            .filter_map(|mesh| self.colliders.get(mesh.collider)?.shape().as_trimesh())
            .any(|mesh| mesh.cast_local_ray(&ray, length, true).is_some())
    }
}

/// Malla de la pista para Rapier. Se unen los vértices repetidos para que el chasis no se
/// trabe en los bordes internos; si eso cambia los triángulos, va sin arreglos.
fn build_trimesh(vertices: Vec<Vec3>, indices: Vec<[u32; 3]>, count: usize) -> TriMesh {
    if let Ok(mesh) = TriMesh::with_flags(vertices.clone(), indices.clone(), TriMeshFlags::FIX_INTERNAL_EDGES) {
        if mesh.num_triangles() == count {
            return mesh;
        }
    }
    TriMesh::new(vertices, indices).expect("malla de pista con índices válidos")
}

/// Índice en la lista de mallas de la pista, si el collider es de la pista.
pub(crate) fn track_mesh_of(collider: &Collider) -> Option<usize> {
    (collider.user_data & TAG_MASK == TAG_TRACK).then_some((collider.user_data & !TAG_MASK) as usize)
}

/// Contactos de una esfera contra las mallas de la pista que ve `viewer`.
pub(crate) fn track_sphere_hits(
    colliders: &ColliderSet,
    track: &[TrackMesh],
    old: Vec3,
    new: Vec3,
    radius: f32,
    viewer: Viewer,
    out: &mut Vec<SphereHit>,
) {
    let reach = Vec3::splat(radius + COLL_EPSILON);
    let aabb = Aabb::new(old.min(new) - reach, old.max(new) + reach);
    for mesh in track {
        let visible = match viewer {
            Viewer::Objects => mesh.objects,
            Viewer::Camera => mesh.camera,
        };
        if !visible {
            continue;
        }
        let Some(trimesh) = colliders.get(mesh.collider).and_then(|c| c.shape().as_trimesh()) else {
            continue;
        };
        for index in trimesh.bvh().intersect_aabb(&aabb) {
            let tri = trimesh.triangle(index);
            if let Some(mut hit) = sphere_triangle(old, new, radius, [tri.a, tri.b, tri.c]) {
                hit.surface = mesh.surfaces[index as usize];
                out.push(hit);
            }
        }
    }
}

/// `SphereCollPoly` (PC) sobre un triángulo: la cara, o un solo borde si el centro cae
/// afuera de uno. La esfera que ya estaba del todo detrás del plano no choca.
pub(crate) fn sphere_triangle(old: Vec3, new: Vec3, radius: f32, [a, b, c]: [Vec3; 3]) -> Option<SphereHit> {
    let n = (b - a).cross(c - a).try_normalize()?;
    let d = -n.dot(a);
    let new_dist = n.dot(new) + d;
    if new_dist - radius > COLL_EPSILON {
        return None;
    }
    let old_dist = n.dot(old) + d;
    if old_dist < -(radius + COLL_EPSILON) {
        return None;
    }
    let corners = [a, b, c];
    let mut edges = [(Vec3::ZERO, 0.0f32); 3];
    let mut dist = [0.0f32; 3];
    let mut outside = 0;
    for i in 0..3 {
        let (p, q) = (corners[i], corners[(i + 1) % 3]);
        let en = (q - p).cross(n).normalize_or_zero();
        edges[i] = (en, -en.dot(p));
        dist[i] = en.dot(new) + edges[i].1;
        if dist[i] >= 0.0 {
            outside += 1;
        }
    }

    if outside == 0 {
        return Some(SphereHit {
            normal: n,
            face_normal: n,
            rel_pos: n * -radius,
            world_pos: new - n * new_dist,
            depth: new_dist - radius,
            surface: SurfaceType::Road,
        });
    }
    if outside != 1 {
        return None;
    }
    let edge = (0..3).find(|&i| dist[i] >= 0.0)?;
    let to_contact = edges[edge].0 * -dist[edge] - n * new_dist;
    let length = to_contact.length();
    if length > radius {
        return None;
    }
    let world_pos = new + to_contact;
    let lo = a.min(b).min(c) - Vec3::splat(COLL_EPSILON);
    let hi = a.max(b).max(c) + Vec3::splat(COLL_EPSILON);
    if world_pos.cmplt(lo).any() || world_pos.cmpgt(hi).any() {
        return None;
    }
    for i in 0..3 {
        if i != edge && edges[i].0.dot(world_pos) + edges[i].1 > 0.0 {
            return None;
        }
    }
    let normal = if length > SMALL { to_contact / -length } else { n };
    Some(SphereHit {
        normal,
        face_normal: n,
        rel_pos: normal * -radius,
        world_pos,
        depth: length - radius,
        surface: SurfaceType::Road,
    })
}

/// Fricción y rebote del chasis contra la pista, por la superficie del triángulo, como
/// `DetectConvexHullPolyColls`: en las paredes resbala más y rebota un poco más.
struct SurfaceHooks<'a> {
    track: &'a [TrackMesh],
    cars: &'a [CarMaterial],
}

impl PhysicsHooks for SurfaceHooks<'_> {
    fn modify_solver_contacts(&self, context: &mut ContactModificationContext) {
        let (Some(c1), Some(c2)) = (context.colliders.get(context.collider1), context.colliders.get(context.collider2))
        else {
            return;
        };
        let (track, car, tri) = if c1.user_data & TAG_MASK == TAG_TRACK && c2.user_data & TAG_MASK == TAG_CAR {
            (c1.user_data, c2.user_data, context.manifold.subshape1)
        } else if c2.user_data & TAG_MASK == TAG_TRACK && c1.user_data & TAG_MASK == TAG_CAR {
            (c2.user_data, c1.user_data, context.manifold.subshape2)
        } else {
            return;
        };
        let (Some(mesh), Some(car)) = (
            self.track.get((track & !TAG_MASK) as usize),
            self.cars.get((car & !TAG_MASK) as usize),
        ) else {
            return;
        };
        let surface = mesh.surfaces.get(tri as usize).copied().unwrap_or(SurfaceType::Road);
        let profile = surfaces::profile(surface);
        let mut friction = car.kinetic_friction * profile.roughness;
        let mut restitution = car.hardness * profile.hardness;
        if context.normal.y.abs() < 0.15 {
            friction *= 0.1;
            restitution += 0.1;
        }
        *context.friction = friction;
        *context.restitution = restitution;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sphere_resting_on_a_face_reports_its_depth() {
        let tri = [Vec3::new(-1.0, 0.0, -1.0), Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, -1.0)];
        let hit = sphere_triangle(Vec3::new(0.0, 0.1, 0.0), Vec3::new(0.0, 0.04, 0.0), 0.05, tri).expect("contacto");
        assert!((hit.depth + 0.01).abs() < 1e-6);
        assert!((hit.normal - Vec3::Y).length() < 1e-6);
    }

    #[test]
    fn sphere_behind_the_face_does_not_collide() {
        let tri = [Vec3::new(-1.0, 0.0, -1.0), Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, -1.0)];
        assert!(sphere_triangle(Vec3::new(0.0, -0.2, 0.0), Vec3::new(0.0, -0.2, 0.0), 0.05, tri).is_none());
    }
}
