//! Teclado y mouse: manejo del auto y cámara libre.
//!
//! Con la cámara del auto, WASD y las flechas manejan. Con la cámara libre, WASD
//! mueve la cámara y las flechas siguen manejando.

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

/// Teclas del auto en un frame. `CRD_KeyboardInput` las lleva a `dx` / `dy`.
#[derive(Clone, Copy, Debug, Default)]
pub struct DriveKeys {
    pub accelerate: bool,
    pub brake: bool,
    pub left: bool,
    pub right: bool,
    pub reset: bool,
}

pub struct Input {
    keys: HashSet<KeyCode>,
    pressed: HashSet<KeyCode>,
    look_dx: f32,
    look_dy: f32,
}

impl Input {
    pub fn new() -> Self {
        Self {
            keys: HashSet::new(),
            pressed: HashSet::new(),
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
            if self.keys.insert(code) {
                self.pressed.insert(code);
            }
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

    fn held(&self, code: KeyCode) -> bool {
        self.keys.contains(&code)
    }

    /// `true` una sola vez por pulsación.
    pub fn take_pressed(&mut self, code: KeyCode) -> bool {
        self.pressed.remove(&code)
    }

    /// Cámara libre. Consume el movimiento del mouse acumulado.
    pub fn fly(&mut self) -> FlyKeys {
        let keys = FlyKeys {
            forward: self.held(KeyCode::KeyW),
            back: self.held(KeyCode::KeyS),
            left: self.held(KeyCode::KeyA),
            right: self.held(KeyCode::KeyD),
            up: self.held(KeyCode::KeyE),
            down: self.held(KeyCode::KeyQ),
            fast: self.held(KeyCode::ShiftLeft),
            look_dx: self.look_dx,
            look_dy: self.look_dy,
        };
        self.look_dx = 0.0;
        self.look_dy = 0.0;
        keys
    }

    /// Manejo. Con `wasd` en falso solo cuentan las flechas.
    pub fn drive(&self, wasd: bool) -> DriveKeys {
        DriveKeys {
            accelerate: self.held(KeyCode::ArrowUp) || (wasd && self.held(KeyCode::KeyW)),
            brake: self.held(KeyCode::ArrowDown) || (wasd && self.held(KeyCode::KeyS)),
            left: self.held(KeyCode::ArrowLeft) || (wasd && self.held(KeyCode::KeyA)),
            right: self.held(KeyCode::ArrowRight) || (wasd && self.held(KeyCode::KeyD)),
            reset: self.held(KeyCode::KeyR),
        }
    }

    /// Descarta las pulsaciones que nadie leyó en este frame.
    pub fn end_frame(&mut self) {
        self.pressed.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_press_is_reported_once() {
        let mut input = Input::new();
        input.keys.insert(KeyCode::KeyC);
        input.pressed.insert(KeyCode::KeyC);
        assert!(input.take_pressed(KeyCode::KeyC));
        assert!(!input.take_pressed(KeyCode::KeyC));
    }
}
