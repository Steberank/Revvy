//! Vehículo de Revvy: el chasis es un cuerpo de Rapier y las cuatro ruedas son propias.
//!
//! Rapier integra el chasis y resuelve sus choques: contra el mundo con las esferas de su
//! casco, contra otros autos con los cascos convexos y las ruedas. Las ruedas no son
//! cuerpos: en cada paso buscan sus contactos con la pista (una esfera contra los
//! triángulos cercanos), mueven la suspensión y empujan al chasis con impulsos.
//!
//! El manejo sigue al de Re-Volt en modo Simulación (`car.cpp`, `wheel.cpp`,
//! `control.cpp`, `move.cpp`): volante cúbico, voltaje del motor, torque que se apaga en
//! la velocidad tope, fricción de eje, cono de fricción estática y cinética, suspensión
//! que se come los golpes y downforce. Es código propio sobre `VehicleParams` en SI.
//!
//! Marco del auto: +X izquierda, +Y arriba, +Z adelante. El eje "abajo" del auto
//! (`down`) es el que Re-Volt llama `U`, y la suspensión crece hacia ese lado.

use glam::{Mat3, Mat4, Quat, Vec3};
use rapier3d::prelude::*;
use revvy_formats::{SurfaceType, VehicleParams, WHEEL_COUNT};

use crate::world::{
    track_sphere_hits, SphereHit, TrackMesh, Viewer, GROUP_CAR_HULL, GROUP_CAR_SKIN, GROUP_OBJECT,
    GROUP_OBJECT_ONLY, GROUP_WORLD, TAG_CAR,
};

/// `FRICTION_TIME_SCALE`: las fricciones de Re-Volt están pensadas por 1/120 s.
const FRICTION_TIME_SCALE: f32 = 120.0;
/// Impulso de giro de una rueda sin piso por voltio (`15000` unidades/s² de Re-Volt).
const SPIN_ACCEL: f32 = 75.0;
/// `SKID_RAISE`: la marca de derrape va un poco sobre el piso.
const SKID_RAISE: f32 = 0.01;
/// Dos contactos de la misma rueda a menos de esto en altura son el mismo.
const MERGE_HEIGHT: f32 = 0.015;
/// `MIN_SPARK_VEL`: deslizamiento a partir del cual el costado raspa.
const MIN_SPARK_VEL: f32 = 0.5;
/// `SMALL_IMPULSE_COMPONENT`.
const SMALL_IMPULSE: f32 = 2.5e-5;
/// `MAX_SCRAPE_TIME`: el material de roce se olvida después de esto.
const MAX_SCRAPE_TIME: f32 = 0.05;
/// `MOV_RightCar`: cuánto sube el auto al enderezarse.
const RIGHTING_LIFT: f32 = 0.25;

const WHEEL_PRESENT: u32 = 1;
const WHEEL_STEERED: u32 = 2;
const WHEEL_POWERED: u32 = 4;
const WHEEL_SPIN: u32 = 16;
const WHEEL_SLIDE: u32 = 32;
const WHEEL_LOCKED: u32 = 64;
const WHEEL_CONTACT_FLOOR: u32 = 256;
const WHEEL_CONTACT_WALL: u32 = 512;
const WHEEL_CONTACT_SIDE: u32 = 1024;
const WHEEL_KEEP: u32 = WHEEL_PRESENT | WHEEL_POWERED | WHEEL_STEERED | WHEEL_LOCKED;

/// Mandos de un auto: `steer` −1 (izquierda) a 1 (derecha), `throttle` −1 (freno y
/// reversa) a 1 (adelante). `reset` endereza el auto si está dado vuelta.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Controls {
    pub steer: f32,
    pub throttle: f32,
    pub reset: bool,
}

/// Lo que el motor de sonido necesita del auto en el frame.
#[derive(Clone, Debug, Default)]
pub struct VehicleSound {
    pub pos: Vec3,
    pub vel: Vec3,
    /// Velocidad de rodadura media de las ruedas con tracción (m/s).
    pub wheel_speed: f32,
    /// Superficie donde raspa el chasis o el costado de una rueda.
    pub scrape: Option<SurfaceType>,
    /// Superficie donde derrapan más ruedas.
    pub skid: Option<SurfaceType>,
    pub steer: f32,
    pub last_steer: f32,
    /// Golpe más fuerte del frame, como cambio de velocidad (m/s).
    pub bang: f32,
}

#[derive(Clone, Debug)]
struct Wheel {
    status: u32,
    radius: f32,
    inv_mass: f32,
    inv_inertia: f32,
    spin_impulse: f32,
    gravity: f32,
    max_travel: f32,
    steer_ratio: f32,
    engine_ratio: f32,
    axle_friction: f32,
    spin_damping: f32,
    grip: f32,
    static_friction: f32,
    kinetic_friction: f32,
    stiffness: f32,
    damping: f32,
    restitution: f32,
    /// Anclaje y centro de la esfera en el marco del auto.
    offset: Vec3,
    centre_offset: Vec3,
    /// Recorrido de la suspensión hacia abajo del auto (m) y su velocidad.
    pos: f32,
    vel: f32,
    ang_pos: f32,
    ang_vel: f32,
    ang_impulse: f32,
    turn_angle: f32,
    /// Buje y centro de la esfera en el mundo.
    hub: Vec3,
    centre: Vec3,
    old_centre: Vec3,
    /// Orientación de la rueda sin rodar (con el volante) y con la rodadura, en el mundo.
    axes: Quat,
    rotation: Quat,
    skid_surface: Option<SurfaceType>,
}

impl Wheel {
    fn is(&self, flag: u32) -> bool {
        self.status & flag != 0
    }

    fn in_contact(&self) -> bool {
        self.is(WHEEL_CONTACT_FLOOR | WHEEL_CONTACT_WALL | WHEEL_CONTACT_SIDE)
    }

    /// `SpringDampedForce`: el resorte no tira hacia afuera del recorrido.
    fn spring_force(&self, extension: f32, velocity: f32) -> f32 {
        let force = -self.stiffness * extension - self.damping * velocity;
        if sign(force) == sign(extension) {
            0.0
        } else {
            force
        }
    }
}

#[derive(Clone, Debug)]
struct WheelContact {
    active: bool,
    wheel: usize,
    /// Punto de contacto relativo al centro de masa.
    pos: Vec3,
    world_pos: Vec3,
    vel: Vec3,
    normal: Vec3,
    face_normal: Vec3,
    depth: f32,
    grip: f32,
    static_friction: f32,
    kinetic_friction: f32,
    surface: SurfaceType,
    /// Borde del mundo: un golpe acá no cuenta.
    boundary: bool,
    vel_dot_norm: f32,
    down_dot_norm: f32,
}

/// Pose del chasis y de las ruedas en un paso, para interpolar el dibujo.
#[derive(Clone, Copy, Debug)]
struct Snapshot {
    pos: Vec3,
    rot: Quat,
    wheels: [(Vec3, Quat); WHEEL_COUNT],
}

struct Righting {
    dest_pos: Vec3,
    dest_rot: Quat,
}

/// Coeficientes del chasis para los contactos que resuelve Rapier.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CarMaterial {
    pub kinetic_friction: f32,
    pub hardness: f32,
}

pub struct Vehicle {
    body: RigidBodyHandle,
    skin: Vec<ColliderHandle>,
    wheel_balls: [Option<ColliderHandle>; WHEEL_COUNT],
    mass: f32,
    inv_inertia: Mat3,
    body_gravity: f32,
    hardness: f32,
    kinetic_friction: f32,
    angular_resistance: f32,
    angular_resistance_air: f32,
    body_offset: Vec3,
    steer_rate: f32,
    engine_rate: f32,
    top_speed: f32,
    down_force: f32,
    wheels: [Wheel; WHEEL_COUNT],
    contacts: Vec<WheelContact>,
    steer_angle: f32,
    last_steer_angle: f32,
    engine_volt: f32,
    revs: f32,
    righting: Option<Righting>,
    /// Sin conductor: se endereza solo cuando la Y de su eje vertical baja de esto.
    pub(crate) self_righting: Option<f32>,
    reset_pressed: bool,
    last_reset: bool,
    no_contact_time: f32,
    body_contact: bool,
    start_vel: Vec3,
    shift: Vec3,
    scrape: Option<SurfaceType>,
    scrape_time: f32,
    bang: f32,
    frame_last_steer: f32,
    prev: Snapshot,
    curr: Snapshot,
}

impl Vehicle {
    /// Crea el chasis y sus colliders en `pos` mirando con `yaw` (radianes sobre +Y).
    pub(crate) fn spawn(
        params: &VehicleParams,
        index: usize,
        pos: Vec3,
        yaw: f32,
        bodies: &mut RigidBodySet,
        colliders: &mut ColliderSet,
    ) -> Self {
        let rot = Quat::from_rotation_y(yaw);
        let inertia = Mat3::from_cols_array_2d(&params.inertia).transpose();
        let body = bodies.insert(
            RigidBodyBuilder::dynamic()
                .pose(Pose::from_parts(pos, rot))
                .additional_mass_properties(MassProperties::with_inertia_matrix(Vec3::ZERO, params.mass, inertia))
                .linear_damping(FRICTION_TIME_SCALE * params.resistance)
                .angular_damping(FRICTION_TIME_SCALE * params.angular_resistance)
                .ccd_enabled(true)
                .can_sleep(false),
        );

        let tag = TAG_CAR | index as u128;
        let world = GROUP_WORLD | GROUP_OBJECT_ONLY;
        let mut skin = Vec::new();
        for &[x, y, z, r] in &params.chassis.spheres {
            let collider = ColliderBuilder::ball(r)
                .position(Pose::from_translation(Vec3::new(x, y, z)))
                .density(0.0)
                .friction(params.kinetic_friction)
                .restitution(params.hardness)
                .collision_groups(InteractionGroups::new(GROUP_CAR_SKIN, world, InteractionTestMode::And))
                .active_hooks(ActiveHooks::MODIFY_SOLVER_CONTACTS)
                .user_data(tag)
                .build();
            skin.push(colliders.insert_with_parent(collider, body, bodies));
        }
        // Los cascos chocan con otros autos. Sin esferas, también tocan el mundo.
        let (member, filter) = if params.chassis.spheres.is_empty() {
            (
                GROUP_CAR_HULL | GROUP_CAR_SKIN,
                GROUP_CAR_HULL | GROUP_OBJECT | world,
            )
        } else {
            (GROUP_CAR_HULL, GROUP_CAR_HULL | GROUP_OBJECT)
        };
        for hull in &params.chassis.hulls {
            let points: Vec<Vec3> = hull.iter().map(|&p| Vec3::from(p)).collect();
            let Some(builder) = ColliderBuilder::convex_hull(&points) else {
                tracing::warn!("casco convexo degenerado: se ignora");
                continue;
            };
            let collider = builder
                .density(0.0)
                .friction(params.kinetic_friction)
                .restitution(params.hardness)
                .collision_groups(InteractionGroups::new(member, filter, InteractionTestMode::And))
                .active_hooks(ActiveHooks::MODIFY_SOLVER_CONTACTS)
                .user_data(tag)
                .build();
            let handle = colliders.insert_with_parent(collider, body, bodies);
            if params.chassis.spheres.is_empty() {
                skin.push(handle);
            }
        }

        let wheels: [Wheel; WHEEL_COUNT] = std::array::from_fn(|i| {
            let w = &params.wheels[i];
            let mut status = 0;
            if w.present {
                status |= WHEEL_PRESENT;
            }
            if w.steered {
                status |= WHEEL_STEERED;
            }
            if w.powered {
                status |= WHEEL_POWERED;
            }
            let inertia = w.mass * w.radius * w.radius / 2.0;
            Wheel {
                status,
                radius: w.radius,
                inv_mass: 1.0 / w.mass,
                inv_inertia: 1.0 / inertia,
                spin_impulse: w.mass * SPIN_ACCEL * w.radius,
                gravity: w.gravity,
                max_travel: w.max_travel,
                steer_ratio: w.steer_ratio,
                engine_ratio: w.engine_ratio,
                axle_friction: w.axle_friction,
                spin_damping: w.spin_damping,
                grip: w.grip,
                static_friction: w.static_friction,
                kinetic_friction: w.kinetic_friction,
                stiffness: w.spring.stiffness,
                damping: w.spring.damping,
                restitution: w.spring.restitution,
                offset: Vec3::from(w.offset),
                centre_offset: Vec3::from(w.offset) + Vec3::from(w.centre_offset),
                pos: 0.0,
                vel: 0.0,
                ang_pos: 0.0,
                ang_vel: 0.0,
                ang_impulse: 0.0,
                turn_angle: 0.0,
                hub: pos + rot * Vec3::from(w.offset),
                centre: pos + rot * (Vec3::from(w.offset) + Vec3::from(w.centre_offset)),
                old_centre: pos + rot * (Vec3::from(w.offset) + Vec3::from(w.centre_offset)),
                axes: rot,
                rotation: rot,
                skid_surface: None,
            }
        });

        // Rueda contra carrocería y rueda contra rueda, como en el modo Simulación.
        let wheel_balls = std::array::from_fn(|i| {
            let w = &wheels[i];
            w.is(WHEEL_PRESENT).then(|| {
                let collider = ColliderBuilder::ball(w.radius)
                    .position(Pose::from_translation(w.centre_offset))
                    .density(0.0)
                    .friction(params.kinetic_friction)
                    .collision_groups(InteractionGroups::new(
                        GROUP_CAR_HULL,
                        GROUP_CAR_HULL | GROUP_OBJECT,
                        InteractionTestMode::And,
                    ))
                    .user_data(tag)
                    .build();
                colliders.insert_with_parent(collider, body, bodies)
            })
        });

        let snapshot = Snapshot {
            pos,
            rot,
            wheels: std::array::from_fn(|i| (wheels[i].hub, wheels[i].rotation)),
        };
        Self {
            body,
            skin,
            wheel_balls,
            mass: params.mass,
            inv_inertia: inertia.inverse(),
            body_gravity: params.gravity,
            hardness: params.hardness,
            kinetic_friction: params.kinetic_friction,
            angular_resistance: params.angular_resistance,
            angular_resistance_air: params.angular_resistance_air,
            body_offset: Vec3::from(params.body_offset),
            steer_rate: params.steer_rate,
            engine_rate: params.engine_rate,
            top_speed: params.top_speed,
            down_force: params.down_force,
            wheels,
            contacts: Vec::new(),
            steer_angle: 0.0,
            last_steer_angle: 0.0,
            engine_volt: 0.0,
            revs: 0.0,
            righting: None,
            self_righting: None,
            reset_pressed: false,
            last_reset: false,
            no_contact_time: 0.0,
            body_contact: false,
            start_vel: Vec3::ZERO,
            shift: Vec3::ZERO,
            scrape: None,
            scrape_time: 0.0,
            bang: 0.0,
            frame_last_steer: 0.0,
            prev: snapshot,
            curr: snapshot,
        }
    }

    pub(crate) fn material(&self) -> CarMaterial {
        CarMaterial {
            kinetic_friction: self.kinetic_friction,
            hardness: self.hardness,
        }
    }

    /// Principio de frame: el golpe y el volante del sonido arrancan de nuevo, y `R` se
    /// lee en el flanco.
    pub(crate) fn begin_frame(&mut self, controls: &Controls) {
        self.bang = 0.0;
        self.frame_last_steer = self.steer_angle;
        if controls.reset && !self.last_reset {
            self.reset_pressed = true;
        }
        self.last_reset = controls.reset;
    }

    /// `CON_LocalCarControl`: volante con respuesta cúbica y voltaje del motor.
    fn control(&mut self, controls: &Controls, dt: f32) {
        self.last_steer_angle = self.steer_angle;
        let steer = controls.steer.clamp(-1.0, 1.0);
        let dest = steer * steer * steer;
        let toward_centre = dest == 0.0 || sign(dest) != sign(self.steer_angle);
        // ×4 al volver al centro o cambiar de lado, ×0.5 al seguir doblando.
        let step = self.steer_rate * dt * if toward_centre { 4.0 } else { 0.5 };
        self.steer_angle = approach(self.steer_angle, dest, step);

        let dest = controls.throttle.clamp(-1.0, 1.0);
        if (dest < 0.0 && self.engine_volt > 0.0) || (dest > 0.0 && self.engine_volt < 0.0) {
            self.engine_volt = 0.0;
        }
        self.engine_volt = approach(self.engine_volt, dest, self.engine_rate * dt);
    }

    /// Antes del paso de Rapier: mandos, contactos de las ruedas e impulsos al chasis.
    pub(crate) fn pre_step(
        &mut self,
        controls: &Controls,
        control_dt: Option<f32>,
        bodies: &mut RigidBodySet,
        colliders: &ColliderSet,
        track: &[TrackMesh],
        dt: f32,
    ) {
        if let Some(control_dt) = control_dt {
            self.control(controls, control_dt);
        }
        let rb = &bodies[self.body];
        let (pos, rot) = (rb.translation(), *rb.rotation());
        let (linvel, angvel) = (rb.linvel(), rb.angvel());
        self.start_vel = linvel;
        self.shift = Vec3::ZERO;
        let down = rot * Vec3::NEG_Y;

        if self.reset_pressed {
            self.reset_pressed = false;
            let touching = !self.contacts.is_empty() || self.body_contact || self.no_contact_time < 0.1;
            if self.righting.is_none() && -down.y <= 0.3 && touching {
                self.start_righting(bodies, pos, rot);
            }
        }
        // `TrolleyAIHandler`: el chango no se deja volcar, toque algo o no.
        if self.righting.is_none() && self.self_righting.is_some_and(|min_up| -down.y < min_up) {
            self.start_righting(bodies, pos, rot);
        }
        if self.righting.is_some() {
            self.contacts.clear();
            return;
        }

        // `DetectCarWorldColls`.
        for w in &mut self.wheels {
            w.status &= WHEEL_KEEP;
        }
        self.contacts.clear();
        let mut hits = Vec::new();
        for i in 0..WHEEL_COUNT {
            let w = &self.wheels[i];
            if !w.is(WHEEL_PRESENT) {
                continue;
            }
            hits.clear();
            track_sphere_hits(colliders, track, w.old_centre, w.centre, w.radius, Viewer::Objects, &mut hits);
            for hit in &hits {
                self.add_wheel_contact(i, hit, pos, linvel, angvel, down);
            }
        }

        // `COL_CarCollHandler`.
        let mut impulse = Vec3::ZERO;
        let mut torque = Vec3::ZERO;
        if !self.contacts.is_empty() {
            self.pre_process_contacts(down);
            let (imp, ang) = self.process_contacts(dt, pos, rot, down);
            impulse += imp;
            torque += ang;
            self.post_process_contacts();
        }
        impulse += self.down_force_impulse(dt, rot, linvel);

        let rb = &mut bodies[self.body];
        let in_contact = self.wheels.iter().any(Wheel::in_contact);
        let ang_res = if in_contact {
            self.angular_resistance
        } else {
            self.angular_resistance_air * self.angular_resistance
        };
        rb.set_angular_damping(FRICTION_TIME_SCALE * ang_res);
        rb.apply_impulse(impulse, true);
        rb.apply_torque_impulse(torque, true);
        if self.shift != Vec3::ZERO {
            rb.set_translation(pos + self.shift, true);
        }
    }

    /// `DetectCarWheelColls2` + `AdjustWheelColl`.
    fn add_wheel_contact(&mut self, i: usize, hit: &SphereHit, pos: Vec3, linvel: Vec3, angvel: Vec3, down: Vec3) {
        let w = &self.wheels[i];
        let rel = hit.rel_pos + w.centre - pos;
        let mut vel = angvel.cross(rel) + linvel;
        if (vel + down * w.vel).dot(hit.normal) > 0.0 {
            return;
        }
        let profile = hit.profile;
        let world_pos = hit.world_pos + hit.normal * SKID_RAISE;
        let mut depth = hit.depth;
        vel -= profile.conveyor;
        if let Some(corrugation) = profile.corrugation {
            depth += corrugation.depth(world_pos.x, world_pos.z);
        }
        self.contacts.push(WheelContact {
            active: true,
            wheel: i,
            pos: rel,
            world_pos,
            vel,
            normal: hit.normal,
            face_normal: hit.face_normal,
            depth,
            grip: w.grip * profile.gripiness,
            static_friction: w.static_friction * profile.roughness,
            kinetic_friction: w.kinetic_friction * profile.roughness,
            surface: hit.surface,
            boundary: profile.boundary,
            vel_dot_norm: 0.0,
            down_dot_norm: 0.0,
        });
    }

    /// `PreProcessCarWheelColls`: une contactos repetidos de una rueda y saca el auto del
    /// piso con lo que la suspensión no absorbe.
    fn pre_process_contacts(&mut self, down: Vec3) {
        let mut wheel_shift = [Vec3::ZERO; WHEEL_COUNT];
        let order: Vec<usize> = (0..self.contacts.len()).rev().collect();
        for p1 in 0..order.len() {
            let i1 = order[p1];
            if !self.contacts[i1].active {
                continue;
            }
            let w = self.contacts[i1].wheel;
            let mut keep_going = true;
            for &i2 in &order[p1 + 1..] {
                if !keep_going {
                    break;
                }
                if !self.contacts[i2].active || self.contacts[i2].wheel != w {
                    continue;
                }
                let (c1, c2) = (&self.contacts[i1], &self.contacts[i2]);
                let same = (c1.pos.y - c2.pos.y).abs() < MERGE_HEIGHT
                    || (c1.world_pos.y - c2.world_pos.y).abs() < MERGE_HEIGHT;
                if same {
                    if c1.depth > c2.depth {
                        self.contacts[i1].active = false;
                        self.contacts[i2].normal = self.contacts[i2].face_normal;
                        keep_going = false;
                    } else {
                        self.contacts[i2].active = false;
                        self.contacts[i1].normal = self.contacts[i1].face_normal;
                    }
                }
            }
            if keep_going && self.contacts[i1].depth < 0.0 {
                let c = &self.contacts[i1];
                wheel_shift[c.wheel] += c.normal * -c.depth;
            }
        }

        for (i, shift) in wheel_shift.iter_mut().enumerate() {
            let w = &mut self.wheels[i];
            let mut along = shift.dot(down);
            w.pos += along;
            if w.pos > w.max_travel {
                along -= w.pos - w.max_travel;
                w.pos = w.max_travel;
            } else if w.pos < -w.max_travel {
                along -= w.pos + w.max_travel;
                w.pos = -w.max_travel;
            }
            *shift -= down * along;
            modify_shift(&mut self.shift, *shift);
        }
    }

    /// `ProcessCarWheelColls`: impulso de cada contacto al chasis.
    fn process_contacts(&mut self, dt: f32, pos: Vec3, rot: Quat, down: Vec3) -> (Vec3, Vec3) {
        let mut impulse = Vec3::ZERO;
        let mut torque = Vec3::ZERO;
        let world_inv_inertia = Mat3::from_quat(rot) * self.inv_inertia * Mat3::from_quat(rot).transpose();
        for ci in (0..self.contacts.len()).rev() {
            if !self.contacts[ci].active {
                continue;
            }
            {
                let c = &mut self.contacts[ci];
                c.vel_dot_norm = c.vel.dot(c.normal);
                c.down_dot_norm = c.normal.dot(down);
            }
            let c = &self.contacts[ci];
            if c.depth < 0.0 {
                let w = &mut self.wheels[c.wheel];
                // La rueda sigue al piso. `-down.y` es el `U.y` de Re-Volt (Y abajo).
                w.vel = -(c.down_dot_norm * c.vel_dot_norm);
                w.vel += w.gravity * dt * -down.y;
            }
            let imp = self.wheel_impulse(ci, dt, pos, down, &world_inv_inertia);
            impulse += imp;
            torque += self.contacts[ci].pos.cross(imp);
        }
        (impulse, torque)
    }

    /// `CarWheelImpulse2` (PC): suspensión, deslizamiento lateral, motor y frenos, con el
    /// cono de fricción.
    fn wheel_impulse(&mut self, ci: usize, dt: f32, pos: Vec3, down: Vec3, world_inv_inertia: &Mat3) -> Vec3 {
        let time_scale = FRICTION_TIME_SCALE * dt;
        let c = self.contacts[ci].clone();
        let iw = c.wheel;
        let n = c.normal;
        // El eje de la rueda (su derecha); las ruedas derechas copian el de la izquierda.
        let right = self.wheels[iw].axes * Vec3::NEG_X;

        let mut look = n.cross(right);
        let look_len = look.length();
        let mut sparks = false;
        let fric_mod;
        {
            let w = &mut self.wheels[iw];
            if look_len > 0.7 {
                look /= look_len;
                fric_mod = 1.0;
                w.status |= if n.y.abs() > 0.5 { WHEEL_CONTACT_FLOOR } else { WHEEL_CONTACT_WALL };
            } else {
                sparks = true;
                if n.y.abs() > 0.5 {
                    fric_mod = 1.0;
                    w.status |= WHEEL_CONTACT_FLOOR | WHEEL_CONTACT_SIDE;
                } else {
                    fric_mod = 0.1 + look_len / 4.0;
                    w.status |= WHEEL_CONTACT_WALL | WHEEL_CONTACT_SIDE;
                }
            }
        }

        // Normal: el choque que la suspensión devuelve según `restitution`.
        let dvel_norm = -c.vel_dot_norm;
        let mut imp_dot_norm = if c.down_dot_norm < 0.9 {
            let t1 = c.pos.cross(n);
            let t2 = *world_inv_inertia * t1;
            let imp = dvel_norm / (1.0 / self.mass + t2.cross(c.pos).dot(n));
            imp + self.wheels[iw].restitution * imp * c.down_dot_norm * c.down_dot_norm
        } else {
            0.0
        };

        // Resorte.
        let w = &self.wheels[iw];
        let to_centre = w.centre - pos - c.pos;
        let spring_imp = if sign(w.pos) == sign(to_centre.dot(down)) {
            dt * w.spring_force(w.pos, w.vel) * c.down_dot_norm
        } else {
            0.0
        };
        imp_dot_norm -= spring_imp;
        if imp_dot_norm < 0.0 {
            return Vec3::ZERO;
        }
        let imp_norm = n * imp_dot_norm;
        if !c.boundary {
            self.bang = self.bang.max(imp_dot_norm / self.mass);
        }

        // Deslizamiento de costado, sin la componente en la dirección de rodadura.
        let mut vel_tan = c.vel - n * c.vel_dot_norm;
        vel_tan -= look * look.dot(vel_tan);
        let slide_vel = vel_tan.length();
        let mut imp_tan = vel_tan * (c.grip * spring_imp.abs() * time_scale * -0.35);

        let volt = self.engine_volt;
        let top_speed = self.top_speed;
        let w = &mut self.wheels[iw];
        let mut torque = if w.is(WHEEL_POWERED) {
            let torque = dt * volt * w.engine_ratio;
            let fade = torque.abs() * (w.ang_vel * w.radius / top_speed);
            let t = torque - fade;
            if sign(torque) != sign(t) {
                0.0
            } else {
                t
            }
        } else {
            0.0
        };
        if w.is(WHEEL_LOCKED) || volt.abs() < 0.01 || sign(volt) == -sign(w.ang_vel) {
            let mut t = w.axle_friction * (w.ang_vel * w.radius) * time_scale;
            if w.is(WHEEL_LOCKED) {
                t *= 3.0;
            }
            torque -= t;
        }
        let max_torque = (w.radius * fric_mod) * (c.static_friction * imp_dot_norm);
        if torque.abs() > max_torque.abs() {
            w.status |= WHEEL_SPIN;
        }
        imp_tan += look * (torque / w.radius);

        let mut imp_tan_len = imp_tan.length();
        if imp_tan_len == 0.0 {
            imp_tan_len = 1.0;
        }
        let max = 4.0 * dt * self.mass * self.body_gravity;
        if imp_tan_len > max {
            imp_tan *= max / imp_tan_len;
            imp_tan_len = max;
            w.status |= WHEEL_SLIDE;
        }
        if imp_tan_len > c.static_friction * fric_mod * imp_dot_norm {
            imp_tan *= (c.kinetic_friction * fric_mod * imp_dot_norm) / imp_tan_len;
            w.status |= WHEEL_SLIDE;
        }
        if !w.is(WHEEL_SPIN) {
            let rolling = c.vel.dot(look) / w.radius;
            w.ang_vel += (rolling - w.ang_vel) * (time_scale / 4.0);
        }
        if sparks && slide_vel > MIN_SPARK_VEL {
            self.scrape = Some(c.surface);
            self.scrape_time = 0.0;
        }

        let mut out = imp_norm + imp_tan;
        for k in 0..3 {
            if out[k].abs() < SMALL_IMPULSE {
                out[k] = 0.0;
            }
        }
        out
    }

    /// `PostProcessCarWheelColls`: el material de cada rueda, para el sonido.
    fn post_process_contacts(&mut self) {
        for c in self.contacts.iter().filter(|c| c.active) {
            self.wheels[c.wheel].skid_surface = Some(c.surface);
        }
    }

    /// `CarDownForce`: con las dos ruedas de un mismo lado en el aire, empuja al piso.
    fn down_force_impulse(&self, dt: f32, rot: Quat, linvel: Vec3) -> Vec3 {
        let mut contact = 0u32;
        for (i, w) in self.wheels.iter().enumerate() {
            if w.is(WHEEL_PRESENT) && w.in_contact() {
                contact |= 1 << i;
            }
        }
        // Solo con FL + BL o FR + BR tocando.
        if contact != 0b0101 && contact != 0b1010 {
            return Vec3::ZERO;
        }
        let up = rot * Vec3::Y;
        let modifier = (1.5 - up.y.abs()).min(1.0);
        let vel = dt * modifier * linvel.dot(rot * Vec3::Z);
        (rot * Vec3::NEG_Y) * (self.down_force * vel)
    }

    /// Después del paso de Rapier: enderezar, ruedas, colliders de rueda y contactos del
    /// chasis para el sonido.
    pub(crate) fn post_step(
        &mut self,
        bodies: &mut RigidBodySet,
        colliders: &mut ColliderSet,
        narrow_phase: &NarrowPhase,
        track: &[TrackMesh],
        dt: f32,
    ) {
        self.prev = self.curr;
        self.body_contacts(bodies, colliders, narrow_phase, track);
        if self.righting.is_some() {
            self.right_car(bodies, dt);
        }

        let rb = &bodies[self.body];
        let (pos, rot) = (rb.translation(), *rb.rotation());
        let (linvel, angvel) = (rb.linvel(), rb.angvel());
        let body_acc = linvel - self.start_vel;
        let steer = self.steer_angle;
        let volt = self.engine_volt;
        self.revs = 0.0;
        let mut powered = 0;
        for i in 0..WHEEL_COUNT {
            if self.wheels[i].is(WHEEL_PRESENT) {
                self.wheels[i].centre += self.shift;
                let (prev_axes, prev_rotation) = if i & 1 == 1 {
                    (self.wheels[i - 1].axes, self.wheels[i - 1].rotation)
                } else {
                    (rot, rot)
                };
                update_wheel(&mut self.wheels[i], i, dt, pos, rot, angvel, body_acc, steer, volt, prev_axes, prev_rotation);
                if let Some(handle) = self.wheel_balls[i] {
                    if let Some(collider) = colliders.get_mut(handle) {
                        let w = &self.wheels[i];
                        collider.set_position_wrt_parent(Pose::from_translation(
                            w.centre_offset + Vec3::NEG_Y * w.pos,
                        ));
                    }
                }
            }
            if self.wheels[i].is(WHEEL_POWERED) {
                self.revs += self.wheels[i].ang_vel * self.wheels[i].radius;
                powered += 1;
            }
        }
        if powered > 0 {
            self.revs /= powered as f32;
        }

        let touching = self.body_contact || self.wheels.iter().any(Wheel::in_contact);
        self.no_contact_time = if touching { 0.0 } else { self.no_contact_time + dt };
        if self.scrape_time > MAX_SCRAPE_TIME {
            self.scrape = None;
        } else {
            self.scrape_time += dt;
        }
        let _ = linvel;
        self.curr = Snapshot {
            pos,
            rot,
            wheels: std::array::from_fn(|i| (self.wheels[i].hub, self.wheels[i].rotation)),
        };
    }

    /// Contactos del chasis que resolvió Rapier: si toca algo (para enderezar), y los
    /// golpes y el roce para el sonido. Un choque contra otro auto también suena.
    fn body_contacts(&mut self, bodies: &RigidBodySet, colliders: &ColliderSet, narrow_phase: &NarrowPhase, track: &[TrackMesh]) {
        self.body_contact = false;
        let rb = &bodies[self.body];
        for &handle in rb.colliders() {
            for pair in narrow_phase.contact_pairs_with(handle) {
                if !pair.has_any_active_contact() {
                    continue;
                }
                self.body_contact = true;
                let other = if pair.collider1 == handle { pair.collider2 } else { pair.collider1 };
                let manifold = pair.manifolds.iter().find(|m| !m.data.solver_contacts.is_empty());
                let hit = colliders.get(other).and_then(|collider| {
                    let mesh = track.get(crate::world::track_mesh_of(collider)?)?;
                    let manifold = manifold?;
                    let tri = if pair.collider1 == other { manifold.subshape1 } else { manifold.subshape2 };
                    let surface = *mesh.surfaces.get(tri as usize)?;
                    Some((surface, mesh.profile(surface).boundary))
                });
                let surface = hit.map(|(surface, _)| surface);
                if !hit.is_some_and(|(_, boundary)| boundary) {
                    let (impulse, _) = pair.max_impulse();
                    self.bang = self.bang.max(impulse / self.mass);
                }
                if let (Some(surface), Some(manifold)) = (surface, manifold) {
                    let centre = colliders.get(handle).map_or(rb.translation(), |c| c.position().translation);
                    let vel = rb.velocity_at_point(centre);
                    let n = manifold.data.normal;
                    if (vel - n * vel.dot(n)).length() > MIN_SPARK_VEL {
                        self.scrape = Some(surface);
                        self.scrape_time = 0.0;
                    }
                }
            }
        }
    }

    /// Primer paso de `MOV_RightCar`: el destino queda 25 cm arriba, derecho y mirando
    /// para donde miraba.
    fn start_righting(&mut self, bodies: &mut RigidBodySet, pos: Vec3, rot: Quat) {
        let mut forward = rot * Vec3::Z;
        forward.y = 0.0;
        let forward = forward.try_normalize().unwrap_or(Vec3::NEG_X);
        let left = Vec3::Y.cross(forward);
        let dest_rot = Quat::from_mat3(&Mat3::from_cols(left, Vec3::Y, forward)).normalize();
        self.righting = Some(Righting {
            dest_pos: pos + Vec3::Y * RIGHTING_LIFT,
            dest_rot,
        });
        let rb = &mut bodies[self.body];
        rb.set_linvel(Vec3::ZERO, true);
        rb.set_angvel(Vec3::ZERO, true);
        rb.set_body_type(RigidBodyType::KinematicPositionBased, true);
    }

    /// `MOV_RightCar`: el auto va hacia el destino y termina cuando quedó derecho.
    fn right_car(&mut self, bodies: &mut RigidBodySet, dt: f32) {
        let Some(righting) = &self.righting else {
            return;
        };
        let rb = &mut bodies[self.body];
        let pos = rb.translation();
        let rot = *rb.rotation();
        let new_pos = pos + (righting.dest_pos - pos) * (dt * 10.0).min(1.0);
        let new_rot = rot.slerp(righting.dest_rot, (dt * 8.0).min(1.0)).normalize();
        let done = new_rot.dot(righting.dest_rot).abs() > 0.9999;
        rb.set_position(Pose::from_parts(new_pos, new_rot), true);
        if done {
            rb.set_body_type(RigidBodyType::Dynamic, true);
            rb.set_linvel(Vec3::ZERO, true);
            rb.set_angvel(Vec3::ZERO, true);
            self.righting = None;
        }
    }

    /// Pone el auto en una pose, quieto y con la suspensión en reposo.
    pub(crate) fn place(&mut self, bodies: &mut RigidBodySet, pos: Vec3, rot: Quat) {
        let rb = &mut bodies[self.body];
        rb.set_body_type(RigidBodyType::Dynamic, true);
        rb.set_position(Pose::from_parts(pos, rot), true);
        rb.set_linvel(Vec3::ZERO, true);
        rb.set_angvel(Vec3::ZERO, true);
        self.righting = None;
        self.contacts.clear();
        self.steer_angle = 0.0;
        self.engine_volt = 0.0;
        for w in &mut self.wheels {
            w.pos = 0.0;
            w.vel = 0.0;
            w.ang_vel = 0.0;
            w.status &= WHEEL_KEEP;
            w.hub = pos + rot * w.offset;
            w.centre = pos + rot * w.centre_offset;
            w.old_centre = w.centre;
            w.axes = rot;
            w.rotation = rot;
        }
        self.curr = Snapshot {
            pos,
            rot,
            wheels: std::array::from_fn(|i| (self.wheels[i].hub, self.wheels[i].rotation)),
        };
        self.prev = self.curr;
    }

    /// Chasis y ruedas para dibujar, interpolados entre los dos últimos pasos.
    pub fn models(&self, alpha: f32) -> [Mat4; 1 + WHEEL_COUNT] {
        let pos = self.prev.pos.lerp(self.curr.pos, alpha);
        let rot = self.prev.rot.slerp(self.curr.rot, alpha);
        let mut out = [Mat4::from_rotation_translation(rot, pos) * Mat4::from_translation(self.body_offset); 1 + WHEEL_COUNT];
        for i in 0..WHEEL_COUNT {
            let (p0, r0) = self.prev.wheels[i];
            let (p1, r1) = self.curr.wheels[i];
            out[i + 1] = Mat4::from_rotation_translation(r0.slerp(r1, alpha), p0.lerp(p1, alpha));
        }
        out
    }

    pub fn pose(&self, alpha: f32) -> (Vec3, Quat) {
        (self.prev.pos.lerp(self.curr.pos, alpha), self.prev.rot.slerp(self.curr.rot, alpha))
    }

    pub fn velocity(&self, bodies: &RigidBodySet) -> Vec3 {
        bodies[self.body].linvel()
    }

    /// Estado para el sonido.
    pub fn sound(&self, bodies: &RigidBodySet) -> VehicleSound {
        let mut count = [0u32; 27];
        for w in &self.wheels {
            if w.is(WHEEL_PRESENT) && w.is(WHEEL_SPIN | WHEEL_SLIDE) && w.in_contact() {
                if let Some(surface) = w.skid_surface {
                    count[surface.index()] += 1;
                }
            }
        }
        let skid = (0..27)
            .filter(|&i| count[i] > 0)
            .max_by_key(|&i| (count[i], std::cmp::Reverse(i)))
            .map(|i| SurfaceType::ALL[i]);
        let rb = &bodies[self.body];
        VehicleSound {
            pos: rb.translation(),
            vel: rb.linvel(),
            wheel_speed: self.revs,
            scrape: self.scrape,
            skid,
            steer: self.steer_angle,
            last_steer: self.frame_last_steer,
            bang: self.bang,
        }
    }

    /// El chasis o alguna rueda tocó algo hace poco: la condición de `MOV_RightCar`.
    pub fn touching(&self) -> bool {
        !self.contacts.is_empty() || self.body_contact || self.no_contact_time < 0.1
    }

    /// Cuántas ruedas tocan algo. Para los tests.
    pub fn wheels_in_contact(&self) -> usize {
        self.wheels.iter().filter(|w| w.in_contact()).count()
    }

    /// Recorrido de la suspensión de cada rueda (m, positivo hacia abajo).
    pub fn suspension(&self) -> [f32; WHEEL_COUNT] {
        std::array::from_fn(|i| self.wheels[i].pos)
    }

    /// Velocidad angular de cada rueda (rad/s, positiva hacia adelante).
    pub fn wheel_spin(&self) -> [f32; WHEEL_COUNT] {
        std::array::from_fn(|i| self.wheels[i].ang_vel)
    }
}

/// `UpdateCarWheel` (PC): motor en el aire, suspensión, rodadura y volante.
#[allow(clippy::too_many_arguments)]
fn update_wheel(
    w: &mut Wheel,
    i: usize,
    dt: f32,
    body_pos: Vec3,
    rot: Quat,
    body_angvel: Vec3,
    body_acc: Vec3,
    steer: f32,
    volt: f32,
    prev_axes: Quat,
    prev_rotation: Quat,
) {
    let down = rot * Vec3::NEG_Y;
    if (w.is(WHEEL_POWERED) && !w.in_contact()) || (w.is(WHEEL_SPIN) && w.in_contact()) {
        let mut scale = volt * w.spin_impulse * dt;
        if w.engine_ratio < 0.0 {
            scale = -scale;
        }
        w.ang_impulse += scale;
    }
    let mut acc = 0.0;
    let ang_acc = w.inv_inertia * w.ang_impulse;
    // Centrípeta del giro del auto y aceleración del chasis sobre el eje de la suspensión.
    let dr = w.hub - body_pos;
    let cent_acc = body_angvel.cross(dr.cross(body_angvel));
    acc += dt * cent_acc.dot(down);
    acc -= body_acc.dot(down);
    w.vel += acc;
    w.ang_vel += ang_acc;
    w.ang_vel *= 1.0 - FRICTION_TIME_SCALE * dt * w.spin_damping;
    w.pos = (w.pos + w.vel * dt).clamp(-w.max_travel, w.max_travel);
    w.ang_pos = (w.ang_pos + w.ang_vel * dt).rem_euclid(std::f32::consts::TAU);

    let force = dt * w.spring_force(w.pos, w.vel);
    if w.in_contact() {
        w.vel += w.inv_mass * force / 2.0;
    } else {
        w.vel += w.inv_mass * force;
    }

    w.old_centre = w.centre;
    let travel = Vec3::NEG_Y * w.pos;
    w.centre = body_pos + rot * (w.centre_offset + travel);
    w.hub = body_pos + rot * (w.offset + travel);

    if i & 1 == 1 {
        // Las ruedas derechas copian la orientación de la izquierda de su eje.
        w.axes = prev_axes;
        w.rotation = prev_rotation;
    } else {
        w.axes = if w.is(WHEEL_STEERED) {
            w.turn_angle = steer * w.steer_ratio;
            rot * Quat::from_rotation_y(w.turn_angle)
        } else {
            rot
        };
        w.rotation = w.axes * Quat::from_rotation_x(w.ang_pos);
    }
    w.ang_impulse = 0.0;
}

/// `ModifyShift`: por componente, el mismo signo se queda con el mayor y distinto signo se suma.
pub(crate) fn modify_shift(shift: &mut Vec3, normal: Vec3) {
    for i in 0..3 {
        let new_shift = normal[i];
        if sign(shift[i]) == sign(new_shift) {
            if shift[i].abs() < new_shift.abs() {
                shift[i] = new_shift;
            }
        } else {
            shift[i] += new_shift;
        }
    }
}

fn sign(value: f32) -> f32 {
    if value > 0.0 {
        1.0
    } else if value < 0.0 {
        -1.0
    } else {
        0.0
    }
}

fn approach(value: f32, dest: f32, step: f32) -> f32 {
    if dest > value {
        (value + step).min(dest)
    } else if dest < value {
        (value - step).max(dest)
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steering_is_cubic_and_recentres_four_times_faster() {
        assert_eq!(approach(0.0, 1.0, 0.3), 0.3);
        assert_eq!(approach(0.9, 1.0, 0.3), 1.0);
        let wheel_spring = |pos: f32, vel: f32| {
            let force = -600.0 * pos - 8.0 * vel;
            if sign(force) == sign(pos) {
                0.0
            } else {
                force
            }
        };
        assert_eq!(wheel_spring(0.01, 0.0), -6.0);
        assert_eq!(wheel_spring(0.01, -2.0), 0.0);
    }
}
