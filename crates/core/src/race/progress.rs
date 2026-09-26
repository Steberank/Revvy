//! Por dónde va un auto en la vuelta, como `UpdateCarAiZone` (`aizone.cpp`) y
//! `UpdateCarFinishDist` (`posnode.cpp`) de Re-Volt.
//!
//! - La zona solo avanza a la siguiente o retrocede a la anterior: para volver a la meta
//!   hay que pasar por todas, en orden.
//! - Afuera de su zona, el auto no avanza en la vuelta y va de contramano.
//! - El POS node solo pasa a un vecino más cercano; lo que falta hasta la meta es su
//!   distancia más lo que el auto se corrió sobre el camino.
//! - La fracción que falta (`panel`) va de 1 a 0 en la vuelta. Pasar de menos de 0.25 a
//!   más de 0.75 es cruzar la línea; al revés, cruzarla hacia atrás (`BackTracking`), y el
//!   próximo cruce no cuenta. El primer cruce solo arranca la primera vuelta (`PreLap`).

use glam::Vec3;

use super::track::RaceTrack;

/// `WRONG_WAY_TOLERANCE`: el aviso de contramano cambia después de 1 s en el otro estado.
const WRONG_WAY_TOLERANCE: f32 = 1.0;
/// Mirando hacia atrás del camino más que esto (el coseno), va de contramano.
const WRONG_WAY_DOT: f32 = 0.6;

/// Qué pasó con la línea de meta en un paso.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Crossing {
    /// El primer cruce: empieza la primera vuelta.
    Start,
    /// Completó una vuelta.
    Lap,
}

#[derive(Clone, Debug)]
pub(crate) struct Progress {
    zone: usize,
    /// El POS node más cercano (`FinishDistNode`).
    node: usize,
    /// El último `node` con el auto dentro de su zona: de ahí reaparece.
    valid_node: usize,
    /// Fracción de la vuelta que falta hasta la meta (`FinishDistPanel`), de 0 a 1.
    panel: f32,
    pre_lap: bool,
    back_tracking: bool,
    /// `CarAI.WrongWay`: mira hacia atrás del camino, o salió de su zona.
    wrong_way: bool,
    /// El aviso (`WrongWayFlag`): sigue a `wrong_way` después de `WRONG_WAY_TOLERANCE`.
    warning: bool,
    warning_timer: f32,
}

impl Progress {
    /// En la largada, como `AI_InitPlayerAI`: zona 0, el nodo de largada y antes de la
    /// línea. Los autos de Re-Volt largan en la última zona, detrás de la meta.
    pub fn new(track: &RaceTrack, pos: Vec3, forward: Vec3) -> Self {
        let mut progress = Self {
            zone: 0,
            node: track.start_node,
            valid_node: track.start_node,
            panel: 0.0,
            pre_lap: true,
            back_tracking: true,
            wrong_way: false,
            warning: false,
            warning_timer: 0.0,
        };
        progress.update_zone(track, pos);
        progress.update_dist(track, pos, forward);
        progress
    }

    /// Un paso de `dt` segundos. Si cruzó la línea, qué pasó y qué parte del paso fue
    /// después de cruzarla (de 0 a 1), para medir la vuelta.
    pub fn update(
        &mut self,
        track: &RaceTrack,
        pos: Vec3,
        forward: Vec3,
        dt: f32,
    ) -> Option<(Crossing, f32)> {
        self.update_zone(track, pos);
        let crossing = self.update_dist(track, pos, forward);
        // `panel.cpp`: el aviso cambia recién después de un segundo en el otro estado.
        if self.warning != self.wrong_way {
            self.warning_timer += dt;
            if self.warning_timer >= WRONG_WAY_TOLERANCE {
                self.warning = self.wrong_way;
                self.warning_timer = 0.0;
            }
        } else {
            self.warning_timer = 0.0;
        }
        crossing
    }

    pub fn valid_node(&self) -> usize {
        self.valid_node
    }

    pub fn warning(&self) -> bool {
        self.warning
    }

    /// Cuánto de la vuelta en curso ya corrió (0 a 1). Antes del primer cruce es negativo:
    /// lo que le falta para llegar a la línea.
    pub fn covered(&self) -> f32 {
        if self.pre_lap {
            -self.panel
        } else {
            1.0 - self.panel
        }
    }

    /// `UpdateCarAiZone`: sigue en su zona, o pasa a la siguiente o a la anterior.
    fn update_zone(&mut self, track: &RaceTrack, pos: Vec3) {
        let count = track.zones.len();
        if track.in_zone(self.zone, pos) {
            return;
        }
        let next = (self.zone + 1) % count;
        if track.in_zone(next, pos) {
            self.zone = next;
            return;
        }
        let prev = (self.zone + count - 1) % count;
        if track.in_zone(prev, pos) {
            self.zone = prev;
        }
    }

    /// `UpdateCarFinishDist`.
    fn update_dist(
        &mut self,
        track: &RaceTrack,
        pos: Vec3,
        forward: Vec3,
    ) -> Option<(Crossing, f32)> {
        if !track.in_zone(self.zone, pos) {
            self.wrong_way = true;
            return None;
        }
        let nodes = &track.nodes;
        let current = &nodes[self.node];
        let mut nearest = (pos.distance_squared(current.pos), self.node);
        for &link in current.prev.iter().chain(&current.next) {
            let dist = pos.distance_squared(nodes[link].pos);
            if dist < nearest.0 {
                nearest = (dist, link);
            }
        }
        self.node = nearest.1;
        self.valid_node = self.node;
        let node = &nodes[self.node];
        let finish_dist = node.dist + node.back.dot(pos - node.pos);
        self.wrong_way = node.back.dot(forward) > WRONG_WAY_DOT;

        let last = self.panel;
        let mut add = finish_dist / track.total - self.panel;
        if add > 0.5 {
            add -= 1.0;
        } else if add < -0.5 {
            add += 1.0;
        }
        self.panel += add;
        if self.panel >= 1.0 {
            self.panel -= 1.0;
        } else if self.panel < 0.0 {
            self.panel += 1.0;
        }

        if last < 0.25 && self.panel > 0.75 {
            let after = 1.0 - self.panel;
            let fraction = if after + last > 0.0 {
                after / (after + last)
            } else {
                0.0
            };
            let was_pre_lap = std::mem::take(&mut self.pre_lap);
            if std::mem::take(&mut self.back_tracking) {
                return was_pre_lap.then_some((Crossing::Start, fraction));
            }
            return Some((Crossing::Lap, fraction));
        }
        if last > 0.75 && self.panel < 0.25 {
            self.back_tracking = true;
        }
        None
    }
}
