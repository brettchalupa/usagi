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
use crate::pad_map::{self, PadMap};
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
    /// True iff the gamepad Start button fired this frame. Also feeds
    /// into `toggle`; tracked separately so views that want a
    /// gamepad-specific Start behavior (Pad Config's reset gesture)
    /// can claim the press before the central toggle dispatcher does.
    pub start_press: bool,
}

/// Raw key state for the Key Config capture/undo gestures. Bundled to
/// keep `update_with`'s arity reasonable.
#[derive(Default, Clone, Copy)]
pub(super) struct KeyConfigInputs {
    pub captured_key: Option<KeyboardKey>,
    pub delete: bool,
    pub backspace: bool,
}

/// Raw gamepad state for the Pad Config capture/undo gestures. Mirror
/// of `KeyConfigInputs`. `delete` / `backspace` are read off the
/// keyboard for parity with Key Config so a player who already has
/// hands on the keyboard doesn't need to look up a gamepad combo, and
/// gamepad-only players can use Select for undo.
#[derive(Default, Clone, Copy)]
pub(super) struct PadConfigInputs {
    pub captured_button: Option<GamepadButton>,
    pub delete: bool,
    pub backspace: bool,
}

/// Capture-mode raw inputs grouped together. Both Key Config and Pad
/// Config feed their state in alongside the navigation inputs, so the
/// pure `update_with` transition takes one bundle instead of a tail of
/// positional args.
#[derive(Default, Clone, Copy)]
pub(super) struct CaptureInputs {
    pub kc: KeyConfigInputs,
    pub pc: PadConfigInputs,
}

/// Per-game override maps. Always flow together (input sampling, the
/// pause draw, the input tester all need both), so bundling them keeps
/// pause-menu signatures compact. Public so the session can construct
/// one to pass into `PauseMenu::update` / `::draw`.
#[derive(Copy, Clone)]
pub struct Maps<'a> {
    pub keymap: &'a crate::keymap::Keymap,
    pub pad_map: &'a crate::pad_map::PadMap,
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
    pad_map: &PadMap,
    axes: &AxisEdgeTracker,
    pause_open: bool,
) -> MenuInputs {
    let alt_held =
        rl.is_key_down(KeyboardKey::KEY_LEFT_ALT) || rl.is_key_down(KeyboardKey::KEY_RIGHT_ALT);
    let enter = rl.is_key_pressed(KeyboardKey::KEY_ENTER) && !alt_held;
    let start_press = gamepad_start_pressed(rl);
    let toggle = rl.is_key_pressed(KeyboardKey::KEY_ESCAPE)
        || rl.is_key_pressed(KeyboardKey::KEY_P)
        || start_press
        || (enter && !pause_open);
    MenuInputs {
        up: input::action_pressed(rl, keymap, pad_map, axes, ACTION_UP),
        down: input::action_pressed(rl, keymap, pad_map, axes, ACTION_DOWN),
        left: input::action_pressed(rl, keymap, pad_map, axes, ACTION_LEFT),
        right: input::action_pressed(rl, keymap, pad_map, axes, ACTION_RIGHT),
        btn1: input::action_pressed(rl, keymap, pad_map, axes, ACTION_BTN1)
            || (enter && pause_open),
        btn2: input::action_pressed(rl, keymap, pad_map, axes, ACTION_BTN2),
        toggle,
        start_press,
    }
}

/// Snapshots which actions are currently held, for the Tester rects to
/// light up. Returned as a fixed-size array so `draw` can read it
/// without a raylib handle.
pub(super) fn snapshot_tester(
    rl: &RaylibHandle,
    keymap: &Keymap,
    pad_map: &PadMap,
) -> [bool; ACTION_COUNT] {
    let mut out = [false; ACTION_COUNT];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = input::action_down(rl, keymap, pad_map, (i + 1) as u32);
    }
    out
}

/// Returns the first bindable gamepad button pressed this frame on any
/// connected pad, or `None`. "Bindable" = the set listed in
/// `pad_map::BINDABLE_BUTTONS`: 4 right-side face buttons + 4
/// shoulder/trigger positions. Dpad / Select / Start / Home / stick
/// clicks are deliberately not bindable so capture can't trap the
/// player in a menu they can't navigate out of.
pub(super) fn first_bindable_button_pressed(rl: &RaylibHandle) -> Option<GamepadButton> {
    for pad in 0..MAX_GAMEPADS {
        if !rl.is_gamepad_available(pad) {
            continue;
        }
        for btn in pad_map::BINDABLE_BUTTONS {
            if rl.is_gamepad_button_pressed(pad, *btn) {
                return Some(*btn);
            }
        }
    }
    None
}

/// True when any connected pad's Select / Back (MIDDLE_LEFT) was
/// pressed this frame. Used by Pad Config as the gamepad-side undo
/// gesture (keyboard equivalent: Backspace).
pub(super) fn gamepad_select_pressed(rl: &RaylibHandle) -> bool {
    for pad in 0..MAX_GAMEPADS {
        if rl.is_gamepad_available(pad)
            && rl.is_gamepad_button_pressed(pad, GamepadButton::GAMEPAD_BUTTON_MIDDLE_LEFT)
        {
            return true;
        }
    }
    false
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
