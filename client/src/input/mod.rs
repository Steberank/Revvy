//! Teclado y mouse para la cámara libre.

use std::collections::HashSet;

use winit::event::{DeviceEvent, ElementState, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

#[derive(Clone, Copy, Debug, Default)]
pub struct FlyKeys {
    pub forward: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub fast: bool,
    pub look_dx: f32,
    pub look_dy: f32,
}

pub struct Input {
    keys: HashSet<KeyCode>,
    look_dx: f32,
    look_dy: f32,
}

impl Input {
    pub fn new() -> Self {
        Self {
            keys: HashSet::new(),
            look_dx: 0.0,
            look_dy: 0.0,
        }
    }

    pub fn window_event(&mut self, event: &WindowEvent) {
        let WindowEvent::KeyboardInput { event, .. } = event else {
            return;
        };
        let PhysicalKey::Code(code) = event.physical_key else {
            return;
        };
        if event.state == ElementState::Pressed {
            self.keys.insert(code);
        } else {
            self.keys.remove(&code);
        }
    }

    pub fn device_event(&mut self, event: &DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            self.look_dx += delta.0 as f32;
            self.look_dy += delta.1 as f32;
        }
    }

    pub fn fly(&mut self) -> FlyKeys {
        let keys = FlyKeys {
            forward: self.keys.contains(&KeyCode::KeyW),
            back: self.keys.contains(&KeyCode::KeyS),
            left: self.keys.contains(&KeyCode::KeyA),
            right: self.keys.contains(&KeyCode::KeyD),
            up: self.keys.contains(&KeyCode::KeyE),
            down: self.keys.contains(&KeyCode::KeyQ),
            fast: self.keys.contains(&KeyCode::ShiftLeft),
            look_dx: self.look_dx,
            look_dy: self.look_dy,
        };
        self.look_dx = 0.0;
        self.look_dy = 0.0;
        keys
    }
}
