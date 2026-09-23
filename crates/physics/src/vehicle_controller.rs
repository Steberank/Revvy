//! Auto con raycasts de rueda sobre un mundo estático (el `.ncp`, no la mesh visible).

use std::sync::Mutex;

use glam::{Quat, Vec3};
use rapier3d::control::{DynamicRayCastVehicleController, WheelTuning};
use rapier3d::prelude::*;

use crate::collision_events::CollisionEvent;
use crate::jump;

/// Esfera del `.hul`. En Re-Volt el cuerpo choca con el mundo mediante estas esferas.
#[derive(Clone, Copy, Debug)]
pub struct BodySphere {
    pub center: Vec3,
    pub radius: f32,
}

#[derive(Clone, Debug)]
pub struct WheelSetup {
    pub position: Vec3,
    pub powered: bool,
    pub steers: bool,
    /// `SteerRatio` del archivo. Ángulo de la rueda, en radianes, con el volante a tope.
    pub steer_ratio: f32,
    /// `EngineRatio` del archivo, antes de pasarlo a newtons.
    pub engine_ratio: f32,
    pub static_friction: f32,
    pub grip: f32,
    /// `MaxPos` ya en metros.
    pub max_travel: f32,
    pub spring_stiffness: f32,
    pub spring_damping: f32,
    /// `SPRING.Restitution`. En el archivo es negativa: -0.75 se come el choque.
    pub spring_restitution: f32,
}

#[derive(Clone, Debug)]
pub struct CarTuning {
    pub mass: f32,
    /// Gravedad en m/s². En el juego `FLD_Gravity` vale 2200 unidades.
    pub gravity: f32,
    /// Tope en m/s. El archivo está en mph y `units.h` lo pasa a unidades de Re-Volt.
    pub top_speed: f32,
    pub engine_rate: f32,
    pub steer_rate: f32,
    pub steer_mod: f32,
    pub body_friction: f32,
    pub linear_drag: f32,
    pub angular_drag: f32,
    pub wheel_radius: f32,
    pub wheels: Vec<WheelSetup>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DriveInput {
    pub throttle: f32,
    pub steer: f32,
    pub brake: f32,
    pub jump: bool,
    pub flip: bool,
    pub respawn: bool,
}

pub struct DriveWorld {
    pipeline: PhysicsPipeline,
    integration: IntegrationParameters,
    islands: IslandManager,
    broad_phase: BroadPhaseBvh,
    narrow_phase: NarrowPhase,
    bodies: RigidBodySet,
    colliders: ColliderSet,
    impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    ccd: CCDSolver,
    vehicle: DynamicRayCastVehicleController,
    chassis: RigidBodyHandle,
    tuning: CarTuning,
    spawn: Vec3,
    spawn_yaw: f32,
    allow_jump: bool,
    steer_angle: f32,
    engine_volt: f32,
    events: Mutex<Vec<crate::collision_events::CollisionEvent>>,
}

impl DriveWorld {
    pub fn new(
        triangles: &[[Vec3; 3]],
        tuning: CarTuning,
        spawn: Vec3,
        yaw: f32,
        body_spheres: &[BodySphere],
    ) -> Self {
        let mut bodies = RigidBodySet::new();
        let mut colliders = ColliderSet::new();

        let mut vertices = Vec::with_capacity(triangles.len() * 3);
        let mut indices = Vec::with_capacity(triangles.len());
        for tri in triangles {
            let base = vertices.len() as u32;
            for point in tri {
                vertices.push(Vec3::new(point.x, point.y, point.z));
            }
            indices.push([base, base + 1, base + 2]);
        }
        if !vertices.is_empty() {
            match ColliderBuilder::trimesh(vertices, indices) {
                Ok(builder) => {
                    colliders.insert(builder.friction(1.0).build());
                }
                Err(err) => {
                    tracing::error!(?err, "no se pudo armar el trimesh del .ncp");
                }
            }
        }

        let radius = tuning.wheel_radius.max(0.05);
        // En reposo (`Pos` 0) el centro de la rueda está en el offset del archivo.
        let connection_y = tuning
            .wheels
            .iter()
            .map(|wheel| wheel.position.y)
            .fold(0.0f32, f32::max);
        let tire_bottom = connection_y - radius;
        let spawn_y = spawn.y - tire_bottom + 0.02;
        let placed = Vec3::new(spawn.x, spawn_y, spawn.z);
        let chassis = bodies.insert(
            RigidBodyBuilder::dynamic()
                .translation(placed)
                .rotation(Vec3::new(0.0, yaw, 0.0))
                .additional_mass(tuning.mass.max(0.2))
                .linear_damping(tuning.linear_drag)
                .angular_damping(tuning.angular_drag)
                .can_sleep(false)
                .ccd_enabled(true)
                .build(),
        );
        attach_body(&mut colliders, &mut bodies, chassis, &tuning, body_spheres);

        let mut vehicle = DynamicRayCastVehicleController::new(chassis);
        vehicle.index_up_axis = 1;
        vehicle.index_forward_axis = 2;
        let down = Vec3::new(0.0, -1.0, 0.0);
        let axle = Vec3::new(-1.0, 0.0, 0.0);
        let mut wheel_tuning = WheelTuning::default();
        for wheel in &tuning.wheels {
            let travel = suspension_travel(wheel.max_travel);
            configure_wheel(&mut wheel_tuning, wheel, tuning.mass, travel);
            // El cero del muelle es el offset. Puede subir o bajar `MaxPos`.
            let hardpoint = Vec3::new(
                wheel.position.x,
                wheel.position.y + travel,
                wheel.position.z,
            );
            vehicle.add_wheel(hardpoint, down, axle, travel, radius, &wheel_tuning);
        }

        Self {
            pipeline: PhysicsPipeline::new(),
            integration: IntegrationParameters::default(),
            islands: IslandManager::new(),
            broad_phase: BroadPhaseBvh::new(),
            narrow_phase: NarrowPhase::new(),
            bodies,
            colliders,
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            ccd: CCDSolver::new(),
            vehicle,
            chassis,
            tuning,
            spawn: placed,
            spawn_yaw: yaw,
            allow_jump: jump::DEFAULT_ALLOW_JUMP,
            steer_angle: 0.0,
            engine_volt: 0.0,
            events: Mutex::new(Vec::new()),
        }
    }

    pub fn allow_jump(&mut self, allow: bool) {
        self.allow_jump = allow;
    }

    pub fn translation(&self) -> Vec3 {
        self.bodies[self.chassis].translation()
    }

    pub fn rotation(&self) -> Quat {
        *self.bodies[self.chassis].rotation()
    }

    pub fn linear_velocity(&self) -> Vec3 {
        self.bodies[self.chassis].linvel()
    }

    pub fn angular_velocity(&self) -> Vec3 {
        self.bodies[self.chassis].angvel()
    }

    pub fn speed(&self) -> f32 {
        self.linear_velocity().length()
    }

    pub fn grounded_wheels(&self) -> usize {
        self.vehicle
            .wheels()
            .iter()
            .filter(|wheel| wheel.raycast_info().is_in_contact)
            .count()
    }

    pub fn wheel_radius(&self) -> f32 {
        self.tuning.wheel_radius
    }

    pub fn wheel_spins(&self) -> Vec<f32> {
        self.vehicle
            .wheels()
            .iter()
            .map(|wheel| wheel.rotation)
            .collect()
    }

    pub fn wheel_steers(&self) -> Vec<f32> {
        self.vehicle
            .wheels()
            .iter()
            .map(|wheel| wheel.steering)
            .collect()
    }

    /// Acerca la cámara si el `.ncp` tapa la línea hasta el ojo deseado.
    pub fn clip_camera(&self, from: Vec3, to: Vec3) -> Vec3 {
        let delta = to - from;
        let dist = delta.length();
        if dist < 0.05 {
            return to;
        }
        let dir = delta / dist;
        let ray = Ray::new(from, dir);
        let query = self.broad_phase.as_query_pipeline(
            self.narrow_phase.query_dispatcher(),
            &self.bodies,
            &self.colliders,
            QueryFilter::default().exclude_rigid_body(self.chassis),
        );
        if let Some((_, toi)) = query.cast_ray(&ray, dist, true) {
            let allowed = (toi - 0.2).max(0.15);
            if allowed < dist {
                return from + dir * allowed;
            }
        }
        to
    }

    pub fn wheel_centers(&self) -> Vec<Vec3> {
        let rotation = self.rotation();
        let origin = self.translation();
        self.vehicle
            .wheels()
            .iter()
            .zip(&self.tuning.wheels)
            .map(|(wheel, setup)| {
                let local = rotation.inverse() * (wheel.center() - origin);
                // La física puede estirar la suspensión en una bajada. El dibujo se queda en el buje.
                let travel = suspension_travel(setup.max_travel);
                let y = local
                    .y
                    .clamp(setup.position.y - travel, setup.position.y + travel);
                origin + rotation * Vec3::new(setup.position.x, y, setup.position.z)
            })
            .collect()
    }

    pub fn step(&mut self, dt: f32, input: DriveInput) -> Vec<CollisionEvent> {
        // Un frame lento no puede integrar 50 ms de golpe: la rueda atraviesa el `.ncp`.
        const STEP: f32 = 1.0 / 120.0;
        let mut left = dt.clamp(0.0, 0.1);
        let mut events = Vec::new();
        let mut once = true;
        let mut n = 0;
        while left > 0.0 && n < 8 {
            let mut sample = input;
            if !once {
                sample.respawn = false;
                sample.flip = false;
                sample.jump = false;
            }
            once = false;
            events.extend(self.step_once(left.min(STEP), sample));
            left -= STEP;
            n += 1;
        }
        events
    }

    fn step_once(&mut self, dt: f32, input: DriveInput) -> Vec<CollisionEvent> {
        if input.respawn {
            self.respawn();
        }
        if input.flip {
            self.upright();
        }
        self.apply_input(input, dt);
        self.integration.dt = dt;
        let query = self.broad_phase.as_query_pipeline_mut(
            self.narrow_phase.query_dispatcher(),
            &mut self.bodies,
            &mut self.colliders,
            QueryFilter::default().exclude_rigid_body(self.chassis),
        );
        self.vehicle.update_vehicle(dt, query);

        self.events.lock().expect("eventos").clear();
        let handler = EventGrab {
            events: &self.events,
        };
        self.pipeline.step(
            Vec3::new(0.0, -self.tuning.gravity, 0.0),
            &self.integration,
            &mut self.islands,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.ccd,
            &WallFriction,
            &handler,
        );
        self.events.lock().expect("eventos").clone()
    }

    fn apply_input(&mut self, input: DriveInput, dt: f32) {
        let forward = self.rotation() * Vec3::Z;
        let speed_forward = self.linear_velocity().dot(forward);
        let top = self.tuning.top_speed.max(0.5);

        // `Control.cpp`: el volante se acerca al mando a `SteerRate`, más rápido al soltar
        // o al invertir, y `SteerMod` lo frena cuando ya vas hacia adelante.
        let dest = input.steer.clamp(-1.0, 1.0);
        let mut steer_step = self.tuning.steer_rate * dt;
        if dest == 0.0 || dest.signum() != self.steer_angle.signum() {
            steer_step *= 2.0;
        } else {
            let scale = 1.0 - self.tuning.steer_mod * (speed_forward / top);
            steer_step *= scale.max(0.05);
        }
        self.steer_angle = approach(self.steer_angle, dest, steer_step);

        // El voltaje del motor sigue a `EngineRate` y pasa por cero al cambiar de sentido.
        let volt_dest = input.throttle.clamp(-1.0, 1.0);
        if volt_dest.signum() != self.engine_volt.signum()
            && volt_dest != 0.0
            && self.engine_volt != 0.0
        {
            self.engine_volt = 0.0;
        }
        self.engine_volt = approach(self.engine_volt, volt_dest, self.tuning.engine_rate * dt);

        // `car.cpp`: el par cae en proporción a la velocidad de rodadura / TopSpeed.
        let fade = 1.0 - self.engine_volt.signum() * (speed_forward / top);
        let volt = if fade <= 0.0 {
            self.engine_volt * 0.01
        } else {
            self.engine_volt * fade
        };
        let radius_units = (self.tuning.wheel_radius / REVOLT_UNIT_METERS).max(1.0);
        let brake = input.brake.clamp(0.0, 1.0) * 6.0 * self.tuning.mass.max(0.2);
        let grounded = self
            .vehicle
            .wheels()
            .iter()
            .any(|wheel| wheel.raycast_info().is_in_contact);
        for (wheel, setup) in self
            .vehicle
            .wheels_mut()
            .iter_mut()
            .zip(self.tuning.wheels.iter())
        {
            // `CarWheelImpulse2`: fuerza = voltaje * EngineRatio / radio, en la escala de las meshes.
            let force = if setup.powered {
                (setup.engine_ratio / radius_units) * REVOLT_UNIT_METERS * volt
            } else {
                0.0
            };
            wheel.engine_force = force;
            wheel.steering = if setup.steers {
                -self.steer_angle * setup.steer_ratio
            } else {
                0.0
            };
            wheel.brake = if setup.powered { brake * 0.35 } else { brake };
            let travel = suspension_travel(setup.max_travel);
            wheel.friction_slip = tire_friction(setup);
            wheel.side_friction_stiffness = tire_side_stiffness(setup);
            wheel.suspension_stiffness = suspension_rate(setup, self.tuning.mass);
            wheel.damping_relaxation = suspension_relax(setup, self.tuning.mass);
            wheel.damping_compression = suspension_compress(setup, self.tuning.mass);
            wheel.max_suspension_travel = travel;
        }
        if let Some(kick) = jump::impulse(self.allow_jump).filter(|_| input.jump && grounded) {
            let mut vel = self.bodies[self.chassis].linvel();
            vel.y = kick;
            self.bodies[self.chassis].set_linvel(vel, true);
        }
    }

    fn upright(&mut self) {
        let yaw = self.rotation().to_euler(glam::EulerRot::YXZ).0;
        self.bodies[self.chassis].set_rotation(Quat::from_rotation_y(yaw), true);
        self.bodies[self.chassis].set_angvel(Vec3::ZERO, true);
    }

    fn respawn(&mut self) {
        self.bodies[self.chassis].set_translation(self.spawn, true);
        self.bodies[self.chassis].set_rotation(Quat::from_rotation_y(self.spawn_yaw), true);
        self.bodies[self.chassis].set_linvel(Vec3::ZERO, true);
        self.bodies[self.chassis].set_angvel(Vec3::ZERO, true);
        self.steer_angle = 0.0;
        self.engine_volt = 0.0;
    }
}

/// 1 unidad de Re-Volt, la misma escala que usan las meshes.
const REVOLT_UNIT_METERS: f32 = 0.01;

fn suspension_travel(max_pos_meters: f32) -> f32 {
    max_pos_meters.clamp(0.02, 0.12)
}

/// Rapier multiplica la rigidez por la masa del chasis. El archivo es `k` en
/// `SpringDampedForce`: fuerza = -k * extensión.
fn suspension_rate(wheel: &WheelSetup, mass: f32) -> f32 {
    (wheel.spring_stiffness / mass.max(0.2)).clamp(30.0, 800.0)
}

fn suspension_relax(wheel: &WheelSetup, mass: f32) -> f32 {
    (wheel.spring_damping / mass.max(0.2)).clamp(0.5, 20.0)
}

/// `CarWheelImpulse2` escala el golpe con `Spring.Restitution` (negativa).
fn suspension_compress(wheel: &WheelSetup, mass: f32) -> f32 {
    let relax = suspension_relax(wheel, mass);
    let rate = suspension_rate(wheel, mass);
    let shock = (-wheel.spring_restitution).clamp(0.0, 1.0);
    (relax + shock * 2.0 * rate.sqrt()).clamp(relax, 80.0)
}

/// `car.cpp`: μ = StaticFriction de la rueda por Roughness del material.
/// El material por defecto de `NewColl.cpp` tiene Roughness 1.
fn tire_friction(wheel: &WheelSetup) -> f32 {
    wheel.static_friction.clamp(0.5, 6.0)
}

/// El impulso lateral del raycast solo anula un 20 % de la velocidad por paso.
/// Con 5 llega al cono de fricción, que es lo que limita el derrape en el juego.
fn tire_side_stiffness(wheel: &WheelSetup) -> f32 {
    ((wheel.grip / 0.018).clamp(0.5, 2.0)) * 5.0
}

fn configure_wheel(tuning: &mut WheelTuning, wheel: &WheelSetup, mass: f32, travel: f32) {
    tuning.suspension_stiffness = suspension_rate(wheel, mass);
    tuning.suspension_damping = suspension_relax(wheel, mass);
    tuning.suspension_compression = suspension_compress(wheel, mass);
    tuning.friction_slip = tire_friction(wheel);
    tuning.side_friction_stiffness = tire_side_stiffness(wheel);
    tuning.max_suspension_travel = travel;
}

fn approach(current: f32, dest: f32, step: f32) -> f32 {
    let delta = dest - current;
    if delta.abs() <= step {
        dest
    } else {
        current + step.copysign(delta)
    }
}

fn attach_body(
    colliders: &mut ColliderSet,
    bodies: &mut RigidBodySet,
    chassis: RigidBodyHandle,
    tuning: &CarTuning,
    spheres: &[BodySphere],
) {
    let friction = tuning.body_friction.max(0.0);
    if spheres.is_empty() {
        let half = chassis_half_extents(&tuning.wheels);
        colliders.insert_with_parent(
            ColliderBuilder::cuboid(half.x, half.y, half.z)
                .translation(Vec3::new(0.0, half.y, 0.0))
                .density(0.0)
                .friction(friction)
                .restitution(0.0)
                .active_hooks(ActiveHooks::MODIFY_SOLVER_CONTACTS)
                .active_events(ActiveEvents::COLLISION_EVENTS | ActiveEvents::CONTACT_FORCE_EVENTS)
                .build(),
            chassis,
            bodies,
        );
        return;
    }
    // El fondo real de las esferas queda encima del origen, así que al inclinarse
    // el cuerpo ya atravesó el piso cuando recién tocan. Se bajan hasta quedar
    // entre el origen y el contacto de la rueda (~-0.13 m).
    let lowest = spheres
        .iter()
        .map(|sphere| sphere.center.y - sphere.radius)
        .fold(f32::MAX, f32::min);
    let drop = (lowest - (-0.06)).max(0.0);
    for (index, sphere) in spheres.iter().enumerate() {
        let mut builder = ColliderBuilder::ball(sphere.radius.max(0.02))
            .translation(sphere.center - Vec3::Y * drop)
            .density(0.0)
            .friction(friction)
            .restitution(0.0)
            .active_hooks(ActiveHooks::MODIFY_SOLVER_CONTACTS);
        if index == 0 {
            builder = builder
                .active_events(ActiveEvents::COLLISION_EVENTS | ActiveEvents::CONTACT_FORCE_EVENTS);
        }
        colliders.insert_with_parent(builder.build(), chassis, bodies);
    }
}

/// `Body.cpp`: un muro (normal casi horizontal) conserva solo un 10 % de la fricción.
struct WallFriction;

impl PhysicsHooks for WallFriction {
    fn modify_solver_contacts(&self, context: &mut ContactModificationContext) {
        // El chasis siempre choca (`Phsics TDR.doc`, §4.7). En el piso la fricción
        // es baja para que las esferas no claven el auto; el agarre lo dan las ruedas.
        // En un muro se queda el 10 % (`Body.cpp`).
        if context.normal.y.abs() > 0.35 {
            *context.friction = context.friction.min(0.15);
            return;
        }
        *context.friction *= 0.1;
        *context.restitution = (*context.restitution + 0.1).min(0.3);
    }
}

fn chassis_half_extents(wheels: &[WheelSetup]) -> Vec3 {
    if wheels.is_empty() {
        return Vec3::new(0.25, 0.1, 0.4);
    }
    let max_x = wheels
        .iter()
        .map(|w| w.position.x.abs())
        .fold(0.2, f32::max);
    let max_z = wheels
        .iter()
        .map(|w| w.position.z.abs())
        .fold(0.3, f32::max);
    // Más chico que las ruedas y levantado, para no raspar el piso ni pegarse a un muro.
    Vec3::new(max_x * 0.72, 0.08, max_z * 0.7)
}

struct EventGrab<'a> {
    events: &'a Mutex<Vec<crate::collision_events::CollisionEvent>>,
}

impl EventHandler for EventGrab<'_> {
    fn handle_collision_event(
        &self,
        _bodies: &RigidBodySet,
        _colliders: &ColliderSet,
        event: rapier3d::geometry::CollisionEvent,
        _pair: Option<&ContactPair>,
    ) {
        if let rapier3d::geometry::CollisionEvent::Started(..) = event {
            self.events
                .lock()
                .expect("eventos")
                .push(crate::collision_events::CollisionEvent { impulse: 0.0 });
        }
    }

    fn handle_contact_force_event(
        &self,
        _dt: f32,
        _bodies: &RigidBodySet,
        _colliders: &ColliderSet,
        _contact: &ContactPair,
        total_force_magnitude: f32,
    ) {
        if total_force_magnitude > 5.0 {
            self.events
                .lock()
                .expect("eventos")
                .push(crate::collision_events::CollisionEvent {
                    impulse: total_force_magnitude,
                });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floor_and_wall() -> Vec<[Vec3; 3]> {
        let y = 0.0;
        vec![
            [
                Vec3::new(-20.0, y, -20.0),
                Vec3::new(20.0, y, -20.0),
                Vec3::new(20.0, y, 20.0),
            ],
            [
                Vec3::new(-20.0, y, -20.0),
                Vec3::new(20.0, y, 20.0),
                Vec3::new(-20.0, y, 20.0),
            ],
            [
                Vec3::new(-2.0, 0.0, 6.0),
                Vec3::new(2.0, 0.0, 6.0),
                Vec3::new(2.0, 2.0, 6.0),
            ],
            [
                Vec3::new(-2.0, 0.0, 6.0),
                Vec3::new(2.0, 2.0, 6.0),
                Vec3::new(-2.0, 2.0, 6.0),
            ],
        ]
    }

    fn wheel(position: Vec3, powered: bool, steers: bool) -> WheelSetup {
        WheelSetup {
            position,
            powered,
            steers,
            steer_ratio: -0.3,
            engine_ratio: if powered { 20_000.0 } else { 0.0 },
            static_friction: 2.0,
            grip: 0.018,
            max_travel: 0.05,
            spring_stiffness: 600.0,
            spring_damping: 8.0,
            spring_restitution: -0.75,
        }
    }

    fn tuning() -> CarTuning {
        CarTuning {
            mass: 2.0,
            gravity: 9.81,
            top_speed: 12.0,
            engine_rate: 80.0,
            steer_rate: 80.0,
            steer_mod: 0.0,
            body_friction: 0.8,
            linear_drag: 0.05,
            angular_drag: 0.4,
            wheel_radius: 0.11,
            wheels: vec![
                wheel(Vec3::new(-0.25, 0.0, 0.45), false, true),
                wheel(Vec3::new(0.25, 0.0, 0.45), false, true),
                wheel(Vec3::new(-0.24, 0.0, -0.39), true, false),
                wheel(Vec3::new(0.24, 0.0, -0.39), true, false),
            ],
        }
    }

    #[test]
    fn car_stays_on_the_floor_and_stops_at_a_wall() {
        let mut world = DriveWorld::new(
            &floor_and_wall(),
            tuning(),
            Vec3::new(0.0, 0.2, 0.0),
            0.0,
            &[],
        );
        let mut saw_contact = false;
        for _ in 0..240 {
            let events = world.step(
                1.0 / 60.0,
                DriveInput {
                    throttle: 1.0,
                    ..DriveInput::default()
                },
            );
            saw_contact |= !events.is_empty();
        }
        let pos = world.translation();
        assert!(pos.y > -0.2, "se cayó del piso: {pos:?}");
        assert!(pos.y < 2.0, "salió volando: {pos:?}");
        assert!(pos.z < 6.3, "atravesó el muro: {pos:?}");
        assert!(pos.z > 0.4, "no avanzó: {pos:?}");
        assert!(saw_contact, "no hubo eventos de colisión");
    }

    #[test]
    fn a_small_drop_compresses_the_suspension_without_flipping() {
        let mut world = DriveWorld::new(&floor_and_wall(), tuning(), Vec3::ZERO, 0.0, &[]);
        let parked = world.translation();
        world.bodies[world.chassis].set_translation(parked + Vec3::Y * 0.35, true);
        for _ in 0..120 {
            world.step(1.0 / 60.0, DriveInput::default());
        }
        let up = world.rotation() * Vec3::Y;
        assert!(up.y > 0.7, "se dio vuelta al caer: {up:?}");
        let compressed = world.vehicle.wheels().iter().any(|wheel| {
            wheel.raycast_info().is_in_contact
                && wheel.raycast_info().suspension_length < wheel.suspension_rest_length - 0.004
        });
        assert!(compressed, "la suspensión no cedió al caer");
        assert!(
            world.angular_velocity().length() < 2.0,
            "sigue girando después del aterrizaje: {}",
            world.angular_velocity().length()
        );
    }

    #[test]
    fn wheels_stay_on_the_hubs_while_accelerating() {
        let mut world = DriveWorld::new(
            &floor_and_wall(),
            tuning(),
            Vec3::new(0.0, 0.2, 0.0),
            0.0,
            &[],
        );
        for _ in 0..90 {
            world.step(1.0 / 60.0, DriveInput::default());
        }
        let rest: Vec<Vec3> = world
            .wheel_centers()
            .into_iter()
            .map(|center| world.rotation().inverse() * (center - world.translation()))
            .collect();
        for _ in 0..180 {
            world.step(
                1.0 / 60.0,
                DriveInput {
                    throttle: 1.0,
                    ..DriveInput::default()
                },
            );
        }
        let fast: Vec<Vec3> = world
            .wheel_centers()
            .into_iter()
            .map(|center| world.rotation().inverse() * (center - world.translation()))
            .collect();
        for (before, after) in rest.iter().zip(fast) {
            let shift = (after - *before).length();
            assert!(
                shift < 0.15,
                "la rueda se fue del buje: {shift} before={before:?} after={after:?}"
            );
        }
    }

    #[test]
    fn camera_stops_before_a_wall() {
        let mut tris = floor_and_wall();
        tris.push([
            Vec3::new(-4.0, 0.0, -1.5),
            Vec3::new(4.0, 0.0, -1.5),
            Vec3::new(4.0, 3.0, -1.5),
        ]);
        tris.push([
            Vec3::new(-4.0, 0.0, -1.5),
            Vec3::new(4.0, 3.0, -1.5),
            Vec3::new(-4.0, 3.0, -1.5),
        ]);
        let mut world = DriveWorld::new(&tris, tuning(), Vec3::ZERO, 0.0, &[]);
        world.step(1.0 / 60.0, DriveInput::default());
        let from = world.translation() + Vec3::Y * 0.4;
        let to = from - Vec3::Z * 6.0;
        let eye = world.clip_camera(from, to);
        assert!(
            eye.z > -1.45,
            "la cámara atravesó el muro: from={from:?} eye={eye:?}"
        );
        assert!(eye.z < from.z, "la cámara no retrocedió: {eye:?}");
    }

    #[test]
    fn upside_down_body_stays_on_the_floor() {
        let spheres = [BodySphere {
            center: Vec3::new(0.0, 0.12, 0.0),
            radius: 0.1,
        }];
        let mut world = DriveWorld::new(
            &floor_and_wall(),
            tuning(),
            Vec3::new(0.0, 0.5, 0.0),
            0.0,
            &spheres,
        );
        world.bodies[world.chassis].set_rotation(Quat::from_rotation_x(std::f32::consts::PI), true);
        world.bodies[world.chassis].set_translation(Vec3::new(0.0, 0.8, 0.0), true);
        world.bodies[world.chassis].set_linvel(Vec3::ZERO, true);
        world.bodies[world.chassis].set_angvel(Vec3::ZERO, true);
        for _ in 0..180 {
            world.step(1.0 / 60.0, DriveInput::default());
        }
        let y = world.translation().y;
        assert!(y > -0.4, "boca abajo se fue al vacío: {y}");
        assert!(y < 1.0, "boca abajo no llegó al piso: {y}");
    }

    #[test]
    fn front_wheels_take_the_steer_angle() {
        let mut world = DriveWorld::new(&floor_and_wall(), tuning(), Vec3::ZERO, 0.0, &[]);
        world.step(
            1.0 / 60.0,
            DriveInput {
                steer: 1.0,
                ..DriveInput::default()
            },
        );
        let steers = world.wheel_steers();
        assert!(steers[0] > 0.2, "rueda delantera sin giro: {steers:?}");
        assert!(steers[2].abs() < 0.001, "rueda trasera giró: {steers:?}");
    }
}
