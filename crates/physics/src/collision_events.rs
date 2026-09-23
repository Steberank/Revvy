//! Contactos del auto contra el mundo. Todavía no se persisten.

#[derive(Clone, Debug)]
pub struct CollisionEvent {
    pub impulse: f32,
}
