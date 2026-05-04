//! Per-game keyboard remapping. Sits alongside `settings.json` (web:
//! same localStorage shim). At most one keyboard override per
//! `ACTION_*`; gamepad and axis bindings stay at `input::BINDINGS`
//! defaults. Override "replaces" the keyboard portion (Pico-8 parity).

use crate::game_id::GameId;
use sola_raylib::prelude::*;

#[cfg(not(target_os = "emscripten"))]
const KEYMAP_FILE: &str = "keymap.json";

const ACTION_COUNT: usize = 7;

/// Per-action keyboard override. `overrides[i]` corresponds to action
/// id `i + 1` (matching the indexing scheme in `input::BINDINGS`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keymap {
    pub overrides: [Option<KeyboardKey>; ACTION_COUNT],
}

impl Default for Keymap {
    fn default() -> Self {
        Self {
            overrides: [None; ACTION_COUNT],
        }
    }
}

impl Keymap {
    /// Override for `action` (1-based) if one is set.
    pub fn override_for(&self, action: u32) -> Option<KeyboardKey> {
        let i = action.checked_sub(1)? as usize;
        self.overrides.get(i).copied().flatten()
    }

    /// True if `k` is bound as any action's override. Lets the input
    /// layer suppress default keys whose physical key was remapped
    /// elsewhere, so each key fires only its current owner.
    pub fn is_used_as_override(&self, k: KeyboardKey) -> bool {
        self.overrides
            .iter()
            .any(|slot| matches!(slot, Some(x) if *x == k))
    }
}

/// Loads the per-game keymap. Returns the default (no overrides) on
/// any failure: missing file, parse error, IO error. Errors log to
/// stderr but never panic.
pub fn load(game_id: &GameId) -> Keymap {
    let body = match read_blob(game_id) {
        Ok(Some(s)) => s,
        Ok(None) => return Keymap::default(),
        Err(e) => {
            crate::msg::warn!("keymap: read error: {e}; using defaults");
            return Keymap::default();
        }
    };
    let value: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            crate::msg::warn!("keymap: parse error: {e}; using defaults");
            return Keymap::default();
        }
    };
    parse(&value)
}

fn parse(value: &serde_json::Value) -> Keymap {
    let mut keymap = Keymap::default();
    let Some(obj) = value.get("overrides").and_then(|v| v.as_object()) else {
        return keymap;
    };
    for (action_name, key_value) in obj {
        let Some(action_idx) = crate::input::ACTION_NAMES
            .iter()
            .position(|n| n.eq_ignore_ascii_case(action_name))
        else {
            continue;
        };
        let Some(label) = key_value.as_str() else {
            continue;
        };
        if let Some(k) = key_from_label(label) {
            keymap.overrides[action_idx] = Some(k);
        }
    }
    keymap
}

/// Persists the keymap. Native writes are atomic (tempfile + rename);
/// web routes through the shared localStorage shim under
/// `usagi.keymap.<game_id>`.
pub fn write(game_id: &GameId, keymap: &Keymap) -> std::io::Result<()> {
    let mut overrides = serde_json::Map::new();
    for (i, slot) in keymap.overrides.iter().enumerate() {
        if let Some(key) = slot
            && let Some(label) = key_label(*key)
        {
            overrides.insert(
                crate::input::ACTION_NAMES[i].to_string(),
                serde_json::Value::String(label.to_string()),
            );
        }
    }
    let json = serde_json::json!({ "overrides": overrides });
    let body = serde_json::to_string_pretty(&json)
        .map_err(|e| std::io::Error::other(format!("serialize keymap: {e}")))?;
    write_blob(game_id, &body)
}

#[cfg(not(target_os = "emscripten"))]
pub fn keymap_path(game_id: &GameId) -> std::io::Result<std::path::PathBuf> {
    Ok(crate::save::save_dir(game_id)?.join(KEYMAP_FILE))
}

#[cfg(not(target_os = "emscripten"))]
fn read_blob(game_id: &GameId) -> std::io::Result<Option<String>> {
    let path = keymap_path(game_id)?;
    match std::fs::read_to_string(&path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(not(target_os = "emscripten"))]
fn write_blob(game_id: &GameId, body: &str) -> std::io::Result<()> {
    let path = keymap_path(game_id)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

#[cfg(target_os = "emscripten")]
fn read_blob(game_id: &GameId) -> std::io::Result<Option<String>> {
    crate::save::kv_read(&format!("usagi.keymap.{}", game_id.as_str()))
}

#[cfg(target_os = "emscripten")]
fn write_blob(game_id: &GameId, body: &str) -> std::io::Result<()> {
    crate::save::kv_write(&format!("usagi.keymap.{}", game_id.as_str()), body)
}

/// Stable canonical label for a key, layout-independent. Returns `None`
/// for keys that aren't in our supported capture set; callers must
/// reject those during the Key Config flow. Reserved (Esc, Enter,
/// Delete, F-keys, modifiers) are intentionally absent.
pub fn key_label(k: KeyboardKey) -> Option<&'static str> {
    Some(match k {
        // Letters
        KeyboardKey::KEY_A => "A",
        KeyboardKey::KEY_B => "B",
        KeyboardKey::KEY_C => "C",
        KeyboardKey::KEY_D => "D",
        KeyboardKey::KEY_E => "E",
        KeyboardKey::KEY_F => "F",
        KeyboardKey::KEY_G => "G",
        KeyboardKey::KEY_H => "H",
        KeyboardKey::KEY_I => "I",
        KeyboardKey::KEY_J => "J",
        KeyboardKey::KEY_K => "K",
        KeyboardKey::KEY_L => "L",
        KeyboardKey::KEY_M => "M",
        KeyboardKey::KEY_N => "N",
        KeyboardKey::KEY_O => "O",
        KeyboardKey::KEY_P => "P",
        KeyboardKey::KEY_Q => "Q",
        KeyboardKey::KEY_R => "R",
        KeyboardKey::KEY_S => "S",
        KeyboardKey::KEY_T => "T",
        KeyboardKey::KEY_U => "U",
        KeyboardKey::KEY_V => "V",
        KeyboardKey::KEY_W => "W",
        KeyboardKey::KEY_X => "X",
        KeyboardKey::KEY_Y => "Y",
        KeyboardKey::KEY_Z => "Z",
        // Digits (top row)
        KeyboardKey::KEY_ZERO => "0",
        KeyboardKey::KEY_ONE => "1",
        KeyboardKey::KEY_TWO => "2",
        KeyboardKey::KEY_THREE => "3",
        KeyboardKey::KEY_FOUR => "4",
        KeyboardKey::KEY_FIVE => "5",
        KeyboardKey::KEY_SIX => "6",
        KeyboardKey::KEY_SEVEN => "7",
        KeyboardKey::KEY_EIGHT => "8",
        KeyboardKey::KEY_NINE => "9",
        // Arrows
        KeyboardKey::KEY_LEFT => "Left",
        KeyboardKey::KEY_RIGHT => "Right",
        KeyboardKey::KEY_UP => "Up",
        KeyboardKey::KEY_DOWN => "Down",
        // Specials
        KeyboardKey::KEY_SPACE => "Space",
        KeyboardKey::KEY_TAB => "Tab",
        KeyboardKey::KEY_PERIOD => "Period",
        KeyboardKey::KEY_COMMA => "Comma",
        KeyboardKey::KEY_SLASH => "Slash",
        KeyboardKey::KEY_APOSTROPHE => "Apostrophe",
        KeyboardKey::KEY_SEMICOLON => "Semicolon",
        KeyboardKey::KEY_MINUS => "Minus",
        KeyboardKey::KEY_EQUAL => "Equal",
        KeyboardKey::KEY_LEFT_BRACKET => "LeftBracket",
        KeyboardKey::KEY_RIGHT_BRACKET => "RightBracket",
        KeyboardKey::KEY_BACKSLASH => "Backslash",
        KeyboardKey::KEY_GRAVE => "Backtick",
        KeyboardKey::KEY_INSERT => "Insert",
        KeyboardKey::KEY_HOME => "Home",
        KeyboardKey::KEY_END => "End",
        KeyboardKey::KEY_PAGE_UP => "PageUp",
        KeyboardKey::KEY_PAGE_DOWN => "PageDown",
        _ => return None,
    })
}

/// Inverse of `key_label`. Anything outside the supported capture set
/// returns `None` so a hand-edited keymap.json can't smuggle in
/// reserved keys.
pub fn key_from_label(label: &str) -> Option<KeyboardKey> {
    Some(match label {
        "A" => KeyboardKey::KEY_A,
        "B" => KeyboardKey::KEY_B,
        "C" => KeyboardKey::KEY_C,
        "D" => KeyboardKey::KEY_D,
        "E" => KeyboardKey::KEY_E,
        "F" => KeyboardKey::KEY_F,
        "G" => KeyboardKey::KEY_G,
        "H" => KeyboardKey::KEY_H,
        "I" => KeyboardKey::KEY_I,
        "J" => KeyboardKey::KEY_J,
        "K" => KeyboardKey::KEY_K,
        "L" => KeyboardKey::KEY_L,
        "M" => KeyboardKey::KEY_M,
        "N" => KeyboardKey::KEY_N,
        "O" => KeyboardKey::KEY_O,
        "P" => KeyboardKey::KEY_P,
        "Q" => KeyboardKey::KEY_Q,
        "R" => KeyboardKey::KEY_R,
        "S" => KeyboardKey::KEY_S,
        "T" => KeyboardKey::KEY_T,
        "U" => KeyboardKey::KEY_U,
        "V" => KeyboardKey::KEY_V,
        "W" => KeyboardKey::KEY_W,
        "X" => KeyboardKey::KEY_X,
        "Y" => KeyboardKey::KEY_Y,
        "Z" => KeyboardKey::KEY_Z,
        "0" => KeyboardKey::KEY_ZERO,
        "1" => KeyboardKey::KEY_ONE,
        "2" => KeyboardKey::KEY_TWO,
        "3" => KeyboardKey::KEY_THREE,
        "4" => KeyboardKey::KEY_FOUR,
        "5" => KeyboardKey::KEY_FIVE,
        "6" => KeyboardKey::KEY_SIX,
        "7" => KeyboardKey::KEY_SEVEN,
        "8" => KeyboardKey::KEY_EIGHT,
        "9" => KeyboardKey::KEY_NINE,
        "Left" => KeyboardKey::KEY_LEFT,
        "Right" => KeyboardKey::KEY_RIGHT,
        "Up" => KeyboardKey::KEY_UP,
        "Down" => KeyboardKey::KEY_DOWN,
        "Space" => KeyboardKey::KEY_SPACE,
        "Tab" => KeyboardKey::KEY_TAB,
        "Period" => KeyboardKey::KEY_PERIOD,
        "Comma" => KeyboardKey::KEY_COMMA,
        "Slash" => KeyboardKey::KEY_SLASH,
        "Apostrophe" => KeyboardKey::KEY_APOSTROPHE,
        "Semicolon" => KeyboardKey::KEY_SEMICOLON,
        "Minus" => KeyboardKey::KEY_MINUS,
        "Equal" => KeyboardKey::KEY_EQUAL,
        "LeftBracket" => KeyboardKey::KEY_LEFT_BRACKET,
        "RightBracket" => KeyboardKey::KEY_RIGHT_BRACKET,
        "Backslash" => KeyboardKey::KEY_BACKSLASH,
        "Backtick" => KeyboardKey::KEY_GRAVE,
        "Insert" => KeyboardKey::KEY_INSERT,
        "Home" => KeyboardKey::KEY_HOME,
        "End" => KeyboardKey::KEY_END,
        "PageUp" => KeyboardKey::KEY_PAGE_UP,
        "PageDown" => KeyboardKey::KEY_PAGE_DOWN,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_all_none() {
        let k = Keymap::default();
        for slot in k.overrides.iter() {
            assert!(slot.is_none());
        }
    }

    #[test]
    fn override_for_returns_set_key_and_none_otherwise() {
        let mut k = Keymap::default();
        k.overrides[0] = Some(KeyboardKey::KEY_W);
        assert_eq!(k.override_for(1), Some(KeyboardKey::KEY_W));
        assert_eq!(k.override_for(2), None);
        // Unknown action ids return None rather than panic.
        assert_eq!(k.override_for(0), None);
        assert_eq!(k.override_for(99), None);
    }

    #[test]
    fn is_used_as_override_finds_keys_in_any_slot() {
        let mut k = Keymap::default();
        assert!(!k.is_used_as_override(KeyboardKey::KEY_W));
        k.overrides[3] = Some(KeyboardKey::KEY_W);
        assert!(k.is_used_as_override(KeyboardKey::KEY_W));
        assert!(!k.is_used_as_override(KeyboardKey::KEY_A));
    }

    #[test]
    fn supported_keys_round_trip_through_label() {
        let sample = [
            KeyboardKey::KEY_A,
            KeyboardKey::KEY_Z,
            KeyboardKey::KEY_ZERO,
            KeyboardKey::KEY_NINE,
            KeyboardKey::KEY_LEFT,
            KeyboardKey::KEY_SPACE,
            KeyboardKey::KEY_TAB,
            KeyboardKey::KEY_GRAVE,
            KeyboardKey::KEY_PAGE_UP,
        ];
        for k in sample {
            let label = key_label(k).expect("supported key has label");
            let back = key_from_label(label).expect("label round-trips");
            assert_eq!(back, k);
        }
    }

    #[test]
    fn reserved_keys_are_unsupported() {
        // Esc / Enter / Delete / Backspace are reserved as menu
        // controls (cancel, advance, reset, undo); F-keys and
        // modifiers are intentionally excluded so capture can't bind
        // something with system meaning.
        for k in [
            KeyboardKey::KEY_ESCAPE,
            KeyboardKey::KEY_ENTER,
            KeyboardKey::KEY_DELETE,
            KeyboardKey::KEY_BACKSPACE,
            KeyboardKey::KEY_F1,
            KeyboardKey::KEY_LEFT_SHIFT,
            KeyboardKey::KEY_LEFT_CONTROL,
            KeyboardKey::KEY_LEFT_ALT,
        ] {
            assert!(
                key_label(k).is_none(),
                "{k:?} must not be in the supported capture set"
            );
        }
    }

    #[test]
    fn parse_picks_up_named_overrides_only() {
        // Recognized actions stick; unknown action names + unknown key
        // labels silently drop, leaving the slot at None.
        let body = r#"{
            "overrides": {
                "LEFT": "W",
                "BTN1": "Z",
                "PAUSE": "Tab",
                "BTN3": "F13",
                "btn2": "X"
            }
        }"#;
        let value: serde_json::Value = serde_json::from_str(body).unwrap();
        let k = parse(&value);
        assert_eq!(k.overrides[0], Some(KeyboardKey::KEY_W));
        assert_eq!(k.overrides[4], Some(KeyboardKey::KEY_Z));
        // BTN3 had an unknown key label.
        assert_eq!(k.overrides[6], None);
        // Unknown action name PAUSE is dropped silently.
        // Action names compare case-insensitively, so "btn2" → BTN2.
        assert_eq!(k.overrides[5], Some(KeyboardKey::KEY_X));
    }

    #[test]
    fn parse_returns_default_for_missing_overrides_section() {
        let value: serde_json::Value = serde_json::from_str("{}").unwrap();
        let k = parse(&value);
        assert_eq!(k, Keymap::default());
    }

    #[test]
    fn load_returns_default_for_missing_game_id() {
        let gid = GameId::resolve(Some("com.usagiengine.test-missing-keymap"), None, None);
        let k = load(&gid);
        assert_eq!(k, Keymap::default());
    }
}
