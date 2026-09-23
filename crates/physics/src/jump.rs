//! Salto del auto. `allow_jump` sale de `GameplayRules` y arranca en falso.

pub const DEFAULT_ALLOW_JUMP: bool = false;

pub fn impulse(allow_jump: bool) -> Option<f32> {
    if allow_jump {
        Some(6.0)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jump_is_gated_off_by_default() {
        assert!(!DEFAULT_ALLOW_JUMP);
        assert!(impulse(DEFAULT_ALLOW_JUMP).is_none());
        assert_eq!(impulse(true), Some(6.0));
    }
}
