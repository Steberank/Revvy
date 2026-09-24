//! El auto de Re-Volt: `car.cpp`, `wheel.cpp`, `control.cpp`, `move.cpp` y los
//! manejadores de colisión del auto de `newcoll.cpp` (PC retail, modo Simulación).

use glam::Vec3;
use revvy_formats::CarInfo;

use super::body::{Body, Sphere};
use super::coll::{modify_shift, sphere_coll_poly, CollPoly};
use super::level::CollWorld;
use super::material::{corrugation_amp, CORRUGATIONS, MATERIALS, MATERIAL_BOUNDARY, MATERIAL_CORRUGATED, MATERIAL_MOVES};
use super::math::{
    good_wrap, mat_mul_mat, mat_to_quat, rot_matrix_y, rotation_x, rotation_y, sign, slerp_quat, vec_mul_mat,
    BBox, Mat, Plane, Quat, IDENTITY,
};
use super::units::{
    CTRL_RANGE_MAX, FLD_GRAVITY, FRICTION_TIME_SCALE, MAX_COLLS_WHEEL, MAX_TIMESTEP, MIN_SPARK_VEL, MPH2OGU_SPEED,
    OILY_WHEEL_TIME, SKID_RAISE, SMALL_IMPULSE_COMPONENT, SMALL_REAL,
};

pub const WHEEL_PRESENT: u32 = 1;
pub const WHEEL_STEERED: u32 = 2;
pub const WHEEL_POWERED: u32 = 4;
pub const WHEEL_SPIN: u32 = 16;
pub const WHEEL_SLIDE: u32 = 32;
pub const WHEEL_LOCKED: u32 = 64;
pub const WHEEL_OIL: u32 = 128;
pub const WHEEL_CONTACT_FLOOR: u32 = 256;
pub const WHEEL_CONTACT_WALL: u32 = 512;
pub const WHEEL_CONTACT_SIDE: u32 = 1024;
pub const WHEEL_CONTACT_OTHER: u32 = 2048;
pub const WHEEL_SKID: u32 = WHEEL_SPIN | WHEEL_SLIDE;
const WHEEL_KEEP: u32 = WHEEL_PRESENT | WHEEL_POWERED | WHEEL_STEERED | WHEEL_OIL | WHEEL_LOCKED;

pub const CAR_NWHEELS: usize = 4;

/// Slot 0 de `CarGridStarts` por tipo de grilla: (x, y, z, rotoff). Con un solo auto
/// solo importa el primero; los tipos 0 y 1 lo tienen en el origen.
const GRID_SLOT0: [[f32; 4]; 4] = [
    [0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0],
    [-1600.0, -250.0, -1100.0, 0.0],
    [0.0, 0.0, 300.0, 0.0],
];

/// `SPRING`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Spring {
    pub stiffness: f32,
    pub damping: f32,
    pub restitution: f32,
}

impl Spring {
    /// `SpringDampedForce`: el resorte no tira hacia afuera del recorrido.
    pub fn damped_force(&self, extension: f32, velocity: f32) -> f32 {
        let force = -self.stiffness * extension - self.damping * velocity;
        if sign(force) == sign(extension) {
            0.0
        } else {
            force
        }
    }
}

/// `WHEEL`.
#[derive(Clone, Debug)]
pub struct Wheel {
    pub status: u32,
    pub mass: f32,
    pub inv_mass: f32,
    pub inertia: f32,
    pub inv_inertia: f32,
    pub radius: f32,
    pub gravity: f32,
    pub grip: f32,
    pub static_friction: f32,
    pub kinetic_friction: f32,
    pub default_static_friction: f32,
    pub default_kinetic_friction: f32,
    pub steer_ratio: f32,
    pub engine_ratio: f32,
    pub axle_friction: f32,
    pub bbox: BBox,
    /// Recorrido de la suspensión sobre el eje up del auto (positivo = abajo).
    pub pos: f32,
    /// Ángulo de rodadura, 0 – 2π.
    pub ang_pos: f32,
    pub vel: f32,
    pub ang_vel: f32,
    pub acc: f32,
    pub ang_acc: f32,
    pub impulse: f32,
    pub ang_impulse: f32,
    pub max_pos: f32,
    pub spin_ang_imp: f32,
    pub wmatrix: Mat,
    pub wpos: Vec3,
    pub old_wpos: Vec3,
    pub centre_pos: Vec3,
    pub old_centre_pos: Vec3,
    pub axes: Mat,
    pub turn_angle: f32,
    pub skid_width: f32,
    pub oil_time: f32,
    pub skid_material: Option<usize>,
    pub skid_started: bool,
    pub no_skid_time: f32,
}

impl Wheel {
    /// `SetupWheel`.
    fn setup(info: &revvy_formats::WheelInfo) -> Self {
        let mut status = 0;
        if info.is_present {
            status |= WHEEL_PRESENT;
        }
        if info.is_turnable {
            status |= WHEEL_STEERED;
        }
        if info.is_powered {
            status |= WHEEL_POWERED;
        }
        let inertia = info.mass * (info.radius * info.radius) / 2.0;
        Self {
            status,
            mass: info.mass,
            inv_mass: 1.0 / info.mass,
            inertia,
            inv_inertia: 1.0 / inertia,
            radius: info.radius,
            gravity: info.gravity,
            grip: info.grip,
            static_friction: info.static_friction,
            kinetic_friction: info.kinetic_friction,
            default_static_friction: info.static_friction,
            default_kinetic_friction: info.kinetic_friction,
            steer_ratio: info.steer_ratio,
            engine_ratio: info.engine_ratio,
            axle_friction: info.axle_friction,
            bbox: BBox {
                min: Vec3::splat(-info.radius),
                max: Vec3::splat(info.radius),
            },
            pos: 0.0,
            ang_pos: 0.0,
            vel: 0.0,
            ang_vel: 0.0,
            acc: 0.0,
            ang_acc: 0.0,
            impulse: 0.0,
            ang_impulse: 0.0,
            max_pos: info.max_pos,
            spin_ang_imp: info.mass * 15000.0 * info.radius,
            wmatrix: IDENTITY,
            wpos: Vec3::ZERO,
            old_wpos: Vec3::ZERO,
            centre_pos: Vec3::ZERO,
            old_centre_pos: Vec3::ZERO,
            axes: IDENTITY,
            turn_angle: 0.0,
            skid_width: info.skid_width,
            oil_time: OILY_WHEEL_TIME,
            skid_material: None,
            skid_started: false,
            no_skid_time: 0.0,
        }
    }

    #[inline]
    pub fn is(&self, flag: u32) -> bool {
        self.status & flag != 0
    }

    #[inline]
    pub fn is_present(&self) -> bool {
        self.is(WHEEL_PRESENT)
    }

    #[inline]
    pub fn is_powered(&self) -> bool {
        self.is(WHEEL_POWERED)
    }

    /// `IsWheelInContact`: piso, pared o costado.
    #[inline]
    pub fn in_contact(&self) -> bool {
        self.is(WHEEL_CONTACT_FLOOR | WHEEL_CONTACT_WALL | WHEEL_CONTACT_SIDE)
    }

    #[inline]
    pub fn skidding(&self) -> bool {
        self.is(WHEEL_SKID)
    }

    fn update_bbox(&mut self) {
        let r = Vec3::splat(self.radius);
        self.bbox = BBox {
            min: self.old_centre_pos.min(self.centre_pos) - r,
            max: self.old_centre_pos.max(self.centre_pos) + r,
        };
    }
}

/// `COLLINFO_WHEEL` contra el mundo.
#[derive(Clone, Debug)]
pub struct WheelColl {
    pub active: bool,
    pub wheel: usize,
    /// Punto de contacto relativo al centro de masa (orientación de mundo).
    pub pos: Vec3,
    pub world_pos: Vec3,
    pub vel: Vec3,
    pub plane: Plane,
    pub depth: f32,
    pub time: f32,
    pub grip: f32,
    pub static_friction: f32,
    pub kinetic_friction: f32,
    pub restitution: f32,
    pub material: usize,
    pub coll_poly: u32,
    pub vel_dot_norm: f32,
    pub up_dot_norm: f32,
    pub slide_vel: f32,
}

/// Mandos del auto en el rango de `CTRL`: `dx` ±127 dobla (negativo = izquierda) y
/// `dy` ±127 acelera (negativo = adelante), como `CRD_KeyboardInput`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Controls {
    pub dx: f32,
    pub dy: f32,
    /// `CTRL_RESET`: enderezar el auto si está dado vuelta. Se lee en el flanco.
    pub reset: bool,
}

impl Controls {
    fn idle(&self) -> bool {
        self.dx == 0.0 && self.dy == 0.0 && !self.reset
    }
}

/// Lo que `UpdateCarSfx` / `UpdateCarMisc` leen del auto al principio del frame.
#[derive(Clone, Debug, Default)]
pub struct SfxState {
    pub pos: Vec3,
    pub vel: Vec3,
    /// `car->Revs`: 8 × velocidad de rodadura media de las ruedas con tracción.
    pub revs: f32,
    /// Material de roce del cuerpo o de un costado de rueda.
    pub scrape_material: Option<usize>,
    /// Material con más ruedas derrapando y en contacto.
    pub skid_material: Option<usize>,
    pub steer_angle: f32,
    pub last_steer_angle: f32,
    /// Golpe más fuerte del frame anterior (`BangMag`).
    pub bang_mag: f32,
    pub class: i32,
}

/// `CAR`.
#[derive(Clone, Debug)]
pub struct Car {
    pub body: Body,
    pub wheels: [Wheel; CAR_NWHEELS],
    pub springs: [Spring; CAR_NWHEELS],
    pub body_offset: Vec3,
    pub wheel_offset: [Vec3; CAR_NWHEELS],
    pub wheel_centre: [Vec3; CAR_NWHEELS],
    pub steer_angle: f32,
    pub last_steer_angle: f32,
    pub steer_rate: f32,
    pub steer_modifier: f32,
    pub engine_volt: f32,
    pub last_engine_volt: f32,
    pub engine_rate: f32,
    pub top_speed: f32,
    pub default_top_speed: f32,
    pub down_force_mod: f32,
    pub bbox: BBox,
    pub wheel_colls: Vec<WheelColl>,
    pub n_wheel_floor_contacts: i32,
    pub n_wheels_in_contact: i32,
    pub revs: f32,
    pub power_timer: f32,
    pub reposition_timer: f32,
    pub righting: bool,
    pub righting_collide: bool,
    pub righting_reach_dest: bool,
    pub dest_pos: Vec3,
    pub dest_quat: Quat,
    pub class: i32,
}

impl Car {
    /// `SetAllCarCoMs` + la parte de colisión de `LoadOneCarModelSet` + `SetupCar`.
    pub fn new(info: &CarInfo, hull_spheres: &[[f32; 4]]) -> Self {
        let com = Vec3::from(info.com);

        let mut body = Body::default();
        body.spheres = hull_spheres
            .iter()
            .map(|s| Sphere {
                pos: Vec3::new(s[0], s[1], s[2]) + com,
                radius: s[3],
            })
            .collect();
        body.world_spheres = body.spheres.clone();
        body.old_world_spheres = body.spheres.clone();
        body.centre.set_mass(info.body.mass);
        body.set_inertia(&Mat::from_rows(info.body.inertia));
        body.centre.gravity = info.body.gravity;
        body.centre.hardness = info.body.hardness;
        body.centre.resistance = info.body.resistance;
        body.centre.static_friction = info.body.static_friction;
        body.centre.kinetic_friction = info.body.kinetic_friction;
        body.default_ang_res = info.body.ang_res;
        body.ang_resistance = info.body.ang_res;
        body.ang_res_mod = info.body.res_mod;
        body.centre.grip = info.body.grip;
        body.allow_sparks = true;
        body.jitter_count_max = 2;
        body.jitter_frames_max = 10;

        let wheels = [0, 1, 2, 3].map(|i| Wheel::setup(&info.wheels[i]));
        let wheel_offset = [0, 1, 2, 3].map(|i| Vec3::from(info.wheels[i].offset1) + com);
        let wheel_centre = [0, 1, 2, 3].map(|i| wheel_offset[i] + Vec3::from(info.wheels[i].offset2));
        let springs = [0, 1, 2, 3].map(|i| Spring {
            stiffness: info.springs[i].stiffness,
            damping: info.springs[i].damping,
            restitution: info.springs[i].restitution,
        });
        let top_speed = info.top_speed_mph * MPH2OGU_SPEED;

        Self {
            body,
            wheels,
            springs,
            body_offset: Vec3::from(info.body.offset) + com,
            wheel_offset,
            wheel_centre,
            steer_angle: 0.0,
            last_steer_angle: 0.0,
            steer_rate: info.steer_rate,
            steer_modifier: info.steer_mod,
            engine_volt: 0.0,
            last_engine_volt: 0.0,
            engine_rate: info.engine_rate,
            top_speed,
            default_top_speed: top_speed,
            down_force_mod: info.down_force_mod,
            bbox: BBox::EMPTY,
            wheel_colls: Vec::new(),
            n_wheel_floor_contacts: 0,
            n_wheels_in_contact: 0,
            revs: 0.0,
            power_timer: 0.0,
            reposition_timer: 0.0,
            righting: false,
            righting_collide: false,
            righting_reach_dest: false,
            dest_pos: Vec3::ZERO,
            dest_quat: Quat::IDENTITY,
            class: info.class,
        }
    }

    /// `GetCarStartGrid` para el slot 0.
    pub fn start_grid(start_pos: [f32; 3], start_rot: f32, grid_type: i32) -> (Vec3, Mat) {
        let grid = GRID_SLOT0[grid_type.clamp(0, GRID_SLOT0.len() as i32 - 1) as usize];
        let mat = rot_matrix_y(-start_rot - grid[3]);
        let mat2 = rot_matrix_y(-start_rot);
        let pos = vec_mul_mat(Vec3::new(grid[0], grid[1], grid[2]), &mat2) + Vec3::from(start_pos);
        (pos, mat)
    }

    /// `ResetCarWheelPos`. El `CentrePos` va en coordenadas de mundo: el original lo
    /// deja relativo (se comentó el `+ WPos` y no se agregó `+ Pos`) solo por un paso.
    fn reset_wheel_pos(&mut self, i: usize) {
        let mat = self.body.centre.wmatrix;
        let pos = self.body.centre.pos;
        let w = &mut self.wheels[i];
        w.pos = 0.0;
        w.vel = 0.0;
        w.acc = 0.0;
        w.impulse = 0.0;
        w.ang_pos = 0.0;
        w.ang_vel = 0.0;
        w.ang_acc = 0.0;
        w.ang_impulse = 0.0;
        w.turn_angle = 0.0;
        w.wpos = vec_mul_mat(self.wheel_offset[i], &mat) + pos;
        w.old_wpos = w.wpos;
        w.centre_pos = vec_mul_mat(self.wheel_centre[i], &mat) + pos;
        w.old_centre_pos = w.centre_pos;
        w.skid_started = false;
        w.update_bbox();
        w.axes = mat;
        w.wmatrix = mat;
    }

    /// `SetCarPos`.
    pub fn set_pos(&mut self, pos: Vec3, mat: &Mat) {
        self.body.set_pos(pos, mat);
        self.steer_angle = 0.0;
        self.last_steer_angle = 0.0;
        self.engine_volt = 0.0;
        self.last_engine_volt = 0.0;
        self.wheel_colls.clear();
        self.n_wheel_floor_contacts = 0;
        self.n_wheels_in_contact = 0;
        for i in 0..CAR_NWHEELS {
            self.reset_wheel_pos(i);
        }
        self.body.init_world_skin();
        self.bbox = self.body.bbox;
        for w in &self.wheels {
            self.bbox.add_pos_rad(w.centre_pos, w.radius);
        }
    }

    /// `CON_LocalCarControl` (PC): volante con respuesta cúbica y voltaje del motor.
    pub fn control(&mut self, controls: &Controls, time_step: f32) {
        self.last_steer_angle = self.steer_angle;

        let dx = controls.dx.clamp(-CTRL_RANGE_MAX, CTRL_RANGE_MAX);
        let dest = dx * dx * dx / (CTRL_RANGE_MAX * CTRL_RANGE_MAX * CTRL_RANGE_MAX);
        let mut step = self.steer_rate * time_step;
        let toward_centre = dest == 0.0 || sign(dest) != sign(self.steer_angle);
        if toward_centre {
            step *= 2.0;
        } else {
            step *= 0.5;
        }
        if self.power_timer > 0.0 {
            step /= 3.0;
        }
        if toward_centre {
            step *= 2.0;
        }
        self.steer_angle = approach(self.steer_angle, dest, step);

        let dy = controls.dy.clamp(-CTRL_RANGE_MAX, CTRL_RANGE_MAX);
        let dest = -(dy / CTRL_RANGE_MAX);
        let step = self.engine_rate * time_step;
        if (dest < 0.0 && self.engine_volt > 0.0) || (dest > 0.0 && self.engine_volt < 0.0) {
            self.engine_volt = 0.0;
        }
        self.engine_volt = approach(self.engine_volt, dest, step);

        // Sin catch-up: tope y fricción de fábrica.
        self.top_speed = self.default_top_speed;
        for w in &mut self.wheels {
            w.static_friction = w.default_static_friction;
            w.kinetic_friction = w.default_kinetic_friction;
        }

        if controls.reset
            && self.reposition_timer == 0.0
            && self.body.centre.wmatrix.u.y <= 0.3
            && (!self.wheel_colls.is_empty()
                || self.body.n_body_colls() > 0
                || self.body.no_contact_time < 0.1
                || self.body.stacked)
        {
            // `movehandler = MOV_RightCar`: el destino se elige en el primer paso.
            self.righting_collide = true;
            self.righting_reach_dest = false;
            if !self.righting {
                self.start_righting();
                self.righting = true;
            }
        }
    }

    /// Estado para el sonido, antes de la física del frame (`UpdateCarSfx`).
    pub fn sfx_state(&self) -> SfxState {
        let mut count = [0u32; MATERIALS.len()];
        for w in &self.wheels {
            if w.is_present() && w.skidding() && w.in_contact() {
                if let Some(material) = w.skid_material {
                    count[material] += 1;
                }
            }
        }
        let mut skid_material = None;
        let mut best = 0;
        for (material, &n) in count.iter().enumerate() {
            if n > best {
                best = n;
                skid_material = Some(material);
            }
        }
        SfxState {
            pos: self.body.centre.pos,
            vel: self.body.centre.vel,
            revs: self.revs,
            scrape_material: self.body.scrape_material,
            skid_material,
            steer_angle: self.steer_angle,
            last_steer_angle: self.last_steer_angle,
            bang_mag: self.body.bang_mag,
            class: self.class,
        }
    }

    /// Inicio de `COL_AllObjectColls` para el auto.
    fn clear_colls(&mut self) {
        self.body.n_world_contacts = 0;
        self.body.n_other_contacts = 0;
        self.body.colls.clear();
        self.wheel_colls.clear();
    }

    /// `DetectCarWorldColls`.
    fn detect_world_colls(&mut self, level: &CollWorld, dt: f32) {
        let Some(cell) = level.grid_for(self.body.centre.pos) else {
            return;
        };
        for w in &mut self.wheels {
            if !w.in_contact() {
                w.skid_started = false;
            }
            w.status &= WHEEL_KEEP;
            if w.is(WHEEL_OIL) {
                w.status &= !WHEEL_OIL;
                w.oil_time = 0.0;
            } else {
                w.oil_time = (w.oil_time + dt).min(OILY_WHEEL_TIME);
            }
        }
        let car_bbox = self.bbox;
        for &index in cell {
            let poly = &level.polys[index as usize];
            if !poly.bbox.overlaps(&car_bbox) || poly.camera_only() {
                continue;
            }
            for i in 0..CAR_NWHEELS {
                if self.wheels[i].is_present() && self.wheels[i].bbox.overlaps(&poly.bbox) {
                    self.detect_wheel_coll(i, poly, index);
                }
            }
            if poly.bbox.overlaps(&self.body.bbox) {
                self.body.detect_poly_colls(poly, index);
            }
        }
    }

    /// `DetectCarWheelColls2`.
    fn detect_wheel_coll(&mut self, i: usize, poly: &CollPoly, index: u32) {
        if self.wheel_colls.len() >= MAX_COLLS_WHEEL {
            return;
        }
        let w = &self.wheels[i];
        let Some(hit) = sphere_coll_poly(w.old_centre_pos, w.centre_pos, w.radius, poly) else {
            return;
        };
        let world_pos = hit.world_pos + hit.plane.n * SKID_RAISE;
        let pos = hit.rel_pos + w.centre_pos - self.body.centre.pos;
        let mut vel = self.body.ang_vel.cross(pos) + self.body.centre.vel;
        let moving = vel + self.body.centre.wmatrix.u * w.vel;
        if moving.dot(poly.plane.n) > 0.0 {
            return;
        }
        let material = &MATERIALS[poly.material];
        let mut depth = hit.depth;
        // `AdjustWheelColl`.
        if material.kind & MATERIAL_MOVES != 0 {
            vel -= material.vel;
        }
        if material.kind & MATERIAL_CORRUGATED != 0 {
            depth += corrugation_amp(&CORRUGATIONS[material.corrugation], world_pos.x, world_pos.z);
        }
        self.wheel_colls.push(WheelColl {
            active: true,
            wheel: i,
            pos,
            world_pos,
            vel,
            plane: hit.plane,
            depth,
            time: hit.time,
            grip: w.grip * material.gripiness,
            static_friction: w.static_friction * material.roughness,
            kinetic_friction: w.kinetic_friction * material.roughness,
            restitution: 0.0,
            material: poly.material,
            coll_poly: index,
            vel_dot_norm: 0.0,
            up_dot_norm: 0.0,
            slide_vel: 0.0,
        });
    }

    /// `COL_CarCollHandler` (+ `COL_BodyCollHandler`).
    fn coll_handler(&mut self, level: &CollWorld, dt: f32) {
        self.n_wheel_floor_contacts = 0;
        self.n_wheels_in_contact = 0;
        if !self.wheel_colls.is_empty() {
            self.pre_process_wheel_colls(&level.polys);
            self.process_wheel_colls(dt);
            self.post_process_wheel_colls();
        }
        self.down_force(dt);

        self.body.tick_scrape(dt);
        // Campo de gravedad global (`AddLinearField` con `DownVec` y sin amortiguación).
        self.body.centre.impulse += Vec3::new(0.0, FLD_GRAVITY * dt * self.body.centre.mass, 0.0);
        if self.body.n_body_colls() > 0 {
            self.body.pre_process_colls(&level.polys);
            if self.body.n_body_colls() > 0 {
                self.body.process_colls(dt);
                self.body.post_process_colls();
            }
        }
        self.set_ang_resistance();
    }

    fn wheel_order(&self) -> Vec<usize> {
        (0..self.wheel_colls.len()).rev().filter(|&i| self.wheel_colls[i].active).collect()
    }

    /// `PreProcessCarWheelColls`: une contactos repetidos de una rueda y saca el auto del piso.
    fn pre_process_wheel_colls(&mut self, polys: &[CollPoly]) {
        let mut world_shift = [Vec3::ZERO; CAR_NWHEELS];
        let order: Vec<usize> = (0..self.wheel_colls.len()).rev().collect();
        for p1 in 0..order.len() {
            let i1 = order[p1];
            if !self.wheel_colls[i1].active {
                continue;
            }
            let w = self.wheel_colls[i1].wheel;
            if self.wheels[w].oil_time < OILY_WHEEL_TIME {
                let factor = 0.5 * (self.wheels[w].oil_time + 0.2) / (OILY_WHEEL_TIME + 0.2);
                self.wheel_colls[i1].static_friction *= factor;
                self.wheel_colls[i1].kinetic_friction *= factor;
            }
            let mut keep_going = true;
            for &i2 in &order[p1 + 1..] {
                if !keep_going {
                    break;
                }
                if !self.wheel_colls[i2].active || self.wheel_colls[i2].wheel != w {
                    continue;
                }
                let (c1, c2) = (&self.wheel_colls[i1], &self.wheel_colls[i2]);
                let remove = (c1.pos.y - c2.pos.y).abs() < 3.0 || (c1.world_pos.y - c2.world_pos.y).abs() < 3.0;
                if remove {
                    if c1.depth > c2.depth {
                        self.wheel_colls[i1].active = false;
                        let poly = self.wheel_colls[i2].coll_poly as usize;
                        self.wheel_colls[i2].plane = polys[poly].plane;
                        keep_going = false;
                    } else {
                        self.wheel_colls[i2].active = false;
                        let poly = self.wheel_colls[i1].coll_poly as usize;
                        self.wheel_colls[i1].plane = polys[poly].plane;
                    }
                }
            }
            if keep_going && self.wheel_colls[i1].depth < 0.0 {
                let c1 = &self.wheel_colls[i1];
                world_shift[c1.wheel] += c1.plane.n * -c1.depth;
            }
        }

        if self.body.stacked {
            return;
        }
        let up = self.body.centre.wmatrix.u;
        for (i, shift) in world_shift.iter_mut().enumerate() {
            // Contra otros cuerpos no hay contactos: `bodyShift` es cero y no cambia nada.
            let w = &mut self.wheels[i];
            let mut shift_dot_up = shift.dot(up);
            w.pos += shift_dot_up;
            if w.pos > w.max_pos {
                shift_dot_up -= w.pos - w.max_pos;
                w.pos = w.max_pos;
            } else if w.pos < -w.max_pos {
                shift_dot_up -= w.pos + w.max_pos;
                w.pos = -w.max_pos;
            }
            *shift -= up * shift_dot_up;
            modify_shift(&mut self.body.centre.shift, 1.0, *shift);
        }
    }

    /// `ProcessCarWheelColls`.
    fn process_wheel_colls(&mut self, dt: f32) {
        let mut tot_imp = Vec3::ZERO;
        let mut tot_ang = Vec3::ZERO;
        let up = self.body.centre.wmatrix.u;
        for i in self.wheel_order() {
            let w = self.wheel_colls[i].wheel;
            {
                let c = &mut self.wheel_colls[i];
                c.vel_dot_norm = c.vel.dot(c.plane.n);
                c.up_dot_norm = c.plane.n.dot(up);
            }
            if self.wheel_colls[i].depth < 0.0 {
                let c = &self.wheel_colls[i];
                let wheel = &mut self.wheels[w];
                wheel.vel = -(c.up_dot_norm * c.vel_dot_norm);
                wheel.vel += wheel.gravity * dt * up.y;
            }
            let imp = self.wheel_impulse(i, dt);
            tot_imp += imp;
            // Simulación y Arcade (`PlayMode < MODE_CONSOLE`): el contacto de rueda también hace girar.
            tot_ang += self.wheel_colls[i].pos.cross(imp);
        }
        self.body.ang_impulse += tot_ang;
        self.body.centre.impulse += tot_imp;
    }

    /// `CarWheelImpulse2` (PC).
    fn wheel_impulse(&mut self, ci: usize, dt: f32) -> Vec3 {
        let time_scale = FRICTION_TIME_SCALE * dt;
        let c = self.wheel_colls[ci].clone();
        let iw = c.wheel;
        let spring = self.springs[iw];
        let n = c.plane.n;

        let mut look = n.cross(self.wheels[iw].axes.r);
        let look_len = look.length();
        let hardness = c.restitution;
        let mut do_spark = false;
        let fric_mod;
        {
            let w = &mut self.wheels[iw];
            if look_len > 0.7 {
                look /= look_len;
                fric_mod = 1.0;
                if c.plane.n.y.abs() > 0.5 {
                    w.status |= WHEEL_CONTACT_FLOOR;
                } else {
                    w.status |= WHEEL_CONTACT_WALL;
                }
            } else {
                do_spark = true;
                if c.plane.n.y.abs() > 0.5 {
                    fric_mod = 1.0;
                    w.status |= WHEEL_CONTACT_FLOOR | WHEEL_CONTACT_SIDE;
                } else {
                    fric_mod = 0.1 + look_len / 4.0;
                    w.status |= WHEEL_CONTACT_WALL | WHEEL_CONTACT_SIDE;
                }
            }
        }

        let dvel_norm = c.vel_dot_norm * -(1.0 + hardness);
        let mut imp_dot_norm = if c.up_dot_norm < 0.9 {
            let imp = self.body.zero_friction_impulse(c.pos, n, dvel_norm);
            // Amortiguador.
            let imp_up = imp * c.up_dot_norm * c.up_dot_norm;
            imp + spring.restitution * imp_up
        } else {
            0.0
        };

        // Resorte.
        let up = self.body.centre.wmatrix.u;
        let to_centre = self.wheels[iw].centre_pos - self.body.centre.pos - c.pos;
        let spring_imp = if sign(self.wheels[iw].pos) == sign(to_centre.dot(up)) {
            dt * spring.damped_force(self.wheels[iw].pos, self.wheels[iw].vel) * c.up_dot_norm
        } else {
            0.0
        };
        imp_dot_norm -= spring_imp;

        if imp_dot_norm < 0.0 {
            return Vec3::ZERO;
        }
        let imp_norm = n * imp_dot_norm;

        if c.material != MATERIAL_BOUNDARY {
            let knock = imp_dot_norm.abs() * self.body.centre.inv_mass;
            if knock > self.body.bang_mag {
                self.body.bang_mag = knock;
                self.body.bang_plane = c.plane;
            }
        } else {
            self.body.bang_mag = 0.0;
        }

        // Deslizamiento lateral (sin la componente en la dirección de rodadura).
        let mut vel_tan = c.vel - n * c.vel_dot_norm;
        let imp_dot_look = look.dot(vel_tan);
        vel_tan -= look * imp_dot_look;
        let slide_vel = vel_tan.length();
        self.wheel_colls[ci].slide_vel = slide_vel;

        let mut imp_tan_len = c.grip * spring_imp.abs() * time_scale;
        imp_tan_len *= -0.35;
        let mut imp_tan = vel_tan * imp_tan_len;

        let w = &mut self.wheels[iw];
        let mut torque = if w.is_powered() {
            let torque = dt * (self.engine_volt * w.engine_ratio);
            let t = torque.abs() * ((w.ang_vel * w.radius) / self.top_speed);
            let t = torque - t;
            if sign(torque) != sign(t) {
                0.0
            } else {
                t
            }
        } else {
            0.0
        };

        if w.is(WHEEL_LOCKED)
            || self.engine_volt.abs() < 0.01
            || sign(self.engine_volt) == -sign(w.ang_vel)
        {
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

        imp_tan_len = imp_tan.length();
        if imp_tan_len == 0.0 {
            imp_tan_len = 1.0;
        }
        let max = 4.0 * (dt * self.body.centre.mass) * self.body.centre.gravity;
        if imp_tan_len > max {
            imp_tan *= max / imp_tan_len;
            imp_tan_len = max;
            w.status |= WHEEL_SLIDE;
        }
        let max = c.static_friction * fric_mod * imp_dot_norm;
        if imp_tan_len > max {
            let kinetic = c.kinetic_friction * fric_mod * imp_dot_norm;
            imp_tan *= kinetic / imp_tan_len;
            w.status |= WHEEL_SLIDE;
        }

        if !w.is(WHEEL_SPIN) {
            let ang_vel = c.vel.dot(look) / w.radius;
            w.ang_vel += (ang_vel - w.ang_vel) * ((FRICTION_TIME_SCALE * dt) / 4.0);
        }

        // El costado de la rueda raspa: sin chispas en Revvy, pero sí el material de roce.
        if do_spark && slide_vel > MIN_SPARK_VEL {
            self.body.scrape_material = Some(c.material);
            self.body.last_scrape_time = 0.0;
        }

        let mut impulse = imp_norm + imp_tan;
        for k in 0..3 {
            if impulse[k].abs() < SMALL_IMPULSE_COMPONENT {
                impulse[k] = 0.0;
            }
        }
        impulse
    }

    /// `PostProcessCarWheelColls` sin marcas de derrape: cuenta el piso y guarda el
    /// material de cada rueda para el sonido.
    fn post_process_wheel_colls(&mut self) {
        let car_dot_up = self.body.centre.wmatrix.u.y;
        for i in self.wheel_order() {
            let c = &self.wheel_colls[i];
            if car_dot_up > 0.0 && car_dot_up * c.plane.n.y < -0.15 {
                self.n_wheel_floor_contacts += 1;
            }
            self.wheels[c.wheel].skid_material = Some(c.material);
        }
    }

    /// `CarDownForce`: con dos ruedas de un lado en el aire, empuja contra el piso.
    fn down_force(&mut self, dt: f32) {
        let mut contact = 0u32;
        for (i, w) in self.wheels.iter().enumerate() {
            if w.is_present() && w.in_contact() {
                contact |= 1 << i;
            }
        }
        if contact == 0b1111 {
            return;
        }
        let m = self.body.centre.wmatrix;
        let modifier = (1.0 + 0.5 - m.u.y.abs()).min(1.0);
        let mut down = Vec3::ZERO;
        // FL | BL y FR | BR.
        if contact == 0b0101 || contact == 0b1010 {
            let vel = dt * modifier * self.body.centre.vel.dot(m.l);
            down += m.u * (self.down_force_mod * vel);
        }
        self.body.centre.impulse += down;
    }

    /// `SetCarAngResistance`: en el aire el giro se frena más.
    fn set_ang_resistance(&mut self) {
        let in_contact = self.wheels.iter().any(|w| w.in_contact());
        self.body.ang_resistance = if in_contact {
            self.body.default_ang_res
        } else {
            self.body.ang_res_mod * self.body.default_ang_res
        };
    }

    /// `UpdateCarWheel` (PC).
    fn update_wheel(&mut self, i: usize, dt: f32) {
        let mat = self.body.centre.wmatrix;
        let body_pos = self.body.centre.pos;
        let ang_vel_body = self.body.ang_vel;
        let body_acc = self.body.centre.acc;
        let steer = self.steer_angle;
        let volt = self.engine_volt;
        let spring = self.springs[i];
        let (prev_axes, prev_wmatrix) = if i & 1 == 1 {
            (self.wheels[i - 1].axes, self.wheels[i - 1].wmatrix)
        } else {
            (IDENTITY, IDENTITY)
        };
        let w = &mut self.wheels[i];

        if (w.is_powered() && !w.in_contact()) || (w.is(WHEEL_SPIN) && w.in_contact()) {
            let mut scale = volt * w.spin_ang_imp * dt;
            if w.engine_ratio < 0.0 {
                scale = -scale;
            }
            w.ang_impulse += scale;
        }

        w.acc = w.inv_mass * w.impulse;
        w.ang_acc = w.inv_inertia * w.ang_impulse;

        // Centrípeta del giro del auto y aceleración del cuerpo sobre el eje up.
        let dr = w.wpos - body_pos;
        let tmp = dr.cross(ang_vel_body);
        let cent_acc = ang_vel_body.cross(tmp);
        w.acc += dt * cent_acc.dot(mat.u);
        w.acc -= body_acc.dot(mat.u);

        w.vel += w.acc;
        w.ang_vel += w.ang_acc;
        w.ang_vel *= 1.0 - FRICTION_TIME_SCALE * dt * w.axle_friction;

        w.pos = (w.pos + w.vel * dt).clamp(-w.max_pos, w.max_pos);
        w.ang_pos = good_wrap(w.ang_pos + w.ang_vel * dt, 0.0, std::f32::consts::TAU);

        let force = dt * spring.damped_force(w.pos, w.vel);
        if !w.in_contact() {
            w.vel += w.inv_mass * force;
        } else {
            w.vel += w.inv_mass * force / 2.0;
        }

        w.old_wpos = w.wpos;
        w.old_centre_pos = w.centre_pos;
        let centre = self.wheel_centre[i] + Vec3::new(0.0, w.pos, 0.0);
        w.centre_pos = vec_mul_mat(centre, &mat) + body_pos;
        let offset = self.wheel_offset[i] + Vec3::new(0.0, w.pos, 0.0);
        w.wpos = vec_mul_mat(offset, &mat) + body_pos;

        if i & 1 == 1 {
            // Las ruedas derechas copian la orientación de la izquierda.
            w.axes = prev_axes;
            w.wmatrix = prev_wmatrix;
        } else {
            if w.is(WHEEL_STEERED) {
                w.turn_angle = steer * w.steer_ratio;
                w.axes = mat_mul_mat(&rotation_y(w.turn_angle), &mat);
            } else {
                w.axes = mat;
            }
            w.wmatrix = mat_mul_mat(&rotation_x(w.ang_pos), &w.axes);
        }

        w.update_bbox();
        w.impulse = 0.0;
        w.ang_impulse = 0.0;
    }

    /// `MOV_MoveCarNew`.
    fn move_car(&mut self, dt: f32, last_idle: bool) {
        let v = self.body.centre.vel;
        let a = self.body.ang_vel;
        if v.x.abs() < 20.0
            && v.z.abs() < 20.0
            && v.y.abs() < 20.0
            && a.x.abs() < 0.5
            && a.y.abs() < 0.5
            && a.z.abs() < 0.5
        {
            self.body.no_move_time += dt;
        } else {
            self.body.no_move_time = 0.0;
        }

        self.body.stacked = self.engine_volt == 0.0
            && self.steer_angle == 0.0
            && last_idle
            && self.body.no_move_time > 0.1
            && !self.righting
            && self.n_wheel_floor_contacts == 0
            && self.body.n_other_contacts == 0
            && (self.body.n_body_colls() > 1 || self.body.stacked);

        if self.body.stacked {
            self.body.centre.impulse = Vec3::ZERO;
            self.body.ang_impulse = Vec3::ZERO;
            self.body.centre.vel = Vec3::ZERO;
            self.body.ang_vel = Vec3::ZERO;
            self.body.centre.shift = Vec3::ZERO;
            for w in &mut self.wheels {
                w.vel = 0.0;
            }
        }

        let body_shift = self.body.centre.shift;
        self.body.update(dt);
        self.bbox = self.body.bbox;

        self.revs = 0.0;
        let mut powered = 0;
        for i in 0..CAR_NWHEELS {
            if self.wheels[i].is_present() {
                self.wheels[i].centre_pos += body_shift;
                self.update_wheel(i, dt);
                // Simulación: la caja del auto suma cada rueda.
                self.bbox.add_pos_rad(self.wheels[i].centre_pos, self.wheels[i].radius);
            }
            if self.wheels[i].is_powered() {
                self.revs += 8.0 * (self.wheels[i].ang_vel * self.wheels[i].radius);
                powered += 1;
            }
        }
        if powered > 0 {
            self.revs /= powered as f32;
        }
    }

    /// Primer paso de `MOV_RightCar`: destino 50 unidades arriba y derecho.
    fn start_righting(&mut self) {
        let m = self.body.centre.wmatrix;
        self.dest_pos = self.body.centre.pos - Vec3::new(0.0, 50.0, 0.0);
        let mut l = Vec3::new(m.l.x, 0.0, m.l.z);
        let len = l.length();
        if len > SMALL_REAL {
            l /= len;
        } else {
            l = Vec3::X;
        }
        let u = Vec3::Y;
        let r = u.cross(l);
        self.dest_quat = mat_to_quat(&Mat { r, u, l });
        // `ConstrainQuat2`: mismo hemisferio.
        if self.dest_quat.dot(&self.body.centre.quat) < 0.0 {
            self.body.centre.quat.negate();
        }
    }

    /// `MOV_RightCar`.
    fn right_car(&mut self, dt: f32) {
        self.body.centre.vel = Vec3::ZERO;
        self.body.centre.impulse = Vec3::ZERO;
        self.body.ang_vel = Vec3::ZERO;
        self.body.ang_impulse = Vec3::ZERO;
        if !self.righting_collide {
            self.body.centre.shift = Vec3::ZERO;
        }
        self.body.stacked = false;

        let dr = self.dest_pos - self.body.centre.pos;
        self.body.centre.pos += dr * (dt * 10.0);
        self.body.centre.quat = slerp_quat(&self.body.centre.quat, &self.dest_quat, dt * 8.0);
        self.body.update(dt);
        for i in 0..CAR_NWHEELS {
            if self.wheels[i].is_present() {
                self.update_wheel(i, dt);
            }
        }
        self.revs = 0.0;

        let dr = if self.righting_reach_dest {
            self.body.centre.old_pos - self.body.centre.pos
        } else {
            Vec3::ZERO
        };
        if dr.dot(dr) < SMALL_REAL && self.body.centre.quat.dot(&self.dest_quat) > 0.9999 {
            self.righting = false;
        }
    }
}

fn approach(value: f32, dest: f32, step: f32) -> f32 {
    let mut value = value;
    if dest > value {
        if dest - value < step {
            value = dest;
        } else {
            value += step;
        }
    }
    if dest < value {
        if value - dest < step {
            value = dest;
        } else {
            value -= step;
        }
    }
    value
}

/// El auto y el mundo, con el orden de un frame de `gameloop.cpp`.
#[derive(Clone, Debug)]
pub struct Simulation {
    pub level: CollWorld,
    pub car: Car,
    last_controls: Controls,
}

/// Qué pasó en el frame, para el sonido.
#[derive(Clone, Debug, Default)]
pub struct FrameReport {
    /// Estado leído antes de mover (lo que ve `UpdateCarSfx`).
    pub sfx: SfxState,
    /// Pasos de física de este frame.
    pub steps: usize,
}

impl Simulation {
    pub fn new(level: CollWorld, mut car: Car, start: (Vec3, Mat)) -> Self {
        car.set_pos(start.0, &start.1);
        Self {
            level,
            car,
            last_controls: Controls::default(),
        }
    }

    /// Un frame: mandos con el `TimeStep` entero, estado de sonido, y después
    /// `1 + TimeStep × MAX_TIMESTEP` pasos de colisión y movimiento.
    pub fn frame(&mut self, frame_dt: f32, controls: Controls) -> FrameReport {
        // `UpdateTimeFactor`: el factor de tiempo se corta en 10 (a 72 Hz).
        let time_step = frame_dt.clamp(0.0, 10.0 / 72.0);

        let reset_edge = controls.reset && !self.last_controls.reset;
        let mut input = controls;
        input.reset = reset_edge;
        self.car.control(&input, time_step);

        // `UpdateCarSfx` lee el estado viejo; `UpdateCarMisc` borra el golpe.
        let sfx = self.car.sfx_state();
        self.car.body.banged = false;
        self.car.body.bang_mag = 0.0;

        let steps = 1 + (time_step * MAX_TIMESTEP) as usize;
        let dt = time_step / steps as f32;
        let last_idle = self.last_controls.idle();
        for _ in 0..steps {
            self.car.clear_colls();
            self.car.detect_world_colls(&self.level, dt);
            self.car.coll_handler(&self.level, dt);
            if self.car.righting {
                self.car.right_car(dt);
            } else {
                self.car.move_car(dt, last_idle);
            }
        }
        self.last_controls = controls;
        FrameReport { sfx, steps }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spring_never_pulls_past_rest() {
        let spring = Spring {
            stiffness: 600.0,
            damping: 8.0,
            restitution: -0.75,
        };
        assert_eq!(spring.damped_force(2.0, 0.0), -1200.0);
        assert_eq!(spring.damped_force(2.0, -200.0), 0.0);
    }

    #[test]
    fn approach_stops_at_the_destination() {
        assert_eq!(approach(0.0, 1.0, 0.3), 0.3);
        assert_eq!(approach(0.9, 1.0, 0.3), 1.0);
        assert_eq!(approach(0.0, -1.0, 0.3), -0.3);
    }
}
