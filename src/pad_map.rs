//! Per-game gamepad remapping for BTN1/BTN2/BTN3. Mirror of
//! `keymap.rs`: at most one override per action, stored beside
//! `settings.json` (web: same localStorage shim) as `pad_map.json`.
//! Directional actions (LEFT/RIGHT/UP/DOWN) are intentionally not
//! remappable here; they stay on dpad + left stick.
//!
//! Override "replaces" the gamepad portion of the binding entirely:
//! when an override is set, the default face buttons + triggers for
//! that action no longer fire, and the Nintendo south/east swap does
//! not apply (the player picked the specific physical button they
//! want).

use crate::game_id::GameId;
use crate::input::{ACTION_BTN1, ACTION_BTN2, ACTION_BTN3};
use sola_raylib::prelude::*;

#[cfg(not(target_os = "emscripten"))]
const PAD_MAP_FILE: &str = "pad_map.json";

/// One slot per remappable action. Index 0 = BTN1, 1 = BTN2, 2 = BTN3.
pub const PAD_OVERRIDE_COUNT: usize = 3;

/// Action ids the pad config screen iterates over. Order matches the
/// `overrides` array indexing.
pub const PAD_ACTIONS: [u32; PAD_OVERRIDE_COUNT] = [ACTION_BTN1, ACTION_BTN2, ACTION_BTN3];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PadMap {
    pub overrides: [Option<GamepadButton>; PAD_OVERRIDE_COUNT],
}

impl Default for PadMap {
    fn default() -> Self {
        Self {
            overrides: [None; PAD_OVERRIDE_COUNT],
        }
    }
}

/// Translates `ACTION_BTN*` into a slot in `PadMap::overrides`. Returns
/// `None` for any other action so directional actions skip the override
/// path entirely.
pub fn slot_for_action(action: u32) -> Option<usize> {
    PAD_ACTIONS.iter().position(|a| *a == action)
}

impl PadMap {
    pub fn override_for(&self, action: u32) -> Option<GamepadButton> {
        let i = slot_for_action(action)?;
        self.overrides.get(i).copied().flatten()
    }

    /// True if `b` is bound as any action's override. Lets the input
    /// layer suppress default buttons whose physical button was
    /// remapped elsewhere, so each button fires only its current owner.
    pub fn is_used_as_override(&self, b: GamepadButton) -> bool {
        self.overrides
            .iter()
            .any(|slot| matches!(slot, Some(x) if *x == b))
    }
}

/// Loads the per-game pad map. Returns the default (no overrides) on
/// any failure: missing file, parse error, IO error. Errors log to
/// stderr but never panic.
pub fn load(game_id: &GameId) -> PadMap {
    let body = match read_blob(game_id) {
        Ok(Some(s)) => s,
        Ok(None) => return PadMap::default(),
        Err(e) => {
            crate::msg::warn!("pad_map: read error: {e}; using defaults");
            return PadMap::default();
        }
    };
    let value: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            crate::msg::warn!("pad_map: parse error: {e}; using defaults");
            return PadMap::default();
        }
    };
    parse(&value)
}

fn parse(value: &serde_json::Value) -> PadMap {
    let mut pad_map = PadMap::default();
    let Some(obj) = value.get("overrides").and_then(|v| v.as_object()) else {
        return pad_map;
    };
    for (action_name, btn_value) in obj {
        let Some(slot) = PAD_ACTIONS.iter().position(|a| {
            crate::input::ACTION_NAMES[*a as usize - 1].eq_ignore_ascii_case(action_name)
        }) else {
            continue;
        };
        let Some(label) = btn_value.as_str() else {
            continue;
        };
        if let Some(b) = button_from_canonical(label) {
            pad_map.overrides[slot] = Some(b);
        }
    }
    pad_map
}

pub fn write(game_id: &GameId, pad_map: &PadMap) -> std::io::Result<()> {
    let mut overrides = serde_json::Map::new();
    for (i, slot) in pad_map.overrides.iter().enumerate() {
        if let Some(b) = slot
            && let Some(label) = button_canonical(*b)
        {
            let action = PAD_ACTIONS[i];
            overrides.insert(
                crate::input::ACTION_NAMES[action as usize - 1].to_string(),
                serde_json::Value::String(label.to_string()),
            );
        }
    }
    let json = serde_json::json!({ "overrides": overrides });
    let body = serde_json::to_string_pretty(&json)
        .map_err(|e| std::io::Error::other(format!("serialize pad_map: {e}")))?;
    write_blob(game_id, &body)
}

#[cfg(not(target_os = "emscripten"))]
pub fn pad_map_path(game_id: &GameId) -> std::io::Result<std::path::PathBuf> {
    Ok(crate::save::save_dir(game_id)?.join(PAD_MAP_FILE))
}

#[cfg(not(target_os = "emscripten"))]
fn read_blob(game_id: &GameId) -> std::io::Result<Option<String>> {
    let path = pad_map_path(game_id)?;
    match std::fs::read_to_string(&path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(not(target_os = "emscripten"))]
fn write_blob(game_id: &GameId, body: &str) -> std::io::Result<()> {
    let path = pad_map_path(game_id)?;
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
    crate::save::kv_read(&format!("usagi.pad_map.{}", game_id.as_str()))
}

#[cfg(target_os = "emscripten")]
fn write_blob(game_id: &GameId, body: &str) -> std::io::Result<()> {
    crate::save::kv_write(&format!("usagi.pad_map.{}", game_id.as_str()), body)
}

/// Buttons the pad config flow accepts as overrides: the 4 right-side
/// face buttons + the 4 shoulder/trigger positions. Dpad, thumb clicks,
/// and middle (Start/Select/Home) buttons are deliberately excluded so
/// capture can't bind a navigation button or something the directional
/// inputs already own.
pub const BINDABLE_BUTTONS: &[GamepadButton] = &[
    GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN,
    GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_RIGHT,
    GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_UP,
    GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_LEFT,
    GamepadButton::GAMEPAD_BUTTON_LEFT_TRIGGER_1,
    GamepadButton::GAMEPAD_BUTTON_LEFT_TRIGGER_2,
    GamepadButton::GAMEPAD_BUTTON_RIGHT_TRIGGER_1,
    GamepadButton::GAMEPAD_BUTTON_RIGHT_TRIGGER_2,
];

/// Canonical, family-agnostic name used in `pad_map.json`. Face buttons
/// are described by position on the diamond (FaceDown / FaceRight / etc)
/// instead of A/B/X/Y so the same JSON works regardless of which
/// gamepad family the player saves with. For display use
/// `crate::input::button_label`, which is family-aware.
pub fn button_canonical(b: GamepadButton) -> Option<&'static str> {
    Some(match b {
        GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN => "FaceDown",
        GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_RIGHT => "FaceRight",
        GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_UP => "FaceUp",
        GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_LEFT => "FaceLeft",
        GamepadButton::GAMEPAD_BUTTON_LEFT_TRIGGER_1 => "L1",
        GamepadButton::GAMEPAD_BUTTON_LEFT_TRIGGER_2 => "L2",
        GamepadButton::GAMEPAD_BUTTON_RIGHT_TRIGGER_1 => "R1",
        GamepadButton::GAMEPAD_BUTTON_RIGHT_TRIGGER_2 => "R2",
        _ => return None,
    })
}

pub fn button_from_canonical(label: &str) -> Option<GamepadButton> {
    Some(match label {
        "FaceDown" => GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN,
        "FaceRight" => GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_RIGHT,
        "FaceUp" => GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_UP,
        "FaceLeft" => GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_LEFT,
        "L1" => GamepadButton::GAMEPAD_BUTTON_LEFT_TRIGGER_1,
        "L2" => GamepadButton::GAMEPAD_BUTTON_LEFT_TRIGGER_2,
        "R1" => GamepadButton::GAMEPAD_BUTTON_RIGHT_TRIGGER_1,
        "R2" => GamepadButton::GAMEPAD_BUTTON_RIGHT_TRIGGER_2,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_all_none() {
        let m = PadMap::default();
        for slot in m.overrides.iter() {
            assert!(slot.is_none());
        }
    }

    #[test]
    fn override_for_returns_none_for_directional_actions() {
        // PadMap deliberately doesn't remap directionals; the type
        // expresses the constraint via `slot_for_action`.
        let mut m = PadMap::default();
        m.overrides[0] = Some(GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_RIGHT);
        assert_eq!(m.override_for(crate::input::ACTION_LEFT), None);
        assert_eq!(m.override_for(crate::input::ACTION_UP), None);
        assert_eq!(
            m.override_for(ACTION_BTN1),
            Some(GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_RIGHT),
        );
        // Unknown ids stay None rather than panic.
        assert_eq!(m.override_for(0), None);
        assert_eq!(m.override_for(99), None);
    }

    #[test]
    fn is_used_as_override_finds_buttons_in_any_slot() {
        let mut m = PadMap::default();
        let face_right = GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_RIGHT;
        assert!(!m.is_used_as_override(face_right));
        m.overrides[2] = Some(face_right);
        assert!(m.is_used_as_override(face_right));
        assert!(!m.is_used_as_override(GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_UP));
    }

    #[test]
    fn bindable_buttons_round_trip_through_canonical() {
        for b in BINDABLE_BUTTONS {
            let label = button_canonical(*b).expect("bindable has canonical name");
            let back = button_from_canonical(label).expect("canonical round-trips");
            assert_eq!(back, *b);
        }
    }

    #[test]
    fn reserved_buttons_have_no_canonical_label_and_are_not_in_bindable_set() {
        for b in [
            GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_UP,
            GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_DOWN,
            GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_LEFT,
            GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_RIGHT,
            GamepadButton::GAMEPAD_BUTTON_MIDDLE_LEFT,
            GamepadButton::GAMEPAD_BUTTON_MIDDLE,
            GamepadButton::GAMEPAD_BUTTON_MIDDLE_RIGHT,
            GamepadButton::GAMEPAD_BUTTON_LEFT_THUMB,
            GamepadButton::GAMEPAD_BUTTON_RIGHT_THUMB,
        ] {
            assert!(!BINDABLE_BUTTONS.contains(&b), "{b:?} must not be bindable");
            assert!(button_canonical(b).is_none());
        }
    }

    #[test]
    fn parse_picks_up_named_overrides_only() {
        // Recognized BTN actions stick; an unknown action (LEFT, since
        // directionals aren't remappable) and an unknown button label
        // silently drop. Action names compare case-insensitively.
        let body = r#"{
            "overrides": {
                "btn1": "FaceRight",
                "BTN2": "R1",
                "BTN3": "L9",
                "LEFT": "FaceDown"
            }
        }"#;
        let value: serde_json::Value = serde_json::from_str(body).unwrap();
        let m = parse(&value);
        assert_eq!(
            m.overrides[0],
            Some(GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_RIGHT)
        );
        assert_eq!(
            m.overrides[1],
            Some(GamepadButton::GAMEPAD_BUTTON_RIGHT_TRIGGER_1)
        );
        // BTN3 had an unknown button label.
        assert_eq!(m.overrides[2], None);
    }

    #[test]
    fn parse_returns_default_for_missing_overrides_section() {
        let value: serde_json::Value = serde_json::from_str("{}").unwrap();
        let m = parse(&value);
        assert_eq!(m, PadMap::default());
    }

    #[test]
    fn load_returns_default_for_missing_game_id() {
        let gid = GameId::resolve(Some("com.usagiengine.test-missing-pad-map"), None, None);
        let m = load(&gid);
        assert_eq!(m, PadMap::default());
    }
}
