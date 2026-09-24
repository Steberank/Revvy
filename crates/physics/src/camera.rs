//! Cámara de persecución del motor de Revvy. Se comporta como `CAM_FOLLOW_BEHIND`, la
//! cámara del jugador en Re-Volt (`camera.cpp`), con sus distancias pasadas a metros.

use glam::{Quat, Vec3};

use crate::vehicle_controller::modify_shift;
use crate::world::{PhysicsWorld, Viewer};

/// Detrás y arriba del auto, en el marco horizontal que mira como él.
const BEHIND_OFFSET: Vec3 = Vec3::new(0.0, 0.75, -2.3);
/// A dónde mira: un poco arriba del centro del auto.
const LOOK_HEIGHT: f32 = 0.3;
/// `InnerRadius`: la esfera que no atraviesa el mundo.
const INNER_RADIUS: f32 = 0.325;
/// `NEAR_CLIP_DIST`: la esfera va un poco delante del lente.
const NEAR_CLIP: f32 = 0.15;
/// `CAM_MOVE_SPEED`: qué tan rápido se acorta o se estira el palo sin vista del auto.
const MOVE_SPEED: f32 = 2.0;
/// Sin vista del auto el palo se acorta hasta esta fracción.
const MIN_LOS_SCALE: f32 = 0.3;
/// Solo se revisa la vista a partir de esta distancia, y no se mira más cerca que esto.
const LOS_MIN_DIST: f32 = 0.5;
const LOOK_MIN_DIST: f32 = 0.25;

#[derive(Clone, Debug)]
pub struct ChaseCamera {
    pub eye: Vec3,
    pub target: Vec3,
    pub vel: Vec3,
    pos_offset: Vec3,
    world_offset: Vec3,
    los_scale: f32,
    coll_pos: Vec3,
    old_coll_pos: Vec3,
    hit_wall: bool,
}

/// Marco horizontal que mira como el auto: izquierda, arriba y adelante.
fn heading(rot: Quat) -> (Vec3, Vec3, Vec3) {
    let mut forward = rot * Vec3::Z;
    forward.y = 0.0;
    let forward = forward.try_normalize().unwrap_or(Vec3::Z);
    (Vec3::Y.cross(forward), Vec3::Y, forward)
}

fn in_heading(rot: Quat, offset: Vec3) -> Vec3 {
    let (left, up, forward) = heading(rot);
    left * offset.x + up * offset.y + forward * offset.z
}

impl ChaseCamera {
    /// Arranca detrás del auto, mirándolo.
    pub fn new(obj_pos: Vec3, obj_rot: Quat) -> Self {
        let world_offset = in_heading(obj_rot, BEHIND_OFFSET);
        let eye = obj_pos + world_offset;
        Self {
            eye,
            target: obj_pos + Vec3::Y * LOOK_HEIGHT,
            vel: Vec3::ZERO,
            pos_offset: BEHIND_OFFSET,
            world_offset,
            los_scale: 1.0,
            coll_pos: eye,
            old_coll_pos: eye,
            hit_wall: false,
        }
    }

    /// Un frame: el palo sigue al auto, choca con el mundo y después mira al auto.
    pub fn update(&mut self, dt: f32, obj_pos: Vec3, obj_rot: Quat, world: &PhysicsWorld) {
        self.hit_wall = false;
        let old_eye = self.eye;

        // Por frame, no por tiempo: el palo se estira un cuarto de lo que falta.
        let offset = BEHIND_OFFSET * self.los_scale;
        self.pos_offset += (offset - self.pos_offset) * 0.25;
        let home_len = self.pos_offset.length();
        let home_dir = in_heading(obj_rot, self.pos_offset).normalize_or_zero();
        let mut pole_len = self.world_offset.length();
        let pole_dir = self.world_offset.normalize_or_zero();
        let pole = (pole_dir + (home_dir - pole_dir) * (dt / 0.25)).normalize_or_zero();
        pole_len += (home_len - pole_len) * dt / 0.30;
        self.eye = obj_pos + pole * pole_len;
        self.world_colls(obj_pos, world);
        self.world_offset = self.eye - obj_pos;
        self.old_coll_pos = self.coll_pos;
        self.vel = if dt > 1e-6 { (self.eye - old_eye) / dt } else { Vec3::ZERO };

        let dist = (obj_pos - self.eye).length();
        if dist > LOS_MIN_DIST && !world.line_of_sight(self.coll_pos, obj_pos) {
            self.los_scale = (self.los_scale - MOVE_SPEED * dt).max(MIN_LOS_SCALE);
        } else if !self.hit_wall {
            self.los_scale = (self.los_scale + MOVE_SPEED * dt).min(1.0);
        }
        if dist >= LOOK_MIN_DIST {
            self.target = obj_pos + Vec3::Y * LOOK_HEIGHT;
        }
    }

    /// `CameraWorldColls`: una esfera delante del lente no atraviesa el mundo.
    fn world_colls(&mut self, obj_pos: Vec3, world: &PhysicsWorld) {
        let to_obj = obj_pos - self.eye;
        let dist = to_obj.length();
        let dir = to_obj.normalize_or_zero();
        self.coll_pos = self.eye + dir * NEAR_CLIP;
        let mut shift = Vec3::ZERO;
        for hit in world.sphere_hits(self.old_coll_pos, self.coll_pos, INNER_RADIUS, Viewer::Camera) {
            modify_shift(&mut shift, hit.normal * -hit.depth);
            if hit.normal.y < 0.7 || hit.world_pos.y > obj_pos.y {
                self.hit_wall = true;
            }
        }
        self.coll_pos += shift;
        let to_obj = obj_pos - self.coll_pos;
        let new_dist = to_obj.length();
        let mut dir = Vec3::ZERO;
        if new_dist > 1e-6 {
            dir = to_obj / new_dist;
            if new_dist > dist - NEAR_CLIP {
                self.coll_pos = obj_pos + dir * (NEAR_CLIP - dist);
            }
        }
        self.eye = self.coll_pos - dir * NEAR_CLIP;
    }

    /// Adelante de la cámara.
    pub fn forward(&self) -> Vec3 {
        (self.target - self.eye).normalize_or_zero()
    }
}
