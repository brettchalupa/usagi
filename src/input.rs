//! Abstract input actions. User Lua references actions via `input.LEFT`
//! etc. (integer IDs); at runtime each action is a union over keyboard
//! keys, gamepad buttons, and analog-stick directions. Adding a binding
//! only requires extending the `BINDINGS` table.

use sola_raylib::prelude::*;

// Action IDs. Stable integers; `setup_api` exposes these as `input.LEFT`
// etc. on the Lua side.
pub const ACTION_LEFT: u32 = 1;
pub const ACTION_RIGHT: u32 = 2;
pub const ACTION_UP: u32 = 3;
pub const ACTION_DOWN: u32 = 4;
pub const ACTION_BTN1: u32 = 5;
pub const ACTION_BTN2: u32 = 6;
pub const ACTION_BTN3: u32 = 7;

/// Deadzone for analog-stick direction checks. Values within +/- this
/// range count as centered.
const STICK_DEADZONE: f32 = 0.3;

/// Upper bound on gamepad slots to poll. Matches sola-raylib max. Any connected
/// pad (Steam Deck built-in, external pad over USB/Bluetooth, dongle) fires
/// every action, independent of slot index. So hot-swapping works no problem.
/// This is naive but works for what Usagi needs.
pub const MAX_GAMEPADS: i32 = 4;

/// Bindings for a single action: the keyboard keys, gamepad buttons, and
/// analog-axis directions that all count as "this action is pressed".
struct Binding {
    keys: &'static [KeyboardKey],
    buttons: &'static [GamepadButton],
    /// (axis, sign) pairs. Sign is -1 for "tilt negative" or +1 for "tilt
    /// positive"; either direction past the deadzone triggers the action.
    axes: &'static [(GamepadAxis, i8)],
}

/// Indexed by action_id - 1. The source of truth for the input map.
/// Add a new row and a matching `ACTION_*` constant to introduce a new
/// action; `is_valid_action` / `action_down` / `action_pressed` will
/// automatically include it.
const BINDINGS: [Binding; 7] = [
    // LEFT
    Binding {
        keys: &[KeyboardKey::KEY_LEFT, KeyboardKey::KEY_A],
        buttons: &[GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_LEFT],
        axes: &[(GamepadAxis::GAMEPAD_AXIS_LEFT_X, -1)],
    },
    // RIGHT
    Binding {
        keys: &[KeyboardKey::KEY_RIGHT, KeyboardKey::KEY_D],
        buttons: &[GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_RIGHT],
        axes: &[(GamepadAxis::GAMEPAD_AXIS_LEFT_X, 1)],
    },
    // UP
    Binding {
        keys: &[KeyboardKey::KEY_UP, KeyboardKey::KEY_W],
        buttons: &[GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_UP],
        axes: &[(GamepadAxis::GAMEPAD_AXIS_LEFT_Y, -1)],
    },
    // DOWN
    Binding {
        keys: &[KeyboardKey::KEY_DOWN, KeyboardKey::KEY_S],
        buttons: &[GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_DOWN],
        axes: &[(GamepadAxis::GAMEPAD_AXIS_LEFT_Y, 1)],
    },
    // BTN1: Z or J on keyboard; south face button (Xbox A, PS Cross).
    Binding {
        keys: &[KeyboardKey::KEY_Z, KeyboardKey::KEY_J],
        buttons: &[GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN],
        axes: &[],
    },
    // BTN2: X or K on keyboard; east face button (Xbox B, PS Circle).
    Binding {
        keys: &[KeyboardKey::KEY_X, KeyboardKey::KEY_K],
        buttons: &[GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_RIGHT],
        axes: &[],
    },
    // BTN3: C or L on keyboard; north + west face buttons on gamepad
    // (Xbox Y/X, PS Triangle/Square). Both faces fire BTN3 because either
    // is much easier to hit than reaching across the diamond from A.
    Binding {
        keys: &[KeyboardKey::KEY_C, KeyboardKey::KEY_L],
        buttons: &[
            GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_UP,
            GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_LEFT,
        ],
        axes: &[],
    },
];

fn binding(action: u32) -> Option<&'static Binding> {
    BINDINGS.get(action.checked_sub(1)? as usize)
}

/// True if `action` is one of the exposed `ACTION_*` constants. Currently
/// only consumed by tests, but kept public for future runtime validation.
#[allow(dead_code)]
pub fn is_valid_action(action: u32) -> bool {
    binding(action).is_some()
}

/// True while any source bound to `action` is held.
pub fn action_down(rl: &RaylibHandle, action: u32) -> bool {
    let Some(b) = binding(action) else {
        return false;
    };
    for k in b.keys {
        if rl.is_key_down(*k) {
            return true;
        }
    }
    for pad in 0..MAX_GAMEPADS {
        if !rl.is_gamepad_available(pad) {
            continue;
        }
        for btn in b.buttons {
            if rl.is_gamepad_button_down(pad, *btn) {
                return true;
            }
        }
        for (axis, sign) in b.axes {
            let v = rl.get_gamepad_axis_movement(pad, *axis);
            if (*sign < 0 && v < -STICK_DEADZONE) || (*sign > 0 && v > STICK_DEADZONE) {
                return true;
            }
        }
    }
    false
}

/// True the frame any key or button bound to `action` transitions to
/// pressed. Analog sticks aren't edge-detected; if you want "just pushed
/// the stick past the deadzone" semantics, track the last frame yourself
/// using `action_down`.
pub fn action_pressed(rl: &RaylibHandle, action: u32) -> bool {
    let Some(b) = binding(action) else {
        return false;
    };
    for k in b.keys {
        if rl.is_key_pressed(*k) {
            return true;
        }
    }
    for pad in 0..MAX_GAMEPADS {
        if !rl.is_gamepad_available(pad) {
            continue;
        }
        for btn in b.buttons {
            if rl.is_gamepad_button_pressed(pad, *btn) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_known_actions_are_valid() {
        for a in [
            ACTION_LEFT,
            ACTION_RIGHT,
            ACTION_UP,
            ACTION_DOWN,
            ACTION_BTN1,
            ACTION_BTN2,
            ACTION_BTN3,
        ] {
            assert!(is_valid_action(a), "action {a} should be valid");
        }
    }

    #[test]
    fn unknown_actions_are_not_valid() {
        assert!(!is_valid_action(0));
        assert!(!is_valid_action(8));
        assert!(!is_valid_action(99));
        assert!(!is_valid_action(u32::MAX));
    }

    /// Each action should have at least one source bound, otherwise
    /// `action_down` / `action_pressed` can never be true.
    #[test]
    fn every_action_has_at_least_one_binding() {
        for (i, b) in BINDINGS.iter().enumerate() {
            assert!(
                !b.keys.is_empty() || !b.buttons.is_empty() || !b.axes.is_empty(),
                "action {} has no bindings",
                i + 1
            );
        }
    }
}
