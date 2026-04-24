//! Input helpers. Maps raw u32 key codes (as stored on the Lua `input` table)
//! back into the raylib `KeyboardKey` enum.

use sola_raylib::prelude::*;

/// Converts a u32 into a `KeyboardKey`, or None for unknown codes.
/// Only the keys we expose via the `input.*` constants are recognised; this
/// acts as a whitelist so user Lua can't poke arbitrary key codes.
pub fn key_from_u32(k: u32) -> Option<KeyboardKey> {
    use KeyboardKey::*;
    match k {
        x if x == KEY_LEFT as u32 => Some(KEY_LEFT),
        x if x == KEY_RIGHT as u32 => Some(KEY_RIGHT),
        x if x == KEY_UP as u32 => Some(KEY_UP),
        x if x == KEY_DOWN as u32 => Some(KEY_DOWN),
        x if x == KEY_Z as u32 => Some(KEY_Z),
        x if x == KEY_X as u32 => Some(KEY_X),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_exposed_keys() {
        use KeyboardKey::*;
        for k in [KEY_LEFT, KEY_RIGHT, KEY_UP, KEY_DOWN, KEY_Z, KEY_X] {
            assert_eq!(
                key_from_u32(k as u32),
                Some(k),
                "key {k:?} should round-trip"
            );
        }
    }

    #[test]
    fn rejects_unexposed_keys() {
        use KeyboardKey::*;
        // Keys that exist in raylib but we don't expose should return None.
        for k in [KEY_SPACE, KEY_ENTER, KEY_A, KEY_B] {
            assert_eq!(
                key_from_u32(k as u32),
                None,
                "key {k:?} should not be recognised"
            );
        }
    }

    #[test]
    fn rejects_garbage_codes() {
        assert_eq!(key_from_u32(0), None);
        assert_eq!(key_from_u32(u32::MAX), None);
        assert_eq!(key_from_u32(99999), None);
    }
}
