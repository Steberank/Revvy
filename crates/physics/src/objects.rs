//! Objetos de la pista en Rapier: pelotas, conos, botellas… Cuerpos dinámicos con la forma,
//! la masa, la fricción y el rebote de su `ObjectKind`. Chocan con la pista, con los autos
//! (cascos y ruedas) y entre ellos; la fricción y el rebote de cada contacto los pone
//! `SurfaceHooks` con las reglas de `body.cpp`.
//!
//! Un objeto con camino (`ObjectMotion`), como las puertas corredizas de market2, es
//! cinemático: lo sigue sin que nada lo frene, empuja autos y objetos y no choca con la
//! pista.

use glam::{Quat, Vec3};
use rapier3d::prelude::*;
use revvy_formats::{MotionCue, ObjectKind, ObjectMotion, ObjectShape};

use crate::world::{GROUP_CAR_HULL, GROUP_OBJECT, GROUP_OBJECT_ONLY, GROUP_WORLD, TAG_OBJECT};

/// `MOV_MoveBodyClever`: un objeto que va a menos de 30 unidades por segundo y gira a menos
/// de 1 rad/s durante `MOVE_MAX_NOMOVETIME` (0.2 s) se duerme hasta que algo lo toque. Rapier
/// mide la velocidad del punto más lejano del cuerpo, así que el giro entra por su largo.
/// Dormirlo pronto también corta el bamboleo que el solver agranda en un casco apoyado en
/// una cara plana (la base de un cono) hasta volcarlo.
const SLEEP_SPEED: f32 = 30.0 * 0.005;
const SLEEP_SPIN: f32 = 1.0;
const SLEEP_TIME: f32 = 0.2;

/// Fricción y rebote propios de un objeto, antes de combinarlos con lo que toca.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ObjectMaterial {
    pub friction: f32,
    pub restitution: f32,
}

/// Qué mueve a un objeto.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Mover {
    /// La física, desde esta velocidad (m/s).
    Physics(Vec3),
    /// Su camino, desde la pose de aparición.
    Path(ObjectMotion),
}

/// El camino de un objeto cinemático.
struct Path {
    origin: Vec3,
    rot: Quat,
    motion: ObjectMotion,
}

/// Un objeto en el mundo.
pub struct Prop {
    body: RigidBodyHandle,
    kind: usize,
    mass: f32,
    pub(crate) material: ObjectMaterial,
    /// Pose al final de los dos últimos pasos, para interpolar el dibujo.
    prev: (Vec3, Quat),
    curr: (Vec3, Quat),
    /// El golpe más fuerte desde la última lectura: cambio de velocidad en m/s, como
    /// `BangMag` de Re-Volt (impulso sobre masa).
    knock: f32,
    /// Arranca dormido. Rapier despierta el cuerpo en el primer paso porque sus colliders
    /// son nuevos, así que se lo vuelve a dormir después, sin moverlo: en Re-Volt un objeto
    /// apilado solo se despierta cuando algo lo choca (`COL_WaitForCollision`).
    put_to_sleep: bool,
    path: Option<Path>,
    /// La última punta del camino por la que pasó, hasta que se lee.
    cue: Option<MotionCue>,
}

impl Prop {
    /// `now` es la hora del mundo: un objeto con camino aparece donde el camino dice.
    pub(crate) fn spawn(
        kind_index: usize,
        kind: &ObjectKind,
        index: usize,
        (pos, rot): (Vec3, Quat),
        mover: Mover,
        now: f64,
        (bodies, colliders): (&mut RigidBodySet, &mut ColliderSet),
    ) -> Option<Self> {
        let shapes: Vec<SharedShape> = match &kind.shape {
            ObjectShape::Sphere { radius } => vec![SharedShape::ball(*radius)],
            ObjectShape::Hulls(hulls) => hulls
                .iter()
                .filter_map(|points| SharedShape::convex_hull(points))
                .collect(),
        };
        if shapes.is_empty() {
            tracing::warn!(objeto = %kind.name, "objeto sin forma válida: no aparece");
            return None;
        }
        let path = match mover {
            Mover::Physics(_) => None,
            Mover::Path(motion) => Some(Path {
                origin: pos,
                rot,
                motion,
            }),
        };
        let start = path.as_ref().map_or(pos, |path| path.at(now));
        let (builder, density) = match mover {
            Mover::Physics(velocity) => {
                let mut builder = RigidBodyBuilder::dynamic()
                    .linvel(velocity)
                    .linear_damping(kind.linear_damping)
                    .angular_damping(kind.angular_damping)
                    .ccd_enabled(true)
                    .sleeping(kind.starts_asleep);
                // Con inercia, la masa va aparte y las formas no pesan. Sin inercia, la
                // reparten las formas según su volumen.
                let volume: f32 = shapes
                    .iter()
                    .map(|shape| shape.mass_properties(1.0).mass())
                    .sum();
                let density = match kind.inertia {
                    Some(inertia) => {
                        builder = builder.additional_mass_properties(MassProperties::new(
                            Vec3::ZERO,
                            kind.mass,
                            Vec3::from(inertia),
                        ));
                        0.0
                    }
                    None if volume > 0.0 => kind.mass / volume,
                    None => 1.0,
                };
                (builder, density)
            }
            Mover::Path(_) => (RigidBodyBuilder::kinematic_position_based(), 0.0),
        };
        let body = bodies.insert(builder.pose(Pose::from_parts(start, rot)));
        // Un objeto con camino no choca con la pista: solo empuja autos y objetos.
        let filter = if path.is_some() {
            GROUP_CAR_HULL | GROUP_OBJECT
        } else {
            let extent = match &kind.shape {
                ObjectShape::Sphere { radius } => *radius,
                ObjectShape::Hulls(hulls) => hulls
                    .iter()
                    .flatten()
                    .map(|p| p.length())
                    .fold(0.0, f32::max),
            };
            let activation = bodies[body].activation_mut();
            activation.normalized_linear_threshold = SLEEP_SPEED + SLEEP_SPIN * extent;
            activation.time_until_sleep = SLEEP_TIME;
            GROUP_WORLD | GROUP_OBJECT_ONLY | GROUP_CAR_HULL | GROUP_OBJECT
        };
        let groups = InteractionGroups::new(GROUP_OBJECT, filter, InteractionTestMode::And);
        for shape in shapes {
            let collider = ColliderBuilder::new(shape)
                .density(density)
                .friction(kind.friction)
                .restitution(kind.restitution)
                .collision_groups(groups)
                .active_hooks(ActiveHooks::MODIFY_SOLVER_CONTACTS)
                .user_data(TAG_OBJECT | index as u128)
                .build();
            colliders.insert_with_parent(collider, body, bodies);
        }
        Some(Self {
            body,
            kind: kind_index,
            mass: kind.mass,
            material: ObjectMaterial {
                friction: kind.friction,
                restitution: kind.restitution,
            },
            prev: (start, rot),
            curr: (start, rot),
            knock: 0.0,
            put_to_sleep: kind.starts_asleep && path.is_none(),
            path,
            cue: None,
        })
    }

    /// Antes del paso: un objeto con camino va a donde el camino dice a la hora `now`, al
    /// final del paso. Rapier saca de ahí la velocidad con la que empuja.
    pub(crate) fn pre_step(&self, bodies: &mut RigidBodySet, now: f64) {
        if let Some(path) = &self.path {
            bodies[self.body].set_next_kinematic_position(Pose::from_parts(path.at(now), path.rot));
        }
    }

    /// Después del paso que terminó a la hora `now`: la pose nueva, el golpe más fuerte de
    /// sus contactos y la punta del camino por la que pasó.
    pub(crate) fn post_step(
        &mut self,
        bodies: &mut RigidBodySet,
        narrow_phase: &NarrowPhase,
        (before, now): (f64, f64),
    ) {
        if std::mem::take(&mut self.put_to_sleep) {
            if let Some(rb) = bodies.get_mut(self.body) {
                rb.sleep();
            }
        }
        let rb = &bodies[self.body];
        self.prev = self.curr;
        self.curr = (rb.translation(), *rb.rotation());
        match &self.path {
            Some(path) => {
                if let Some(cue) = path.motion.cue_between(before, now) {
                    self.cue = Some(cue);
                }
            }
            None => {
                for &handle in rb.colliders() {
                    for pair in narrow_phase.contact_pairs_with(handle) {
                        if pair.has_any_active_contact() {
                            let (impulse, _) = pair.max_impulse();
                            self.knock = self.knock.max(impulse / self.mass);
                        }
                    }
                }
            }
        }
    }

    /// Índice en `TrackObjects::kinds`.
    pub fn kind(&self) -> usize {
        self.kind
    }

    /// Pose interpolada entre los dos últimos pasos.
    pub fn pose(&self, alpha: f32) -> (Vec3, Quat) {
        (
            self.prev.0.lerp(self.curr.0, alpha),
            self.prev.1.slerp(self.curr.1, alpha),
        )
    }

    /// Punto de partida del camino, donde suenan sus puntas. `None`: lo mueve la física.
    pub fn path_origin(&self) -> Option<Vec3> {
        self.path.as_ref().map(|path| path.origin)
    }

    pub fn velocity(&self, bodies: &RigidBodySet) -> Vec3 {
        bodies[self.body].linvel()
    }

    pub fn sleeping(&self, bodies: &RigidBodySet) -> bool {
        bodies[self.body].is_sleeping()
    }

    /// El golpe más fuerte desde la última vez que se leyó.
    pub(crate) fn take_knock(&mut self) -> f32 {
        std::mem::take(&mut self.knock)
    }

    /// La punta del camino por la que pasó desde la última vez que se leyó.
    pub(crate) fn take_cue(&mut self) -> Option<MotionCue> {
        self.cue.take()
    }
}

impl Path {
    fn at(&self, time: f64) -> Vec3 {
        self.origin + self.motion.offset_at(time)
    }
}
