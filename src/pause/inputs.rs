//! Input bundles for the pause menu.
//!
//! `MenuInputs` is the per-frame snapshot of the navigation actions
//! (up/down/left/right/btn1/btn2/toggle). `KeyConfigInputs` is the raw
//! key state the Key Config flow consumes (one captured key plus the
//! delete/backspace gestures).
//!
//! Splitting them out lets the parent `update_with` stay a pure
//! transition that takes already-bundled inputs, which is what makes
//! the integration tests in `pause.rs` cheap to write — they
//! construct these structs directly without any raylib handle.

use super::ACTION_COUNT;
use crate::input::{
    self, ACTION_BTN1, ACTION_BTN2, ACTION_DOWN, ACTION_LEFT, ACTION_RIGHT, ACTION_UP,
    AxisEdgeTracker, MAX_GAMEPADS,
};
use crate::keymap::Keymap;
use sola_raylib::prelude::*;

/// Per-frame navigation inputs for the menu. `toggle` (Esc / P /
/// gamepad Start) is the open/close key and is shared across every
/// view to consistently mean "go up one level." Enter is asymmetric:
/// it opens the menu like a toggle when closed, but inside the menu
/// it folds into `btn1` so it picks the highlighted option instead
/// of dismissing.
#[derive(Default, Clone, Copy)]
pub(super) struct MenuInputs {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub btn1: bool,
    pub btn2: bool,
    pub toggle: bool,
}

/// Raw key state for the Key Config capture/undo gestures. Bundled to
/// keep `update_with`'s arity reasonable.
#[derive(Default, Clone, Copy)]
pub(super) struct KeyConfigInputs {
    pub captured_key: Option<KeyboardKey>,
    pub delete: bool,
    pub backspace: bool,
}

/// Reads the navigation inputs once per frame. Face buttons are
/// per-pad family-aware inside `action_pressed`, so this works
/// correctly when multiple controllers of different families are
/// connected. Analog stick edge detection comes from `axes`.
///
/// `pause_open` routes Enter asymmetrically: when closed, Enter is a
/// toggle (opens the menu); when open, Enter folds into `btn1`
/// (selects the highlighted option) so a stray Enter doesn't dismiss
/// the menu without confirming. Alt+Enter is reserved for fullscreen
/// in either state.
pub(super) fn read_inputs(
    rl: &RaylibHandle,
    keymap: &Keymap,
    axes: &AxisEdgeTracker,
    pause_open: bool,
) -> MenuInputs {
    let alt_held =
        rl.is_key_down(KeyboardKey::KEY_LEFT_ALT) || rl.is_key_down(KeyboardKey::KEY_RIGHT_ALT);
    let enter = rl.is_key_pressed(KeyboardKey::KEY_ENTER) && !alt_held;
    let toggle = rl.is_key_pressed(KeyboardKey::KEY_ESCAPE)
        || rl.is_key_pressed(KeyboardKey::KEY_P)
        || gamepad_start_pressed(rl)
        || (enter && !pause_open);
    MenuInputs {
        up: input::action_pressed(rl, keymap, axes, ACTION_UP),
        down: input::action_pressed(rl, keymap, axes, ACTION_DOWN),
        left: input::action_pressed(rl, keymap, axes, ACTION_LEFT),
        right: input::action_pressed(rl, keymap, axes, ACTION_RIGHT),
        btn1: input::action_pressed(rl, keymap, axes, ACTION_BTN1) || (enter && pause_open),
        btn2: input::action_pressed(rl, keymap, axes, ACTION_BTN2),
        toggle,
    }
}

/// Snapshots which actions are currently held, for the Tester rects to
/// light up. Returned as a fixed-size array so `draw` can read it
/// without a raylib handle.
pub(super) fn snapshot_tester(rl: &RaylibHandle, keymap: &Keymap) -> [bool; ACTION_COUNT] {
    let mut out = [false; ACTION_COUNT];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = input::action_down(rl, keymap, (i + 1) as u32);
    }
    out
}

fn gamepad_start_pressed(rl: &RaylibHandle) -> bool {
    for pad in 0..MAX_GAMEPADS {
        if rl.is_gamepad_available(pad)
            && rl.is_gamepad_button_pressed(pad, GamepadButton::GAMEPAD_BUTTON_MIDDLE_RIGHT)
        {
            return true;
        }
    }
    false
}
