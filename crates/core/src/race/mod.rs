//! Carrera local (§4.7, §7.12): vueltas, posiciones, contramano y reposición, la misma para
//! pistas de Re-Volt y propias. Es lógica pura: recibe dónde está cada auto y avisa qué
//! pasó; mover los autos y dibujar queda afuera.
//!
//! La vuelta sigue a Re-Volt (`progress.rs`). Un auto se reposiciona (§7.12) cuando entra
//! en una kill volume, cuando sale de la caja de la pista o cuando pasa más de
//! `off_track_secs` fuera de todas las zonas; también cuando lo pide el jugador. Reaparece
//! en su último POS node sano, derecho y mirando hacia donde sigue la carrera.

mod progress;
mod track;

use glam::Vec3;

use crate::rules::GameplayRules;
use progress::{Crossing, Progress};
pub use track::{Obb, RaceTrack};

/// Después de reaparecer, el auto no se reposiciona solo durante este tiempo (s): si
/// reaparece al borde de una kill volume, no entra en una vuelta de reposiciones.
const RESPAWN_GRACE: f32 = 1.0;

/// Dónde está un auto y hacia dónde mira.
#[derive(Clone, Copy, Debug)]
pub struct CarPose {
    pub pos: Vec3,
    pub forward: Vec3,
}

/// Por qué hay que reposicionar un auto (§7.12).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RespawnReason {
    /// Entró en una kill volume.
    KillVolume,
    /// Pasó más de `off_track_secs` fuera de todas las zonas.
    OffTrack,
    /// Salió de la caja de la pista.
    OutOfWorld,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RaceEvent {
    /// El auto completó la vuelta `lap` (1, 2…), que duró `time` s.
    Lap { car: usize, lap: u32, time: f64 },
    /// El auto terminó la carrera: `time` s desde la largada.
    Finished { car: usize, time: f64 },
    /// Hay que llevar el auto a `Race::respawn_pose`.
    Respawn { car: usize, reason: RespawnReason },
}

/// Un auto en carrera.
#[derive(Clone, Debug)]
pub struct Racer {
    /// `None` si la pista no tiene zonas o camino: no hay vueltas.
    progress: Option<Progress>,
    laps: u32,
    /// Cuándo empezó la vuelta en curso. `None` hasta cruzar la línea por primera vez.
    lap_start: Option<f64>,
    last_lap: Option<f64>,
    best_lap: Option<f64>,
    finish: Option<f64>,
    /// Segundos seguidos fuera de todas las zonas.
    off_track: f32,
    /// Segundos que le quedan sin reposicionarse solo.
    grace: f32,
}

impl Racer {
    /// Vueltas completas.
    pub fn laps(&self) -> u32 {
        self.laps
    }

    /// Lo que va de la vuelta en curso, desde que cruzó la línea.
    pub fn lap_time(&self, now: f64) -> Option<f64> {
        self.lap_start
            .filter(|_| self.finish.is_none())
            .map(|start| now - start)
    }

    pub fn last_lap(&self) -> Option<f64> {
        self.last_lap
    }

    pub fn best_lap(&self) -> Option<f64> {
        self.best_lap
    }

    /// El tiempo total, si terminó.
    pub fn finish_time(&self) -> Option<f64> {
        self.finish
    }

    /// El aviso de contramano. Después de terminar no se muestra.
    pub fn wrong_way(&self) -> bool {
        self.finish.is_none() && self.progress.as_ref().is_some_and(Progress::warning)
    }

    /// Para ordenar: los que terminaron primero, por tiempo; después, por lo que corrieron.
    fn rank(&self) -> (u8, f64) {
        match (self.finish, &self.progress) {
            (Some(time), _) => (0, time),
            (None, Some(progress)) => (1, -(f64::from(self.laps) + f64::from(progress.covered()))),
            (None, None) => (2, 0.0),
        }
    }
}

pub struct Race {
    track: RaceTrack,
    laps: u32,
    off_track_secs: f32,
    /// Segundos desde la largada.
    time: f64,
    racers: Vec<Racer>,
}

impl Race {
    /// `cars`: cada auto en la largada, en el orden de la grilla.
    pub fn new(track: RaceTrack, rules: &GameplayRules, cars: &[CarPose]) -> Self {
        let racers = cars
            .iter()
            .map(|car| Racer {
                progress: track
                    .has_laps()
                    .then(|| Progress::new(&track, car.pos, car.forward)),
                laps: 0,
                lap_start: None,
                last_lap: None,
                best_lap: None,
                finish: None,
                off_track: 0.0,
                grace: 0.0,
            })
            .collect();
        Self {
            track,
            laps: rules.laps.max(1),
            off_track_secs: rules.off_track_secs,
            time: 0.0,
            racers,
        }
    }

    /// Un frame de `dt` segundos con los autos en `cars`, en el orden de `new`.
    pub fn update(&mut self, dt: f32, cars: &[CarPose]) -> Vec<RaceEvent> {
        self.time += f64::from(dt);
        let mut events = Vec::new();
        for (car, (racer, pose)) in self.racers.iter_mut().zip(cars).enumerate() {
            if let Some(progress) = &mut racer.progress {
                if let Some((crossing, after)) =
                    progress.update(&self.track, pose.pos, pose.forward, dt)
                {
                    // El momento exacto del cruce, dentro del paso.
                    let at = self.time - f64::from(after * dt);
                    match crossing {
                        Crossing::Start => racer.lap_start = Some(at),
                        Crossing::Lap if racer.finish.is_none() => {
                            let time = at - racer.lap_start.unwrap_or(0.0);
                            racer.laps += 1;
                            racer.last_lap = Some(time);
                            racer.best_lap =
                                Some(racer.best_lap.map_or(time, |best| best.min(time)));
                            racer.lap_start = Some(at);
                            events.push(RaceEvent::Lap {
                                car,
                                lap: racer.laps,
                                time,
                            });
                            if racer.laps >= self.laps {
                                racer.finish = Some(at);
                                events.push(RaceEvent::Finished { car, time: at });
                            }
                        }
                        Crossing::Lap => {}
                    }
                }
            }

            if racer.grace > 0.0 {
                racer.grace = (racer.grace - dt).max(0.0);
                racer.off_track = 0.0;
                continue;
            }
            let reason = if self.track.in_kill_volume(pose.pos) {
                Some(RespawnReason::KillVolume)
            } else if !self.track.in_world(pose.pos) {
                Some(RespawnReason::OutOfWorld)
            } else if !self.track.on_track(pose.pos) {
                racer.off_track += dt;
                (racer.off_track >= self.off_track_secs).then_some(RespawnReason::OffTrack)
            } else {
                racer.off_track = 0.0;
                None
            };
            if let Some(reason) = reason {
                racer.grace = RESPAWN_GRACE;
                racer.off_track = 0.0;
                events.push(RaceEvent::Respawn { car, reason });
            }
        }
        events
    }

    /// Dónde reaparece el auto: su último POS node sano, mirando hacia donde sigue la
    /// carrera (yaw sobre +Y). `None` si la pista no tiene camino.
    pub fn respawn_pose(&self, car: usize) -> Option<(Vec3, f32)> {
        let node = self.racers.get(car)?.progress.as_ref()?.valid_node();
        self.track.respawn_pose(node)
    }

    /// El auto ya se reposicionó (por ejemplo, lo pidió el jugador): un rato sin volver a
    /// hacerlo solo.
    pub fn respawned(&mut self, car: usize) {
        if let Some(racer) = self.racers.get_mut(car) {
            racer.grace = RESPAWN_GRACE;
            racer.off_track = 0.0;
        }
    }

    /// Hay vueltas: la pista tiene zonas y camino.
    pub fn has_laps(&self) -> bool {
        self.track.has_laps()
    }

    /// Las vueltas de la carrera.
    pub fn laps(&self) -> u32 {
        self.laps
    }

    /// Segundos desde la largada.
    pub fn time(&self) -> f64 {
        self.time
    }

    pub fn racer(&self, car: usize) -> &Racer {
        &self.racers[car]
    }

    /// Los autos en el orden de la carrera: primero los que terminaron, por tiempo;
    /// después los demás, por vueltas y por lo que les falta hasta la meta.
    pub fn standings(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.racers.len()).collect();
        order.sort_by(|&a, &b| {
            let (a, b) = (self.racers[a].rank(), self.racers[b].rank());
            a.0.cmp(&b.0).then(a.1.total_cmp(&b.1))
        });
        order
    }

    /// El puesto del auto, desde 1.
    pub fn position(&self, car: usize) -> usize {
        self.standings()
            .iter()
            .position(|&i| i == car)
            .map_or(0, |i| i + 1)
    }

    /// Los que terminaron, en el orden en que llegaron, con su tiempo.
    pub fn results(&self) -> Vec<(usize, f64)> {
        let mut done: Vec<(usize, f64)> = self
            .racers
            .iter()
            .enumerate()
            .filter_map(|(car, racer)| Some((car, racer.finish?)))
            .collect();
        done.sort_by(|a, b| a.1.total_cmp(&b.1));
        done
    }
}

#[cfg(test)]
mod tests;
