//! La pista vista por la carrera: las zonas en el orden de la vuelta (`.taz` o `zones`), el
//! camino de POS nodes con lo que falta hasta la meta (`.pan` o `pos_nodes`), las kill
//! volumes y el borde del mundo.

use glam::{Quat, Vec3};
use revvy_formats::layout::TrackLayout;
use revvy_formats::Collision;

/// Afuera de la caja de la colisión más este margen (m), el auto se cayó del mundo.
const WORLD_MARGIN: f32 = 10.0;
/// Un POS node fuera de todas las zonas no sirve para reaparecer: se prueba con los
/// anteriores y después con los siguientes, hasta esta cantidad para cada lado.
const RESPAWN_SEARCH: usize = 8;

/// Caja orientada: una zona o una kill volume.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Obb {
    pub center: Vec3,
    pub rotation: Quat,
    pub half_extents: Vec3,
}

impl Obb {
    pub fn contains(&self, point: Vec3) -> bool {
        let local = self.rotation.inverse() * (point - self.center);
        local.abs().cmple(self.half_extents).all()
    }
}

/// Un POS node.
#[derive(Clone, Debug)]
pub(crate) struct PathNode {
    pub pos: Vec3,
    /// Lo que falta desde acá hasta la meta siguiendo la carrera (m): 0 en el nodo de
    /// largada.
    pub dist: f32,
    pub prev: Vec<usize>,
    pub next: Vec<usize>,
    /// Hacia atrás de la carrera: el promedio de las direcciones desde cada `next` hasta
    /// este nodo (`UpdateCarFinishDist`).
    pub back: Vec3,
}

#[derive(Clone, Debug)]
pub struct RaceTrack {
    /// Las cajas de cada id de zona. La vuelta recorre los ids en orden y la meta queda al
    /// pasar de la última a la 0.
    pub(crate) zones: Vec<Vec<Obb>>,
    pub(crate) nodes: Vec<PathNode>,
    pub(crate) start_node: usize,
    /// Largo de la vuelta (m).
    pub(crate) total: f32,
    kill_volumes: Vec<Obb>,
    /// La caja de la colisión, con margen.
    world: Option<(Vec3, Vec3)>,
}

impl RaceTrack {
    pub fn new(layout: &TrackLayout, collision: Option<&Collision>) -> Self {
        let count = layout
            .zones
            .iter()
            .map(|zone| zone.id.saturating_add(1).max(0) as usize)
            .max()
            .unwrap_or(0);
        let mut zones = vec![Vec::new(); count];
        for zone in layout.zones.iter().filter(|zone| zone.id >= 0) {
            zones[zone.id as usize].push(Obb {
                center: zone.center,
                rotation: zone.rotation,
                half_extents: zone.half_extents,
            });
        }
        if zones.iter().any(Vec::is_empty) {
            tracing::warn!(zonas = count, "faltan ids de zona: no se cuentan vueltas");
            zones.clear();
        }

        let len = layout.pos_nodes.len();
        let links = |ids: &[i32]| -> Vec<usize> {
            ids.iter()
                .filter_map(|&id| usize::try_from(id).ok())
                .filter(|&id| id < len)
                .collect()
        };
        let mut nodes: Vec<PathNode> = layout
            .pos_nodes
            .iter()
            .map(|node| PathNode {
                pos: node.position,
                dist: node.distance,
                prev: links(&node.prev),
                next: links(&node.next),
                back: Vec3::ZERO,
            })
            .collect();
        for i in 0..nodes.len() {
            let back: Vec3 = nodes[i]
                .next
                .iter()
                .map(|&next| (nodes[i].pos - nodes[next].pos).normalize_or_zero())
                .sum();
            nodes[i].back = back.normalize_or_zero();
        }

        let world = collision.and_then(|collision| {
            let mut points = collision.triangles.iter().flat_map(|tri| tri.positions);
            let first = points.next()?;
            let (min, max) = points.fold((first, first), |(min, max), p| (min.min(p), max.max(p)));
            Some((
                min - Vec3::splat(WORLD_MARGIN),
                max + Vec3::splat(WORLD_MARGIN),
            ))
        });
        Self {
            zones,
            nodes,
            start_node: (layout.start_node as usize).min(len.saturating_sub(1)),
            total: layout.total_distance,
            kill_volumes: layout
                .kill_volumes
                .iter()
                .map(|volume| Obb {
                    center: volume.center,
                    rotation: volume.rotation,
                    half_extents: volume.half_extents,
                })
                .collect(),
            world,
        }
    }

    /// Hay zonas y camino: se cuentan vueltas.
    pub fn has_laps(&self) -> bool {
        !self.zones.is_empty() && !self.nodes.is_empty() && self.total > 0.0
    }

    pub(crate) fn in_zone(&self, id: usize, point: Vec3) -> bool {
        self.zones[id].iter().any(|zone| zone.contains(point))
    }

    /// Dentro de alguna zona: en la pista. Sin zonas, siempre.
    pub fn on_track(&self, point: Vec3) -> bool {
        self.zones.is_empty() || self.zones.iter().flatten().any(|zone| zone.contains(point))
    }

    pub fn in_kill_volume(&self, point: Vec3) -> bool {
        self.kill_volumes
            .iter()
            .any(|volume| volume.contains(point))
    }

    /// Dentro de la caja de la pista con margen. Sin colisión, siempre.
    pub fn in_world(&self, point: Vec3) -> bool {
        self.world
            .is_none_or(|(min, max)| point.cmpge(min).all() && point.cmple(max).all())
    }

    /// Dónde reaparece un auto cuyo último POS node sano es `node`: sobre ese nodo, o el
    /// más cercano del camino que esté dentro de una zona, mirando hacia donde sigue la
    /// carrera (yaw sobre +Y).
    pub(crate) fn respawn_pose(&self, node: usize) -> Option<(Vec3, f32)> {
        let node = &self.nodes[self.respawn_node(node)?];
        let forward = -node.back;
        let yaw = if forward.x == 0.0 && forward.z == 0.0 {
            0.0
        } else {
            forward.x.atan2(forward.z)
        };
        Some((node.pos, yaw))
    }

    fn respawn_node(&self, start: usize) -> Option<usize> {
        self.nodes.get(start)?;
        let ok = |node: usize| self.on_track(self.nodes[node].pos);
        if ok(start) {
            return Some(start);
        }
        let walk = |links: fn(&PathNode) -> &[usize]| {
            let mut node = start;
            for _ in 0..RESPAWN_SEARCH {
                node = *links(&self.nodes[node]).first()?;
                if ok(node) {
                    return Some(node);
                }
            }
            None
        };
        walk(|node| &node.prev)
            .or_else(|| walk(|node| &node.next))
            .or(Some(start))
    }
}
