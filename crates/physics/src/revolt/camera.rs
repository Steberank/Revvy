//! Cámara de persecución de `camera.cpp`: `CAM_FOLLOW_BEHIND`, la que usa el jugador
//! por defecto (`SetCameraFollow(CAM_MainCamera, …, 0)`).

use glam::Vec3;

use super::coll::{modify_shift, sphere_coll_poly};
use super::level::CollWorld;
use super::math::{build_look_matrix_forward, vec_mul_mat, Mat, IDENTITY};
use super::units::SMALL_REAL;

/// `CamFollowData[CAM_FOLLOW_BEHIND]`.
const BEHIND_POS_OFFSET: Vec3 = Vec3::new(0.0, -150.0, -460.0);
const BEHIND_LOOK_OFFSET: Vec3 = Vec3::new(0.0, -60.0, 0.0);
/// `InnerRadius`, `NEAR_CLIP_DIST` y `CAM_MOVE_SPEED`.
const INNER_RADIUS: f32 = 65.0;
const NEAR_CLIP: f32 = 30.0;
const CAM_MOVE_SPEED: f32 = 2.0;
/// `obj->CamLength` de los autos.
const CAM_LENGTH: f32 = 1.0;

#[derive(Clone, Debug)]
pub struct FollowCamera {
    pub wpos: Vec3,
    pub old_wpos: Vec3,
    pub wmatrix: Mat,
    pub vel: Vec3,
    pos_offset: Vec3,
    dest_offset: Vec3,
    look_offset: Vec3,
    world_pos_offset: Vec3,
    los_scale: f32,
    coll_pos: Vec3,
    old_coll_pos: Vec3,
    has_collided_with_wall: bool,
}

/// Matriz horizontal que mira como el auto (`InitCamPos` / `CameraFollowPos`).
fn heading_matrix(obj: &Mat) -> Mat {
    let mut r = Vec3::new(obj.l.z, 0.0, -obj.l.x);
    if r.x == 0.0 && r.z == 0.0 {
        r.x = 1.0;
    }
    let r = r.normalize();
    let u = Vec3::Y;
    Mat { r, u, l: r.cross(u) }
}

impl FollowCamera {
    /// `SetCameraFollow` + `InitCamPos`. En el juego `PosOffset` viene de la cámara
    /// anterior (el barrido de la largada); acá arranca ya detrás del auto.
    pub fn new(obj_pos: Vec3, obj_mat: &Mat) -> Self {
        let mat = heading_matrix(obj_mat);
        let pos_offset = BEHIND_POS_OFFSET;
        let world_pos_offset = vec_mul_mat(pos_offset, &mat);
        let wpos = world_pos_offset + obj_pos;
        let mut camera = Self {
            wpos,
            old_wpos: wpos,
            wmatrix: IDENTITY,
            vel: Vec3::ZERO,
            pos_offset,
            dest_offset: BEHIND_POS_OFFSET,
            look_offset: BEHIND_LOOK_OFFSET,
            world_pos_offset,
            los_scale: 1.0,
            coll_pos: wpos,
            old_coll_pos: wpos,
            has_collided_with_wall: false,
        };
        camera.away_look(obj_pos, obj_mat, None, 0.0);
        camera
    }

    /// `UpdateCamera`: posición (`CameraFollowPos`) y después la mirada (`CameraAwayLook`).
    pub fn update(&mut self, dt: f32, obj_pos: Vec3, obj_mat: &Mat, level: &CollWorld) {
        self.has_collided_with_wall = false;
        self.follow_pos(dt, obj_pos, obj_mat, level);
        self.away_look(obj_pos, obj_mat, Some(level), dt);
    }

    fn follow_pos(&mut self, dt: f32, obj_pos: Vec3, obj_mat: &Mat, level: &CollWorld) {
        self.old_wpos = self.wpos;
        let mat = heading_matrix(obj_mat);

        let mut offset = self.dest_offset * self.los_scale;
        offset.y *= CAM_LENGTH;
        let scale = (1.0 + CAM_LENGTH) * 0.5;
        offset.x *= scale;
        offset.z *= scale;
        // Por frame, no por tiempo: el palo se estira un cuarto de lo que falta.
        self.pos_offset += (offset - self.pos_offset) * 0.25;

        let mut new_pole = vec_mul_mat(self.pos_offset, &mat);
        let home_len = self.pos_offset.length();
        if home_len > SMALL_REAL {
            new_pole /= home_len;
        }

        let mut pole_len = self.world_pos_offset.length();
        if pole_len > SMALL_REAL {
            self.world_pos_offset /= pole_len;
        }

        let delta = new_pole - self.world_pos_offset;
        let mut new_pole = self.world_pos_offset + delta * (dt / 0.25);
        if new_pole.x.abs() > SMALL_REAL || new_pole.y.abs() > SMALL_REAL || new_pole.z.abs() > SMALL_REAL {
            new_pole = new_pole.normalize();
        }
        pole_len += (home_len - pole_len) * dt / 0.30;
        new_pole *= pole_len;

        self.wpos = obj_pos + new_pole;
        self.world_colls(obj_pos, level);

        self.world_pos_offset = self.wpos - obj_pos;
        self.old_coll_pos = self.coll_pos;
        self.vel = if dt > SMALL_REAL {
            (self.wpos - self.old_wpos) / dt
        } else {
            Vec3::ZERO
        };
    }

    /// `CameraWorldColls`: una esfera delante del lente no atraviesa el mundo.
    fn world_colls(&mut self, obj_pos: Vec3, level: &CollWorld) {
        let mut shift = Vec3::ZERO;
        let to_obj = obj_pos - self.wpos;
        let obj_cam_dist = to_obj.length();
        let dir = if obj_cam_dist > SMALL_REAL {
            to_obj / obj_cam_dist
        } else {
            Vec3::ZERO
        };
        self.coll_pos = self.wpos + dir * NEAR_CLIP;
        let Some(cell) = level.grid_for(self.coll_pos) else {
            return;
        };
        let radius = (INNER_RADIUS * CAM_LENGTH).max(INNER_RADIUS * 0.75);
        for &index in cell {
            let poly = &level.polys[index as usize];
            if poly.object_only() {
                continue;
            }
            if let Some(hit) = sphere_coll_poly(self.old_coll_pos, self.coll_pos, radius, poly) {
                modify_shift(&mut shift, -hit.depth, hit.plane.n);
                if hit.plane.n.y > -0.7 || hit.world_pos.y < obj_pos.y {
                    self.has_collided_with_wall = true;
                }
            }
        }
        self.coll_pos += shift;

        let to_obj = obj_pos - self.coll_pos;
        let new_dist = to_obj.length();
        let mut dir = Vec3::ZERO;
        if new_dist > SMALL_REAL {
            dir = to_obj / new_dist;
            if new_dist > obj_cam_dist - NEAR_CLIP {
                self.coll_pos = obj_pos + dir * (-obj_cam_dist + NEAR_CLIP);
            }
        }
        self.wpos = self.coll_pos - dir * NEAR_CLIP;
    }

    /// `CameraAwayLook`: mira al auto, un poco por encima; sin vista, el palo se acorta.
    fn away_look(&mut self, obj_pos: Vec3, obj_mat: &Mat, level: Option<&CollWorld>, dt: f32) {
        let dr = obj_pos - self.wpos;
        let dr_len = dr.length();
        if let Some(level) = level {
            if dr_len > 100.0 && !level.line_of_sight(self.coll_pos, obj_pos) {
                self.los_scale = (self.los_scale - CAM_MOVE_SPEED * dt).max(0.3);
            } else if !self.has_collided_with_wall {
                self.los_scale = (self.los_scale + CAM_MOVE_SPEED * dt).min(1.0);
            }
        }
        if dr_len < 50.0 {
            return;
        }
        let mut look_off = self.look_offset;
        let y = look_off.y;
        look_off.y = 0.0;
        let mut look_pos = vec_mul_mat(look_off, obj_mat);
        look_pos.y = y;
        look_pos += obj_pos;
        self.wmatrix = build_look_matrix_forward(self.wpos, look_pos);
    }
}
