//! `particle.cpp` y `body.cpp`: cuerpo rígido, piel de esferas y respuesta a
//! colisiones del cuerpo (solo contra el mundo: el auto es el único objeto).

use glam::Vec3;

use super::coll::{modify_shift, sphere_coll_poly, CollPoly};
use super::math::{
    get_frame_inertia, mat_cross_vec, mat_mul_vec, mat_to_quat, quat_to_mat, vec_cross_mat, vec_mul_mat,
    vec_mul_quat, BBox, Mat, Plane, Quat, IDENTITY,
};
use super::material::{MATERIALS, MATERIAL_BOUNDARY, MATERIAL_MOVES};
use super::units::{
    FRICTION_TIME_SCALE, MAX_COLLS_PER_BODY, MAX_SCRAPE_TIME, MIN_SPARK_VEL, SMALL_REAL,
};

/// `PARTICLE`.
#[derive(Clone, Debug)]
pub struct Particle {
    pub mass: f32,
    pub inv_mass: f32,
    pub old_pos: Vec3,
    pub pos: Vec3,
    pub vel: Vec3,
    pub acc: Vec3,
    pub impulse: Vec3,
    pub quat: Quat,
    pub wmatrix: Mat,
    pub old_wmatrix: Mat,
    pub hardness: f32,
    pub resistance: f32,
    pub grip: f32,
    pub static_friction: f32,
    pub kinetic_friction: f32,
    pub gravity: f32,
    pub boost: f32,
    pub shift: Vec3,
    pub last_vel: Vec3,
}

impl Default for Particle {
    fn default() -> Self {
        Self {
            mass: 0.0,
            inv_mass: 0.0,
            old_pos: Vec3::ZERO,
            pos: Vec3::ZERO,
            vel: Vec3::ZERO,
            acc: Vec3::ZERO,
            impulse: Vec3::ZERO,
            quat: Quat::IDENTITY,
            wmatrix: IDENTITY,
            old_wmatrix: IDENTITY,
            hardness: 0.0,
            resistance: 0.0,
            grip: 0.0,
            static_friction: 0.0,
            kinetic_friction: 0.0,
            gravity: 2000.0,
            boost: 0.0,
            shift: Vec3::ZERO,
            last_vel: Vec3::ZERO,
        }
    }
}

impl Particle {
    /// `SetParticleMass`.
    pub fn set_mass(&mut self, mass: f32) {
        self.mass = mass;
        self.inv_mass = 1.0 / mass;
    }

    /// `UpdateParticle`.
    pub fn update(&mut self, dt: f32) {
        self.pos += self.shift;
        self.shift = Vec3::ZERO;
        self.old_pos = self.pos;
        let old_vel = self.vel;
        self.vel += self.impulse * self.inv_mass;
        let t = self.resistance * FRICTION_TIME_SCALE * dt;
        self.vel *= 1.0 - t;
        self.pos += self.vel * dt;
        self.impulse = Vec3::ZERO;
        self.acc = self.vel - old_vel;
    }
}

/// `SPHERE`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sphere {
    pub pos: Vec3,
    pub radius: f32,
}

/// `COLLINFO_BODY` contra el mundo (`Body2` es siempre `BDY_MassiveBody`).
#[derive(Clone, Debug)]
pub struct BodyColl {
    pub active: bool,
    pub pos1: Vec3,
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
}

/// `NEWBODY`.
#[derive(Clone, Debug)]
pub struct Body {
    pub centre: Particle,
    pub body_inertia: Mat,
    pub body_inv_inertia: Mat,
    pub world_inv_inertia: Mat,
    pub ang_vel: Vec3,
    pub ang_acc: Vec3,
    pub ang_impulse: Vec3,
    pub default_ang_res: f32,
    pub ang_res_mod: f32,
    pub ang_resistance: f32,
    pub spheres: Vec<Sphere>,
    pub world_spheres: Vec<Sphere>,
    pub old_world_spheres: Vec<Sphere>,
    pub tight_bbox: BBox,
    pub bbox: BBox,
    pub last_ang_vel: Vec3,
    pub is_jittering: bool,
    pub jitter_count: i32,
    pub jitter_count_max: i32,
    pub jitter_frames: i32,
    pub jitter_frames_max: i32,
    /// Lista de contactos en orden de alta. La cabeza de la lista del original es el último.
    pub colls: Vec<BodyColl>,
    pub n_world_contacts: i32,
    pub n_other_contacts: i32,
    pub no_contact_time: f32,
    pub no_move_time: f32,
    pub allow_sparks: bool,
    pub scrape_material: Option<usize>,
    pub last_scrape_time: f32,
    pub banged: bool,
    pub bang_mag: f32,
    pub bang_plane: Plane,
    pub stacked: bool,
}

impl Default for Body {
    /// `InitBodyDefault`.
    fn default() -> Self {
        Self {
            centre: Particle::default(),
            body_inertia: IDENTITY,
            body_inv_inertia: IDENTITY,
            world_inv_inertia: IDENTITY,
            ang_vel: Vec3::ZERO,
            ang_acc: Vec3::ZERO,
            ang_impulse: Vec3::ZERO,
            default_ang_res: 0.0,
            ang_res_mod: 1.0,
            ang_resistance: 0.0,
            spheres: Vec::new(),
            world_spheres: Vec::new(),
            old_world_spheres: Vec::new(),
            tight_bbox: BBox {
                min: Vec3::ZERO,
                max: Vec3::ZERO,
            },
            bbox: BBox::EMPTY,
            last_ang_vel: Vec3::ZERO,
            is_jittering: false,
            jitter_count: 0,
            jitter_count_max: 0,
            jitter_frames: 0,
            jitter_frames_max: 0,
            colls: Vec::new(),
            n_world_contacts: 0,
            n_other_contacts: 0,
            no_contact_time: 0.0,
            no_move_time: 0.0,
            allow_sparks: false,
            scrape_material: None,
            last_scrape_time: 0.0,
            banged: false,
            bang_mag: 0.0,
            bang_plane: Plane {
                n: Vec3::new(0.0, -1.0, 0.0),
                d: 0.0,
            },
            stacked: false,
        }
    }
}

impl Body {
    pub fn n_body_colls(&self) -> usize {
        self.colls.iter().filter(|c| c.active).count()
    }

    /// Índices de los contactos activos en el orden de la lista enlazada original.
    fn active_order(&self) -> Vec<usize> {
        (0..self.colls.len()).rev().filter(|&i| self.colls[i].active).collect()
    }

    /// `SetBodyInertia`: el inverso sale de la matriz del archivo.
    pub fn set_inertia(&mut self, inertia: &Mat) {
        self.body_inertia = *inertia;
        self.body_inv_inertia = super::math::invert_mat(inertia);
    }

    /// `InitWorldSkin`.
    pub fn init_world_skin(&mut self) {
        let (pos, mat) = (self.centre.pos, self.centre.wmatrix);
        self.bbox = BBox::EMPTY;
        self.world_spheres = self
            .spheres
            .iter()
            .map(|s| Sphere {
                pos: vec_mul_mat(s.pos, &mat) + pos,
                radius: s.radius,
            })
            .collect();
        self.old_world_spheres = self.world_spheres.clone();
        for s in &self.world_spheres {
            self.bbox.add_pos_rad(s.pos, s.radius);
        }
    }

    /// `BuildWorldSkin`: la caja suma la posición vieja del arreglo y la nueva.
    pub fn build_world_skin(&mut self) {
        let (pos, mat) = (self.centre.pos, self.centre.wmatrix);
        self.bbox = BBox::EMPTY;
        for (world, local) in self.world_spheres.iter_mut().zip(&self.spheres) {
            self.bbox.add_pos_rad(world.pos, world.radius);
            world.pos = vec_mul_mat(local.pos, &mat) + pos;
            self.bbox.add_pos_rad(world.pos, world.radius);
        }
    }

    /// `SetBodyPos`.
    pub fn set_pos(&mut self, pos: Vec3, mat: &Mat) {
        self.centre.pos = pos;
        self.centre.old_pos = pos;
        self.centre.wmatrix = *mat;
        self.centre.old_wmatrix = *mat;
        self.centre.quat = mat_to_quat(mat);
        self.centre.vel = Vec3::ZERO;
        self.centre.impulse = Vec3::ZERO;
        self.centre.last_vel = Vec3::ZERO;
        self.ang_vel = Vec3::ZERO;
        self.ang_impulse = Vec3::ZERO;
        self.centre.shift = Vec3::ZERO;
        self.world_inv_inertia = get_frame_inertia(&self.body_inv_inertia, &self.centre.wmatrix);
        self.init_world_skin();
        self.build_world_skin();
        self.last_ang_vel = Vec3::ZERO;
        self.is_jittering = false;
        self.jitter_count = 0;
        self.jitter_frames = 0;
        self.n_world_contacts = 0;
        self.n_other_contacts = 0;
        self.no_move_time = 0.0;
        self.no_contact_time = 0.0;
        self.centre.boost = 0.0;
        self.last_scrape_time = 0.0;
        self.scrape_material = None;
    }

    /// `UpdateBody`.
    pub fn update(&mut self, dt: f32) {
        self.last_ang_vel = self.ang_vel;
        if self.is_jittering {
            self.ang_impulse *= 0.5;
        }
        self.centre.update(dt);
        self.ang_acc = mat_mul_vec(&self.world_inv_inertia, self.ang_impulse);
        self.ang_vel += self.ang_acc;
        let scale = 1.0 - self.ang_resistance * dt * FRICTION_TIME_SCALE;
        if scale > 0.0 {
            self.ang_vel *= scale;
        } else {
            self.ang_vel = Vec3::ZERO;
        }
        let dq = vec_mul_quat(self.ang_vel, &self.centre.quat);
        let half = 0.5 * dt;
        self.centre.quat.v += dq.v * half;
        self.centre.quat.s += dq.s * half;
        self.centre.quat.normalize();
        self.centre.wmatrix = quat_to_mat(&self.centre.quat);
        self.world_inv_inertia = get_frame_inertia(&self.body_inv_inertia, &self.centre.wmatrix);

        std::mem::swap(&mut self.world_spheres, &mut self.old_world_spheres);
        self.build_world_skin();

        self.jitter_frames += 1;
        if self.ang_vel.dot(self.last_ang_vel) < 0.0 {
            if self.jitter_frames < self.jitter_frames_max {
                self.jitter_count += 1;
            } else {
                self.jitter_count = 0;
                self.is_jittering = false;
            }
            self.jitter_frames = 0;
            if self.jitter_count > self.jitter_count_max {
                self.is_jittering = true;
            }
        } else if self.is_jittering && self.jitter_frames > self.jitter_frames_max {
            self.is_jittering = false;
        }

        if self.n_world_contacts == 0 {
            self.no_contact_time += dt;
        } else {
            self.no_contact_time = 0.0;
        }
        self.ang_impulse = Vec3::ZERO;
    }

    /// `ApplyBodyImpulse`.
    pub fn apply_impulse(&mut self, impulse: Vec3, pos: Vec3) {
        self.centre.impulse += impulse;
        self.ang_impulse += pos.cross(impulse);
    }

    /// `OneBodyZeroFrictionImpulse`.
    pub fn zero_friction_impulse(&self, pos: Vec3, normal: Vec3, delta_vel: f32) -> f32 {
        let t1 = pos.cross(normal);
        let t2 = mat_mul_vec(&self.world_inv_inertia, t1);
        let t1 = t2.cross(pos);
        let imp_mag = self.centre.inv_mass + t1.dot(normal);
        delta_vel / imp_mag
    }

    /// `BuildOneBodyColMat`.
    fn one_body_col_mat(&self, col_pos: Vec3, col_pos2: Vec3) -> Mat {
        let work = vec_cross_mat(col_pos, &self.world_inv_inertia);
        let mut m = mat_cross_vec(&work, col_pos2);
        let inv = self.centre.inv_mass;
        m.r = Vec3::new(inv - m.r.x, -m.r.y, -m.r.z);
        m.u = Vec3::new(-m.u.x, inv - m.u.y, -m.u.z);
        m.l = Vec3::new(-m.l.x, -m.l.y, inv - m.l.z);
        m
    }

    /// `DetectConvexHullPolyColls`: cada esfera del casco contra un polígono del mundo.
    pub fn detect_poly_colls(&mut self, poly: &CollPoly, poly_index: u32) {
        for i in 0..self.world_spheres.len() {
            if self.n_body_colls() >= MAX_COLLS_PER_BODY {
                return;
            }
            let old_pos = self.old_world_spheres[i].pos;
            let new_pos = self.world_spheres[i].pos;
            let radius = self.world_spheres[i].radius;
            let Some(hit) = sphere_coll_poly(old_pos, new_pos, radius, poly) else {
                continue;
            };
            let pos1 = hit.rel_pos + new_pos - self.centre.pos;
            let mut vel = self.ang_vel.cross(pos1) + self.centre.vel;
            if vel.dot(poly.plane.n) >= 0.0 {
                continue;
            }
            let material = &MATERIALS[poly.material];
            let mut static_friction = self.centre.static_friction * material.roughness;
            let mut kinetic_friction = self.centre.kinetic_friction * material.roughness;
            let mut restitution = self.centre.hardness * material.hardness;
            if poly.plane.n.y.abs() < 0.15 {
                static_friction *= 0.1;
                kinetic_friction *= 0.1;
                restitution += 0.1;
            }
            // `AdjustBodyColl`: la cinta se mueve bajo el cuerpo.
            if material.kind & MATERIAL_MOVES != 0 {
                vel -= material.vel;
            }
            self.colls.push(BodyColl {
                active: true,
                pos1,
                world_pos: hit.world_pos,
                vel,
                plane: poly.plane,
                depth: hit.depth,
                time: hit.time,
                grip: self.centre.grip * material.gripiness,
                static_friction,
                kinetic_friction,
                restitution,
                material: poly.material,
                coll_poly: poly_index,
            });
        }
    }

    /// `PreProcessBodyColls`.
    pub fn pre_process_colls(&mut self, polys: &[CollPoly]) {
        let shift = self.centre.shift;
        if shift.x != 0.0 && shift.y != 0.0 && shift.z != 0.0 {
            for i in self.active_order() {
                let c = &mut self.colls[i];
                if shift.dot(c.plane.n) > -c.depth {
                    c.active = false;
                }
            }
        }

        let order: Vec<usize> = (0..self.colls.len()).rev().collect();
        for p1 in 0..order.len() {
            let i1 = order[p1];
            if !self.colls[i1].active {
                continue;
            }
            if self.colls[i1].depth < 0.0 {
                // Contra el mundo `shiftMod` es 1: todo el empuje va al cuerpo.
                let depth = self.colls[i1].depth;
                let n = self.colls[i1].plane.n;
                let inv = self.centre.inv_mass;
                let shift_mod = if inv > SMALL_REAL { inv / inv } else { 0.0 };
                modify_shift(&mut self.centre.shift, -shift_mod * depth, n);
            }
            for &i2 in &order[p1 + 1..] {
                if !self.colls[i2].active {
                    continue;
                }
                let (c1, c2) = (&self.colls[i1], &self.colls[i2]);
                let dr = c1.pos1 - c2.pos1;
                let mut remove = dr.x.abs() < 30.0 && dr.y.abs() < 30.0 && dr.z.abs() < 30.0;
                if !remove {
                    let drw = c1.world_pos - c2.world_pos;
                    remove = drw.y.abs() < 2.0 && drw.x.abs() < 30.0 && drw.z.abs() < 30.0;
                }
                if remove {
                    if c1.depth < c2.depth {
                        self.colls[i2].active = false;
                        let poly = self.colls[i1].coll_poly as usize;
                        self.colls[i1].plane = polys[poly].plane;
                    } else {
                        self.colls[i1].active = false;
                        let poly = self.colls[i2].coll_poly as usize;
                        self.colls[i2].plane = polys[poly].plane;
                        break;
                    }
                }
            }
        }
    }

    /// `ProcessBodyColls3`: un sistema de ecuaciones con todos los contactos.
    pub fn process_colls(&mut self, dt: f32) {
        let order = self.active_order();
        let n = order.len();
        if n == 0 {
            return;
        }
        // `BuildCollisionEquations3`.
        let mut a = vec![0.0f32; n * n];
        let mut residual = vec![0.0f32; n];
        for (row, &i) in order.iter().enumerate() {
            let ci = &self.colls[i];
            for (col, &j) in order.iter().enumerate() {
                let cj = &self.colls[j];
                let m = self.one_body_col_mat(ci.pos1, cj.pos1);
                let nj = cj.plane.n;
                let t0 = m.r.dot(nj);
                let t1 = m.u.dot(nj);
                let t2 = m.l.dot(nj);
                a[row * n + col] = ci.plane.n.x * t0 + ci.plane.n.y * t1 + ci.plane.n.z * t2;
            }
            let mut res = -((1.0 + ci.restitution) * ci.vel.dot(ci.plane.n));
            let t = self.ang_impulse.cross(ci.pos1);
            let dv = mat_mul_vec(&self.world_inv_inertia, t) + self.centre.impulse * self.centre.inv_mass;
            let dvel_norm = ci.plane.n.dot(dv);
            if dvel_norm < res {
                res -= dvel_norm;
            } else {
                res = 0.0;
            }
            residual[row] = res;
        }

        let tol = 0.001;
        let (soln, res) = if n > 1 {
            super::conjgrad::conj_grad(&a, &residual, n, tol, 2 * n)
        } else {
            (vec![residual[0] / a[0]], 0.0)
        };
        if res.abs() > 1000.0 * tol {
            return;
        }

        self.banged = true;
        let mut tot_imp = Vec3::ZERO;
        let mut tot_ang = Vec3::ZERO;
        for (k, &i) in order.iter().enumerate() {
            let mut imp = self.colls[i].plane.n * soln[k];
            self.add_friction(&mut imp, i, dt);
            tot_imp += imp;
            // `PlayMode < MODE_CONSOLE` (Simulación y Arcade): siempre suma el giro.
            tot_ang += self.colls[i].pos1.cross(imp);
            if self.colls[i].material != MATERIAL_BOUNDARY {
                let knock = soln[k].abs() * self.centre.inv_mass;
                if knock > self.bang_mag {
                    self.bang_mag = knock;
                    self.bang_plane = self.colls[i].plane;
                }
            } else {
                self.bang_mag = 0.0;
            }
        }
        self.ang_impulse += tot_ang;
        self.centre.impulse += tot_imp;
    }

    /// `AddBodyFriction`.
    fn add_friction(&mut self, impulse: &mut Vec3, i: usize, dt: f32) {
        let c = self.colls[i].clone();
        let imp_dot_norm = impulse.dot(c.plane.n);
        if imp_dot_norm < 0.0 {
            return;
        }
        let vel_dot_norm = c.vel.dot(c.plane.n);
        let vel_tan = c.vel - c.plane.n * vel_dot_norm;
        let vel_tan_len = vel_tan.length();

        if vel_tan_len > MIN_SPARK_VEL && self.allow_sparks {
            self.scrape_material = Some(c.material);
            self.last_scrape_time = 0.0;
        }

        let mut imp_tan_len = self.centre.grip * imp_dot_norm;
        imp_tan_len *= dt * FRICTION_TIME_SCALE;
        let mut imp_tan = vel_tan * -imp_tan_len;
        imp_tan_len *= vel_tan_len;

        let imp_tan_max = 0.5 * dt * (self.centre.mass * self.centre.gravity);
        if imp_tan_len > imp_tan_max {
            imp_tan *= imp_tan_max / imp_tan_len;
            imp_tan_len = imp_tan_max;
        }
        let imp_tan_max = c.static_friction * imp_dot_norm;
        if imp_tan_len > imp_tan_max {
            imp_tan *= (c.kinetic_friction * imp_dot_norm) / imp_tan_len;
        }
        *impulse += imp_tan;
    }

    /// `PostProcessBodyColls`: todos los contactos son contra el mundo.
    pub fn post_process_colls(&mut self) {
        self.n_world_contacts += self.n_body_colls() as i32;
    }

    /// Principio de `COL_BodyCollHandler`: el material de roce se olvida con el tiempo.
    pub fn tick_scrape(&mut self, dt: f32) {
        if self.last_scrape_time > MAX_SCRAPE_TIME {
            self.scrape_material = None;
            self.last_scrape_time = 0.0;
        } else {
            self.last_scrape_time += dt;
        }
    }
}
