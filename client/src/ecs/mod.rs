//! Schedule de `bevy_ecs` standalone: un tick por frame, sin el motor Bevy.

use bevy_ecs::prelude::*;

#[derive(Resource)]
pub struct FrameIndex(pub u64);

fn advance_frame(mut frame: ResMut<FrameIndex>) {
    frame.0 = frame.0.wrapping_add(1);
}

pub struct FrameLoop {
    world: World,
    schedule: Schedule,
}

impl FrameLoop {
    pub fn new() -> Self {
        let mut world = World::new();
        world.insert_resource(FrameIndex(0));
        let mut schedule = Schedule::default();
        schedule.add_systems(advance_frame);
        Self { world, schedule }
    }

    pub fn tick(&mut self) {
        self.schedule.run(&mut self.world);
    }

    pub fn index(&self) -> u64 {
        self.world.resource::<FrameIndex>().0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_runs_once_per_frame() {
        let mut frames = FrameLoop::new();
        assert_eq!(frames.index(), 0);
        frames.tick();
        frames.tick();
        assert_eq!(frames.index(), 2);
    }
}
