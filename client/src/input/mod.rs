//! Gamepads vía gilrs. En esta fase solo se hace poll; los bindings llegan después.

pub struct Gamepads {
    gilrs: Option<gilrs::Gilrs>,
}

impl Gamepads {
    pub fn new() -> Self {
        match gilrs::Gilrs::new() {
            Ok(gilrs) => Self { gilrs: Some(gilrs) },
            Err(err) => {
                tracing::warn!(%err, "gilrs no disponible; se sigue sin gamepad");
                Self { gilrs: None }
            }
        }
    }

    pub fn poll(&mut self) {
        let Some(gilrs) = &mut self.gilrs else {
            return;
        };
        while let Some(event) = gilrs.next_event() {
            tracing::trace!(?event, "gilrs");
        }
    }
}
