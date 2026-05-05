//! Abstract input actions. User Lua references actions via `input.LEFT`
//! etc. (integer IDs); at runtime each action is a union over keyboard
//! keys, gamepad buttons, and analog-stick directions. Adding a binding
//! only requires extending the `BINDINGS` table.
//!
//! Input is sampled into an `InputState` snapshot once per frame and
//! shared with the Lua side via `Rc<Cell<InputState>>`. That lets a
//! single closure registration cover every Lua callback (`_init`,
//! `_update`, `_draw`) without per-frame `lua.scope` rewiring, and
//! lets the closures sit alongside `gfx.*` in `_draw` without fighting
//! the `&mut RaylibHandle` borrow that `begin_texture_mode` holds.

use crate::keymap::Keymap;
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

// Mouse button IDs. Values match raylib's `MouseButton` enum so we can
// cast straight through; `setup_api` exposes them as `input.MOUSE_LEFT`
// / `input.MOUSE_RIGHT` on the Lua side.
pub const MOUSE_LEFT: u32 = 0;
pub const MOUSE_RIGHT: u32 = 1;

/// Public keyboard names and their raylib `KeyboardKey` values. Single
/// source of truth: `setup_api` registers each entry as `input.<name>`
/// on the Lua side using the raylib enum's i32 discriminant cast to
/// u32, and `InputState::sample` iterates this same table to fill the
/// per-frame bitmasks. Bit position = index into `KEY_TABLE`.
///
/// Only common keys are exposed; numpad, Insert/Home/End/PgUp/PgDn,
/// PrintScreen, etc. can be added on demand. Keep the length under 128
/// or widen the bitmask. `KEY_BACKTICK` is the friendlier alias for
/// raylib's `KEY_GRAVE` (the backtick/tilde key).
///
/// Note: this surface is the documented escape hatch for direct
/// keyboard reads (dev hotkeys, KB+M-only games). It deliberately
/// bypasses `Keymap` overrides and gamepad bindings; games that want
/// remappable cross-device input should use the abstract `input.LEFT`
/// / `input.BTN1` actions.
pub const KEY_TABLE: &[(&str, KeyboardKey)] = &[
    // Letters
    ("KEY_A", KeyboardKey::KEY_A),
    ("KEY_B", KeyboardKey::KEY_B),
    ("KEY_C", KeyboardKey::KEY_C),
    ("KEY_D", KeyboardKey::KEY_D),
    ("KEY_E", KeyboardKey::KEY_E),
    ("KEY_F", KeyboardKey::KEY_F),
    ("KEY_G", KeyboardKey::KEY_G),
    ("KEY_H", KeyboardKey::KEY_H),
    ("KEY_I", KeyboardKey::KEY_I),
    ("KEY_J", KeyboardKey::KEY_J),
    ("KEY_K", KeyboardKey::KEY_K),
    ("KEY_L", KeyboardKey::KEY_L),
    ("KEY_M", KeyboardKey::KEY_M),
    ("KEY_N", KeyboardKey::KEY_N),
    ("KEY_O", KeyboardKey::KEY_O),
    ("KEY_P", KeyboardKey::KEY_P),
    ("KEY_Q", KeyboardKey::KEY_Q),
    ("KEY_R", KeyboardKey::KEY_R),
    ("KEY_S", KeyboardKey::KEY_S),
    ("KEY_T", KeyboardKey::KEY_T),
    ("KEY_U", KeyboardKey::KEY_U),
    ("KEY_V", KeyboardKey::KEY_V),
    ("KEY_W", KeyboardKey::KEY_W),
    ("KEY_X", KeyboardKey::KEY_X),
    ("KEY_Y", KeyboardKey::KEY_Y),
    ("KEY_Z", KeyboardKey::KEY_Z),
    // Digits (top row)
    ("KEY_0", KeyboardKey::KEY_ZERO),
    ("KEY_1", KeyboardKey::KEY_ONE),
    ("KEY_2", KeyboardKey::KEY_TWO),
    ("KEY_3", KeyboardKey::KEY_THREE),
    ("KEY_4", KeyboardKey::KEY_FOUR),
    ("KEY_5", KeyboardKey::KEY_FIVE),
    ("KEY_6", KeyboardKey::KEY_SIX),
    ("KEY_7", KeyboardKey::KEY_SEVEN),
    ("KEY_8", KeyboardKey::KEY_EIGHT),
    ("KEY_9", KeyboardKey::KEY_NINE),
    // Function row
    ("KEY_F1", KeyboardKey::KEY_F1),
    ("KEY_F2", KeyboardKey::KEY_F2),
    ("KEY_F3", KeyboardKey::KEY_F3),
    ("KEY_F4", KeyboardKey::KEY_F4),
    ("KEY_F5", KeyboardKey::KEY_F5),
    ("KEY_F6", KeyboardKey::KEY_F6),
    ("KEY_F7", KeyboardKey::KEY_F7),
    ("KEY_F8", KeyboardKey::KEY_F8),
    ("KEY_F9", KeyboardKey::KEY_F9),
    ("KEY_F10", KeyboardKey::KEY_F10),
    ("KEY_F11", KeyboardKey::KEY_F11),
    ("KEY_F12", KeyboardKey::KEY_F12),
    // Specials
    ("KEY_SPACE", KeyboardKey::KEY_SPACE),
    ("KEY_ENTER", KeyboardKey::KEY_ENTER),
    ("KEY_ESCAPE", KeyboardKey::KEY_ESCAPE),
    ("KEY_TAB", KeyboardKey::KEY_TAB),
    ("KEY_BACKSPACE", KeyboardKey::KEY_BACKSPACE),
    ("KEY_DELETE", KeyboardKey::KEY_DELETE),
    // Arrows (prefixed so they don't collide with abstract input.LEFT etc.)
    ("KEY_LEFT", KeyboardKey::KEY_LEFT),
    ("KEY_RIGHT", KeyboardKey::KEY_RIGHT),
    ("KEY_UP", KeyboardKey::KEY_UP),
    ("KEY_DOWN", KeyboardKey::KEY_DOWN),
    // Modifiers (short names: LSHIFT, not LEFT_SHIFT)
    ("KEY_LSHIFT", KeyboardKey::KEY_LEFT_SHIFT),
    ("KEY_RSHIFT", KeyboardKey::KEY_RIGHT_SHIFT),
    ("KEY_LCTRL", KeyboardKey::KEY_LEFT_CONTROL),
    ("KEY_RCTRL", KeyboardKey::KEY_RIGHT_CONTROL),
    ("KEY_LALT", KeyboardKey::KEY_LEFT_ALT),
    ("KEY_RALT", KeyboardKey::KEY_RIGHT_ALT),
    // Punctuation. KEY_BACKTICK aliases raylib's KEY_GRAVE; nobody
    // outside Unicode docs calls it grave.
    ("KEY_BACKTICK", KeyboardKey::KEY_GRAVE),
    ("KEY_MINUS", KeyboardKey::KEY_MINUS),
    ("KEY_EQUAL", KeyboardKey::KEY_EQUAL),
    ("KEY_LBRACKET", KeyboardKey::KEY_LEFT_BRACKET),
    ("KEY_RBRACKET", KeyboardKey::KEY_RIGHT_BRACKET),
    ("KEY_BACKSLASH", KeyboardKey::KEY_BACKSLASH),
    ("KEY_SEMICOLON", KeyboardKey::KEY_SEMICOLON),
    ("KEY_APOSTROPHE", KeyboardKey::KEY_APOSTROPHE),
    ("KEY_COMMA", KeyboardKey::KEY_COMMA),
    ("KEY_PERIOD", KeyboardKey::KEY_PERIOD),
    ("KEY_SLASH", KeyboardKey::KEY_SLASH),
];

/// Lookup the bit index for a given Lua-side key value (raylib enum's
/// i32 discriminant cast to u32). Returns `None` for any value that
/// isn't in `KEY_TABLE`, which keeps `input.key_*` calls with bogus
/// arguments cheap and false rather than panicking.
fn key_bit_index(value: u32) -> Option<usize> {
    KEY_TABLE
        .iter()
        .position(|(_, k)| (*k as i32 as u32) == value)
}

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
        buttons: &[
            GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN,
            GamepadButton::GAMEPAD_BUTTON_LEFT_TRIGGER_1,
        ],
        axes: &[],
    },
    // BTN2: X or K on keyboard; east face button (Xbox B, PS Circle).
    Binding {
        keys: &[KeyboardKey::KEY_X, KeyboardKey::KEY_K],
        buttons: &[
            GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_RIGHT,
            GamepadButton::GAMEPAD_BUTTON_RIGHT_TRIGGER_1,
        ],
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

// Nintendo's UX swaps which face button is "primary": A is east, B is
// south (vs Xbox/PS where the primary lands on south). To make
// BTN1=primary and BTN2=cancel feel native on Switch, swap south/east
// for those two actions. Triggers (LB/RB) and BTN3 stay put.
const SWITCH_BTN1_BUTTONS: [GamepadButton; 2] = [
    GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_RIGHT,
    GamepadButton::GAMEPAD_BUTTON_LEFT_TRIGGER_1,
];
const SWITCH_BTN2_BUTTONS: [GamepadButton; 2] = [
    GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN,
    GamepadButton::GAMEPAD_BUTTON_RIGHT_TRIGGER_1,
];

/// Gamepad face buttons that fire `action` for the given family. Same
/// as the static `BINDINGS[i].buttons` for everyone except Switch on
/// BTN1/BTN2, where south/east are swapped to match Nintendo's
/// "A=primary, B=cancel" convention.
fn effective_face_buttons(action: u32, family: GamepadFamily) -> &'static [GamepadButton] {
    use GamepadFamily::Nintendo;
    match (action, family) {
        (ACTION_BTN1, Nintendo) => &SWITCH_BTN1_BUTTONS,
        (ACTION_BTN2, Nintendo) => &SWITCH_BTN2_BUTTONS,
        _ => match binding(action) {
            Some(b) => b.buttons,
            None => &[],
        },
    }
}

/// Display names for `ACTION_*`, indexed by `action - 1`. Used by the
/// pause menu's Input views.
pub const ACTION_NAMES: [&str; 7] = ["LEFT", "RIGHT", "UP", "DOWN", "BTN1", "BTN2", "BTN3"];

fn key_label(k: KeyboardKey) -> &'static str {
    match k {
        KeyboardKey::KEY_LEFT => "Left",
        KeyboardKey::KEY_RIGHT => "Right",
        KeyboardKey::KEY_UP => "Up",
        KeyboardKey::KEY_DOWN => "Down",
        KeyboardKey::KEY_A => "A",
        KeyboardKey::KEY_D => "D",
        KeyboardKey::KEY_W => "W",
        KeyboardKey::KEY_S => "S",
        KeyboardKey::KEY_Z => "Z",
        KeyboardKey::KEY_X => "X",
        KeyboardKey::KEY_C => "C",
        KeyboardKey::KEY_J => "J",
        KeyboardKey::KEY_K => "K",
        KeyboardKey::KEY_L => "L",
        _ => "?",
    }
}

/// Face-button glyph family. Detected from the connected gamepad's
/// name; falls back to Xbox for unknown / generic / Steam Deck pads.
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
pub enum GamepadFamily {
    #[default]
    Xbox,
    PlayStation,
    Nintendo,
}

impl GamepadFamily {
    /// Substring-matches the gamepad name from `GetGamepadName`. Names
    /// vary by OS and driver, so the matchers are intentionally loose.
    pub fn detect(name: &str) -> Self {
        let n = name.to_ascii_lowercase();
        if n.contains("playstation")
            || n.contains("dualshock")
            || n.contains("dualsense")
            || n.contains("sony")
            || n.starts_with("ps")
        {
            return GamepadFamily::PlayStation;
        }
        if n.contains("switch")
            || n.contains("nintendo")
            || n.contains("joy-con")
            || n.contains("pro controller")
        {
            return GamepadFamily::Nintendo;
        }
        GamepadFamily::Xbox
    }
}

/// First connected gamepad's family. Multiple pads are rare in
/// practice; pick slot 0 (or the lowest connected slot) so the glyphs
/// stay stable as long as a single pad is in use.
pub fn current_gamepad_family(rl: &RaylibHandle) -> GamepadFamily {
    for pad in 0..MAX_GAMEPADS {
        if rl.is_gamepad_available(pad)
            && let Some(name) = rl.get_gamepad_name(pad)
        {
            return GamepadFamily::detect(&name);
        }
    }
    GamepadFamily::default()
}

/// Tracks per-slot gamepad names across frames so the session can log
/// connect / disconnect / hot-swap events exactly once. Connection
/// events include the raw name and the detected family, which is
/// what you want when a controller's face buttons feel wrong: the
/// name is the only knob `GamepadFamily::detect` reads.
///
/// Cheap to call every frame: the inner state is `MAX_GAMEPADS`
/// `Option<String>`s and the comparison is a string equality test
/// per slot.
pub struct GamepadProbe {
    last_seen: [Option<String>; MAX_GAMEPADS as usize],
}

impl GamepadProbe {
    pub fn new() -> Self {
        Self {
            last_seen: std::array::from_fn(|_| None),
        }
    }

    /// Polls every slot and emits one log line per state change.
    pub fn poll(&mut self, rl: &RaylibHandle) {
        for pad in 0..MAX_GAMEPADS {
            let idx = pad as usize;
            let current: Option<String> = if rl.is_gamepad_available(pad) {
                rl.get_gamepad_name(pad)
            } else {
                None
            };
            match (self.last_seen[idx].as_deref(), current.as_deref()) {
                (None, Some(name)) => {
                    let family = GamepadFamily::detect(name);
                    crate::msg::info!("gamepad {pad} connected: '{name}' (family: {family:?})");
                }
                (Some(prev), Some(name)) if prev != name => {
                    let family = GamepadFamily::detect(name);
                    crate::msg::info!("gamepad {pad} changed: '{name}' (family: {family:?})");
                }
                (Some(prev), None) => {
                    crate::msg::info!("gamepad {pad} disconnected: '{prev}'");
                }
                _ => {}
            }
            self.last_seen[idx] = current;
        }
    }
}

impl Default for GamepadProbe {
    fn default() -> Self {
        Self::new()
    }
}

fn button_label(b: GamepadButton, family: GamepadFamily) -> &'static str {
    use GamepadButton::*;
    use GamepadFamily::*;
    // Face buttons differ by family. South face is always at the
    // bottom of the diamond but its label is A on Xbox, Cross on
    // PlayStation, B on Switch (Nintendo mirrors A/B/X/Y vs Xbox).
    match (b, family) {
        (GAMEPAD_BUTTON_RIGHT_FACE_DOWN, Xbox) => "A",
        (GAMEPAD_BUTTON_RIGHT_FACE_DOWN, PlayStation) => "Cross",
        (GAMEPAD_BUTTON_RIGHT_FACE_DOWN, Nintendo) => "B",
        (GAMEPAD_BUTTON_RIGHT_FACE_RIGHT, Xbox) => "B",
        (GAMEPAD_BUTTON_RIGHT_FACE_RIGHT, PlayStation) => "Circle",
        (GAMEPAD_BUTTON_RIGHT_FACE_RIGHT, Nintendo) => "A",
        (GAMEPAD_BUTTON_RIGHT_FACE_UP, Xbox) => "Y",
        (GAMEPAD_BUTTON_RIGHT_FACE_UP, PlayStation) => "Triangle",
        (GAMEPAD_BUTTON_RIGHT_FACE_UP, Nintendo) => "X",
        (GAMEPAD_BUTTON_RIGHT_FACE_LEFT, Xbox) => "X",
        (GAMEPAD_BUTTON_RIGHT_FACE_LEFT, PlayStation) => "Square",
        (GAMEPAD_BUTTON_RIGHT_FACE_LEFT, Nintendo) => "Y",
        (GAMEPAD_BUTTON_LEFT_FACE_LEFT, _) => "Left",
        (GAMEPAD_BUTTON_LEFT_FACE_RIGHT, _) => "Right",
        (GAMEPAD_BUTTON_LEFT_FACE_UP, _) => "Up",
        (GAMEPAD_BUTTON_LEFT_FACE_DOWN, _) => "Down",
        (GAMEPAD_BUTTON_LEFT_TRIGGER_1, Xbox) => "LB",
        (GAMEPAD_BUTTON_LEFT_TRIGGER_1, PlayStation) => "L1",
        (GAMEPAD_BUTTON_LEFT_TRIGGER_1, Nintendo) => "L",
        (GAMEPAD_BUTTON_RIGHT_TRIGGER_1, Xbox) => "RB",
        (GAMEPAD_BUTTON_RIGHT_TRIGGER_1, PlayStation) => "R1",
        (GAMEPAD_BUTTON_RIGHT_TRIGGER_1, Nintendo) => "R",
        _ => "?",
    }
}

fn axis_label(axis: GamepadAxis, sign: i8) -> &'static str {
    match (axis, sign.signum()) {
        (GamepadAxis::GAMEPAD_AXIS_LEFT_X, -1) => "Left",
        (GamepadAxis::GAMEPAD_AXIS_LEFT_X, 1) => "Right",
        (GamepadAxis::GAMEPAD_AXIS_LEFT_Y, -1) => "Up",
        (GamepadAxis::GAMEPAD_AXIS_LEFT_Y, 1) => "Down",
        _ => "?",
    }
}

/// Per-action binding split into keyboard and gamepad strings for the
/// pause menu's Input Test table. Override-replaces-keyboard mirrors
/// `action_pressed`.
pub fn binding_columns(
    keymap: &Keymap,
    family: GamepadFamily,
) -> [(&'static str, String, String); 7] {
    let mut out: [(&'static str, String, String); 7] =
        std::array::from_fn(|i| (ACTION_NAMES[i], String::new(), String::new()));
    for (i, b) in BINDINGS.iter().enumerate() {
        let action = (i + 1) as u32;
        let kb = match keymap.override_for(action) {
            Some(k) => key_label(k).to_string(),
            None => b
                .keys
                .iter()
                .map(|k| key_label(*k).to_string())
                .collect::<Vec<_>>()
                .join(", "),
        };
        let mut gp = Vec::new();
        for btn in effective_face_buttons(action, family) {
            gp.push(button_label(*btn, family).to_string());
        }
        for (axis, sign) in b.axes {
            gp.push(axis_label(*axis, *sign).to_string());
        }
        out[i].1 = kb;
        out[i].2 = gp.join(", ");
    }
    out
}

/// Which input source most recently fired any bound action. Drives
/// `mapping_for` so games can render the right glyph based on what
/// the player just used. Stored on `InputState` and refreshed each
/// frame in `sample`.
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
pub enum InputSource {
    #[default]
    Keyboard,
    Gamepad,
}

impl InputSource {
    /// Lowercase name exposed to Lua as `input.last_source()` and the
    /// `input.SOURCE_*` constants.
    pub fn as_str(self) -> &'static str {
        match self {
            InputSource::Keyboard => "keyboard",
            InputSource::Gamepad => "gamepad",
        }
    }
}

/// Label of the primary binding for `action` on the given input source,
/// honoring `keymap` overrides and the suppress-overridden-defaults
/// rule. Returns `None` for unknown actions or when no source-side
/// binding exists (e.g. every keyboard default for the action has been
/// claimed as another action's override).
pub fn mapping_for(
    action: u32,
    keymap: &Keymap,
    source: InputSource,
    family: GamepadFamily,
) -> Option<&'static str> {
    let b = binding(action)?;
    match source {
        InputSource::Keyboard => {
            if let Some(k) = keymap.override_for(action) {
                return Some(key_label(k));
            }
            for k in b.keys {
                if !keymap.is_used_as_override(*k) {
                    return Some(key_label(*k));
                }
            }
            None
        }
        InputSource::Gamepad => {
            let buttons = effective_face_buttons(action, family);
            if let Some(btn) = buttons.first() {
                return Some(button_label(*btn, family));
            }
            b.axes.first().map(|(axis, sign)| axis_label(*axis, *sign))
        }
    }
}

/// Returns the input source that fired any bound action this frame
/// and, for gamepad sources, the specific pad slot that fired. Gamepad
/// wins ties since gamepads only emit deliberate input. If neither
/// side fired anything bound, `(prior, None)` is returned so a stray
/// Esc or F-key press can't flip the indicator.
///
/// The pad slot is what lets `mapping_for` pick the right family for
/// glyphs when multiple controllers of different families are
/// connected: pressing X on a DualShock at slot 1 should label as
/// "Cross" regardless of what's at slot 0.
pub fn detect_source(
    rl: &RaylibHandle,
    keymap: &Keymap,
    prior: InputSource,
) -> (InputSource, Option<i32>) {
    for b in BINDINGS.iter() {
        for pad in 0..MAX_GAMEPADS {
            if !rl.is_gamepad_available(pad) {
                continue;
            }
            for btn in b.buttons {
                if rl.is_gamepad_button_down(pad, *btn) {
                    return (InputSource::Gamepad, Some(pad));
                }
            }
            for (axis, sign) in b.axes {
                let v = rl.get_gamepad_axis_movement(pad, *axis);
                if (*sign < 0 && v < -STICK_DEADZONE) || (*sign > 0 && v > STICK_DEADZONE) {
                    return (InputSource::Gamepad, Some(pad));
                }
            }
        }
    }
    for (i, b) in BINDINGS.iter().enumerate() {
        let action = (i + 1) as u32;
        if let Some(k) = keymap.override_for(action) {
            if rl.is_key_down(k) {
                return (InputSource::Keyboard, None);
            }
        } else {
            for k in b.keys {
                if keymap.is_used_as_override(*k) {
                    continue;
                }
                if rl.is_key_down(*k) {
                    return (InputSource::Keyboard, None);
                }
            }
        }
    }
    (prior, None)
}

/// True if `action` is one of the exposed `ACTION_*` constants. Currently
/// only consumed by tests, but kept public for future runtime validation.
#[allow(dead_code)]
pub fn is_valid_action(action: u32) -> bool {
    binding(action).is_some()
}

/// True while any source bound to `action` is held. An override
/// replaces this action's keyboard defaults (Pico-8 "replace"
/// semantics); a default key claimed as another action's override is
/// suppressed so a key only fires its current owner. Gamepad face
/// buttons use the per-pad family (so a Switch Pro at slot 2 fires
/// BTN1 from its A button, even if an Xbox-style pad is in slot 0).
pub fn action_down(rl: &RaylibHandle, keymap: &Keymap, action: u32) -> bool {
    let Some(b) = binding(action) else {
        return false;
    };
    if let Some(k) = keymap.override_for(action) {
        if rl.is_key_down(k) {
            return true;
        }
    } else {
        for k in b.keys {
            if keymap.is_used_as_override(*k) {
                continue;
            }
            if rl.is_key_down(*k) {
                return true;
            }
        }
    }
    for pad in 0..MAX_GAMEPADS {
        if !rl.is_gamepad_available(pad) {
            continue;
        }
        let buttons = effective_face_buttons(action, pad_family(rl, pad));
        for btn in buttons {
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
/// using `action_down`. Override semantics match `action_down`. Face
/// buttons use the per-pad family.
pub fn action_pressed(rl: &RaylibHandle, keymap: &Keymap, action: u32) -> bool {
    let Some(b) = binding(action) else {
        return false;
    };
    if let Some(k) = keymap.override_for(action) {
        if rl.is_key_pressed(k) {
            return true;
        }
    } else {
        for k in b.keys {
            if keymap.is_used_as_override(*k) {
                continue;
            }
            if rl.is_key_pressed(*k) {
                return true;
            }
        }
    }
    for pad in 0..MAX_GAMEPADS {
        if !rl.is_gamepad_available(pad) {
            continue;
        }
        let buttons = effective_face_buttons(action, pad_family(rl, pad));
        for btn in buttons {
            if rl.is_gamepad_button_pressed(pad, *btn) {
                return true;
            }
        }
    }
    false
}

/// True the frame any key or button bound to `action` transitions from
/// down to up. Analog sticks aren't edge-detected. Override semantics
/// match `action_down`. Face buttons use the per-pad family.
pub fn action_released(rl: &RaylibHandle, keymap: &Keymap, action: u32) -> bool {
    let Some(b) = binding(action) else {
        return false;
    };
    if let Some(k) = keymap.override_for(action) {
        if rl.is_key_released(k) {
            return true;
        }
    } else {
        for k in b.keys {
            if keymap.is_used_as_override(*k) {
                continue;
            }
            if rl.is_key_released(*k) {
                return true;
            }
        }
    }
    for pad in 0..MAX_GAMEPADS {
        if !rl.is_gamepad_available(pad) {
            continue;
        }
        let buttons = effective_face_buttons(action, pad_family(rl, pad));
        for btn in buttons {
            if rl.is_gamepad_button_released(pad, *btn) {
                return true;
            }
        }
    }
    false
}

/// Resolves the family of a single pad slot. Used by `action_down` /
/// `action_pressed` / `action_released` so each connected pad is
/// matched against its own family's face button layout. Falls back to
/// `GamepadFamily::default` for slots whose name can't be read.
fn pad_family(rl: &RaylibHandle, pad: i32) -> GamepadFamily {
    rl.get_gamepad_name(pad)
        .map(|n| GamepadFamily::detect(&n))
        .unwrap_or_default()
}

/// Inverts the screen-to-game render transform so a window-pixel mouse
/// position becomes game-pixel coords. Pure (no raylib handle), so tests
/// can exercise the math directly. May return values outside
/// `0..game.w` / `0..game.h` when the cursor is over the letterbox
/// bars; games can detect that with a simple bounds check.
pub fn screen_to_game(
    mouse_x: f32,
    mouse_y: f32,
    screen_w: i32,
    screen_h: i32,
    res: crate::config::Resolution,
    pixel_perfect: bool,
) -> (i32, i32) {
    let (scale, ox, oy) =
        crate::render::game_view_transform(screen_w, screen_h, res, pixel_perfect);
    let gx = ((mouse_x - ox) / scale).floor() as i32;
    let gy = ((mouse_y - oy) / scale).floor() as i32;
    (gx, gy)
}

fn mouse_button_from_u32(button: u32) -> Option<MouseButton> {
    match button {
        MOUSE_LEFT => Some(MouseButton::MOUSE_BUTTON_LEFT),
        MOUSE_RIGHT => Some(MouseButton::MOUSE_BUTTON_RIGHT),
        _ => None,
    }
}

/// Toggles the OS cursor's visibility. Called by the session at frame
/// start when a Lua-side `input.set_mouse_visible` is pending; the
/// closure itself just records the request into a `Cell` so it can be
/// applied here, where `&mut RaylibHandle` is freely available outside
/// of any `begin_texture_mode` borrow.
pub fn set_mouse_visible(rl: &mut RaylibHandle, visible: bool) {
    if visible {
        rl.show_cursor();
    } else {
        rl.hide_cursor();
    }
}

/// One frame's worth of input state, sampled from raylib. `Copy` so the
/// session can stash the latest snapshot in a `Cell<InputState>` and
/// the Lua closures can read whole snapshots cheaply on each call. The
/// action fields are bitmasks indexed by `action - 1`.
#[derive(Default, Copy, Clone)]
pub struct InputState {
    actions_down: u32,
    actions_pressed: u32,
    actions_released: u32,
    /// Bitmasks indexed by position in `KEY_TABLE`. u128 is enough for
    /// 75-ish keys; widen if `KEY_TABLE` ever grows past 128 entries.
    keys_held: u128,
    keys_pressed: u128,
    keys_released: u128,
    mouse_left_down: bool,
    mouse_right_down: bool,
    mouse_left_pressed: bool,
    mouse_right_pressed: bool,
    mouse_left_released: bool,
    mouse_right_released: bool,
    mouse_x: i32,
    mouse_y: i32,
    /// Per-action label for the currently-active source's primary
    /// binding. Indexed by `action - 1`. Pre-computed in `sample` so
    /// the `input.mapping_for` Lua closure stays cheap.
    mapping: [Option<&'static str>; 7],
    last_source: InputSource,
    /// Slot of the most recently active gamepad, carried forward across
    /// frames so glyphs stay stable when no input fires. `None` until
    /// any gamepad has fired this session. Drives `gamepad_family` for
    /// the precomputed `mapping` so multi-pad setups label correctly.
    last_pad: Option<i32>,
    gamepad_family: GamepadFamily,
}

impl InputState {
    /// Polls raylib once and rolls the result into a snapshot. Called
    /// at the top of each frame, before user Lua runs. `prior_source`
    /// and `prior_pad` carry forward when no bound input fired this
    /// frame, so glyphs stay stable across idle moments.
    pub fn sample(
        rl: &RaylibHandle,
        res: crate::config::Resolution,
        pixel_perfect: bool,
        keymap: &Keymap,
        prior_source: InputSource,
        prior_pad: Option<i32>,
    ) -> Self {
        let mut down = 0u32;
        let mut pressed = 0u32;
        let mut released = 0u32;
        for (i, _) in BINDINGS.iter().enumerate() {
            let action = (i + 1) as u32;
            if action_down(rl, keymap, action) {
                down |= 1 << i;
            }
            if action_pressed(rl, keymap, action) {
                pressed |= 1 << i;
            }
            if action_released(rl, keymap, action) {
                released |= 1 << i;
            }
        }
        let m = rl.get_mouse_position();
        let sw = rl.get_screen_width();
        let sh = rl.get_screen_height();
        let (mx, my) = screen_to_game(m.x, m.y, sw, sh, res, pixel_perfect);
        let (last_source, fired_pad) = detect_source(rl, keymap, prior_source);
        // Carry the slot forward when nothing fired so a stretch of
        // idle frames doesn't wipe the glyph identity.
        let last_pad = fired_pad.or(prior_pad);
        // Family follows whichever pad was last actively used. Falls
        // back to slot 0's family (or default) before any pad has
        // fired so glyphs render reasonably on first frame.
        let gamepad_family = match last_pad {
            Some(pad) => pad_family(rl, pad),
            None => current_gamepad_family(rl),
        };
        let mapping = std::array::from_fn(|i| {
            mapping_for((i + 1) as u32, keymap, last_source, gamepad_family)
        });
        let mut keys_held = 0u128;
        let mut keys_pressed = 0u128;
        let mut keys_released = 0u128;
        for (i, (_, key)) in KEY_TABLE.iter().enumerate() {
            let bit = 1u128 << i;
            if rl.is_key_down(*key) {
                keys_held |= bit;
            }
            if rl.is_key_pressed(*key) {
                keys_pressed |= bit;
            }
            if rl.is_key_released(*key) {
                keys_released |= bit;
            }
        }
        Self {
            actions_down: down,
            actions_pressed: pressed,
            actions_released: released,
            keys_held,
            keys_pressed,
            keys_released,
            mouse_left_down: rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT),
            mouse_right_down: rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_RIGHT),
            mouse_left_pressed: rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT),
            mouse_right_pressed: rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_RIGHT),
            mouse_left_released: rl.is_mouse_button_released(MouseButton::MOUSE_BUTTON_LEFT),
            mouse_right_released: rl.is_mouse_button_released(MouseButton::MOUSE_BUTTON_RIGHT),
            mouse_x: mx,
            mouse_y: my,
            mapping,
            last_source,
            last_pad,
            gamepad_family,
        }
    }

    pub fn mapping_for(&self, action: u32) -> Option<&'static str> {
        let i = action.checked_sub(1)? as usize;
        self.mapping.get(i).copied().flatten()
    }

    pub fn last_source(&self) -> InputSource {
        self.last_source
    }

    pub fn last_pad(&self) -> Option<i32> {
        self.last_pad
    }

    pub fn gamepad_family(&self) -> GamepadFamily {
        self.gamepad_family
    }

    pub fn action_down(&self, action: u32) -> bool {
        action
            .checked_sub(1)
            .filter(|i| (*i as usize) < BINDINGS.len())
            .map(|i| self.actions_down & (1 << i) != 0)
            .unwrap_or(false)
    }

    pub fn action_pressed(&self, action: u32) -> bool {
        action
            .checked_sub(1)
            .filter(|i| (*i as usize) < BINDINGS.len())
            .map(|i| self.actions_pressed & (1 << i) != 0)
            .unwrap_or(false)
    }

    pub fn action_released(&self, action: u32) -> bool {
        action
            .checked_sub(1)
            .filter(|i| (*i as usize) < BINDINGS.len())
            .map(|i| self.actions_released & (1 << i) != 0)
            .unwrap_or(false)
    }

    pub fn mouse_button_down(&self, button: u32) -> bool {
        match mouse_button_from_u32(button) {
            Some(MouseButton::MOUSE_BUTTON_LEFT) => self.mouse_left_down,
            Some(MouseButton::MOUSE_BUTTON_RIGHT) => self.mouse_right_down,
            _ => false,
        }
    }

    pub fn mouse_button_pressed(&self, button: u32) -> bool {
        match mouse_button_from_u32(button) {
            Some(MouseButton::MOUSE_BUTTON_LEFT) => self.mouse_left_pressed,
            Some(MouseButton::MOUSE_BUTTON_RIGHT) => self.mouse_right_pressed,
            _ => false,
        }
    }

    pub fn mouse_button_released(&self, button: u32) -> bool {
        match mouse_button_from_u32(button) {
            Some(MouseButton::MOUSE_BUTTON_LEFT) => self.mouse_left_released,
            Some(MouseButton::MOUSE_BUTTON_RIGHT) => self.mouse_right_released,
            _ => false,
        }
    }

    pub fn key_held(&self, key: u32) -> bool {
        key_bit_index(key)
            .map(|i| self.keys_held & (1u128 << i) != 0)
            .unwrap_or(false)
    }

    pub fn key_pressed(&self, key: u32) -> bool {
        key_bit_index(key)
            .map(|i| self.keys_pressed & (1u128 << i) != 0)
            .unwrap_or(false)
    }

    pub fn key_released(&self, key: u32) -> bool {
        key_bit_index(key)
            .map(|i| self.keys_released & (1u128 << i) != 0)
            .unwrap_or(false)
    }

    pub fn mouse_position(&self) -> (i32, i32) {
        (self.mouse_x, self.mouse_y)
    }
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

    #[test]
    fn binding_columns_cover_every_action_with_filled_keyboard_and_gamepad() {
        let keymap = Keymap::default();
        let cols = binding_columns(&keymap, GamepadFamily::Xbox);
        assert_eq!(cols.len(), BINDINGS.len());
        for (i, (name, kb, gp)) in cols.iter().enumerate() {
            assert_eq!(*name, ACTION_NAMES[i]);
            assert!(
                !kb.is_empty(),
                "action {name} (#{n}) keyboard column was empty",
                name = name,
                n = i + 1
            );
            assert!(
                !kb.contains("?"),
                "action {name} keyboard column has unknown source: {kb:?}",
                name = name,
                kb = kb
            );
            assert!(
                !gp.is_empty(),
                "action {name} (#{n}) gamepad column was empty",
                name = name,
                n = i + 1
            );
        }
    }

    #[test]
    fn binding_columns_swaps_keyboard_portion_for_override() {
        let mut keymap = Keymap::default();
        keymap.overrides[ACTION_LEFT as usize - 1] = Some(KeyboardKey::KEY_W);
        let cols = binding_columns(&keymap, GamepadFamily::Xbox);
        let (_, kb, gp) = &cols[ACTION_LEFT as usize - 1];
        assert_eq!(kb, "W");
        assert!(!gp.contains('W'));
        assert!(
            gp.contains("Left"),
            "gamepad column must survive keyboard override: {gp}"
        );
    }

    #[test]
    fn mapping_for_keyboard_returns_override_or_first_default() {
        let mut keymap = Keymap::default();
        let xbox = GamepadFamily::Xbox;
        assert_eq!(
            mapping_for(ACTION_LEFT, &keymap, InputSource::Keyboard, xbox),
            Some("Left"),
        );
        keymap.overrides[ACTION_LEFT as usize - 1] = Some(KeyboardKey::KEY_W);
        assert_eq!(
            mapping_for(ACTION_LEFT, &keymap, InputSource::Keyboard, xbox),
            Some("W"),
        );
        // Unknown action.
        assert_eq!(mapping_for(99, &keymap, InputSource::Keyboard, xbox), None,);
    }

    #[test]
    fn mapping_for_keyboard_skips_defaults_used_as_overrides_elsewhere() {
        // RIGHT remapped to Left arrow. LEFT now exposes A as its
        // canonical key because the Left arrow has been claimed.
        let mut keymap = Keymap::default();
        let xbox = GamepadFamily::Xbox;
        keymap.overrides[ACTION_RIGHT as usize - 1] = Some(KeyboardKey::KEY_LEFT);
        assert_eq!(
            mapping_for(ACTION_LEFT, &keymap, InputSource::Keyboard, xbox),
            Some("A"),
        );
        assert_eq!(
            mapping_for(ACTION_RIGHT, &keymap, InputSource::Keyboard, xbox),
            Some("Left"),
        );
    }

    #[test]
    fn mapping_for_gamepad_returns_first_button_or_axis_per_family() {
        let keymap = Keymap::default();
        // Xbox: BTN1 south face = "A".
        assert_eq!(
            mapping_for(
                ACTION_BTN1,
                &keymap,
                InputSource::Gamepad,
                GamepadFamily::Xbox
            ),
            Some("A"),
        );
        // PlayStation: south face = "Cross".
        assert_eq!(
            mapping_for(
                ACTION_BTN1,
                &keymap,
                InputSource::Gamepad,
                GamepadFamily::PlayStation,
            ),
            Some("Cross"),
        );
        // Directional actions have no buttons in BINDINGS; they fall
        // through to the first dpad entry, family-agnostic.
        assert_eq!(
            mapping_for(
                ACTION_LEFT,
                &keymap,
                InputSource::Gamepad,
                GamepadFamily::PlayStation
            ),
            Some("Left"),
        );
    }

    #[test]
    fn nintendo_swaps_btn1_and_btn2_face_buttons() {
        // Switch convention: A (east) = primary, B (south) = cancel.
        // Usagi swaps so BTN1 fires from A and BTN2 from B, matching
        // every native Switch game.
        let keymap = Keymap::default();
        assert_eq!(
            mapping_for(
                ACTION_BTN1,
                &keymap,
                InputSource::Gamepad,
                GamepadFamily::Nintendo,
            ),
            Some("A"),
        );
        assert_eq!(
            mapping_for(
                ACTION_BTN2,
                &keymap,
                InputSource::Gamepad,
                GamepadFamily::Nintendo,
            ),
            Some("B"),
        );
        // BTN3 is unaffected (north + west buttons read the same on
        // any family).
        assert_eq!(
            mapping_for(
                ACTION_BTN3,
                &keymap,
                InputSource::Gamepad,
                GamepadFamily::Nintendo,
            ),
            Some("X"),
        );
    }

    #[test]
    fn effective_face_buttons_swaps_only_btn1_and_btn2_on_nintendo() {
        use GamepadButton::*;
        // Xbox: untouched defaults from BINDINGS.
        let xbox = effective_face_buttons(ACTION_BTN1, GamepadFamily::Xbox);
        assert_eq!(xbox.first().copied(), Some(GAMEPAD_BUTTON_RIGHT_FACE_DOWN));
        // Nintendo: swapped.
        let n_btn1 = effective_face_buttons(ACTION_BTN1, GamepadFamily::Nintendo);
        assert_eq!(
            n_btn1.first().copied(),
            Some(GAMEPAD_BUTTON_RIGHT_FACE_RIGHT),
        );
        let n_btn2 = effective_face_buttons(ACTION_BTN2, GamepadFamily::Nintendo);
        assert_eq!(
            n_btn2.first().copied(),
            Some(GAMEPAD_BUTTON_RIGHT_FACE_DOWN),
        );
        // BTN3 untouched on Nintendo.
        let n_btn3 = effective_face_buttons(ACTION_BTN3, GamepadFamily::Nintendo);
        assert_eq!(n_btn3.first().copied(), Some(GAMEPAD_BUTTON_RIGHT_FACE_UP));
        // Triggers (LB/RB) preserved across the swap.
        assert!(n_btn1.contains(&GAMEPAD_BUTTON_LEFT_TRIGGER_1));
        assert!(n_btn2.contains(&GAMEPAD_BUTTON_RIGHT_TRIGGER_1));
    }

    #[test]
    fn binding_columns_reflects_nintendo_face_swap() {
        let keymap = Keymap::default();
        let cols = binding_columns(&keymap, GamepadFamily::Nintendo);
        // BTN1's gamepad column starts with the Nintendo-A label.
        assert!(
            cols[ACTION_BTN1 as usize - 1].2.starts_with("A"),
            "BTN1 should lead with Nintendo's A: {:?}",
            cols[ACTION_BTN1 as usize - 1].2,
        );
        assert!(
            cols[ACTION_BTN2 as usize - 1].2.starts_with("B"),
            "BTN2 should lead with Nintendo's B: {:?}",
            cols[ACTION_BTN2 as usize - 1].2,
        );
    }

    #[test]
    fn input_source_as_str_round_trips_lowercase_names() {
        assert_eq!(InputSource::Keyboard.as_str(), "keyboard");
        assert_eq!(InputSource::Gamepad.as_str(), "gamepad");
    }

    #[test]
    fn gamepad_family_detect_classifies_known_names() {
        // Names captured loosely; matches are case-insensitive
        // substrings since OS/driver text varies. Default falls
        // through to Xbox so Steam Deck and generic pads are right.
        assert_eq!(
            GamepadFamily::detect("Sony DualSense Wireless Controller"),
            GamepadFamily::PlayStation,
        );
        assert_eq!(
            GamepadFamily::detect("PS4 Controller"),
            GamepadFamily::PlayStation,
        );
        assert_eq!(
            GamepadFamily::detect("Nintendo Switch Pro Controller"),
            GamepadFamily::Nintendo,
        );
        assert_eq!(
            GamepadFamily::detect("Joy-Con (L)"),
            GamepadFamily::Nintendo,
        );
        assert_eq!(
            GamepadFamily::detect("Xbox Wireless Controller"),
            GamepadFamily::Xbox,
        );
        assert_eq!(
            GamepadFamily::detect("Generic USB Gamepad"),
            GamepadFamily::Xbox,
        );
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

    /// Mouse constants must match raylib's enum values, since
    /// `mouse_button_from_u32` casts through them. If raylib renumbers
    /// these in a future release, this test catches it before users hit
    /// silently wrong button mappings.
    #[test]
    fn mouse_constants_match_raylib_enum() {
        assert_eq!(MOUSE_LEFT, MouseButton::MOUSE_BUTTON_LEFT as u32);
        assert_eq!(MOUSE_RIGHT, MouseButton::MOUSE_BUTTON_RIGHT as u32);
    }

    #[test]
    fn unknown_mouse_buttons_map_to_none() {
        assert!(mouse_button_from_u32(2).is_none());
        assert!(mouse_button_from_u32(99).is_none());
    }

    /// 1280x720 window with a 320x180 game gives a clean 4x scale, so
    /// the screen center maps to the game center. Spot-check a couple
    /// of corners too.
    #[test]
    fn screen_to_game_at_clean_4x_scale() {
        let (sw, sh) = (1280, 720);
        let res = crate::config::Resolution::DEFAULT;
        assert_eq!(screen_to_game(640.0, 360.0, sw, sh, res, false), (160, 90));
        assert_eq!(screen_to_game(0.0, 0.0, sw, sh, res, false), (0, 0));
        // Pixel just past the right edge: outside the game viewport
        // (game is 320 wide, so 320 itself is one past the last pixel).
        assert_eq!(
            screen_to_game(1280.0, 720.0, sw, sh, res, false),
            (320, 180),
            "should return out-of-range, not clamp"
        );
    }

    /// A non-integer scale window letterboxes either side. Verify the
    /// cursor over a letterbox bar produces a negative x, and that the
    /// game-space center is still hit when the cursor is at the window
    /// center.
    #[test]
    fn screen_to_game_letterbox_yields_out_of_range() {
        // 800x600: width-limited scale = 800/320 = 2.5, height fits
        // 600/180 = 3.33, so scale = 2.5 (non-pixel-perfect). Scaled
        // height = 450, leaving 75px black bars top and bottom.
        let (sw, sh) = (800, 600);
        let res = crate::config::Resolution::DEFAULT;
        let (cx, cy) = screen_to_game(400.0, 300.0, sw, sh, res, false);
        assert_eq!((cx, cy), (160, 90));
        // Click on the top letterbox bar: y should be negative.
        let (_, top_y) = screen_to_game(400.0, 10.0, sw, sh, res, false);
        assert!(top_y < 0, "expected negative y for top bar, got {top_y}");
    }

    /// Pixel-perfect mode floors the scale, so 800x600 (width-limited
    /// scale 2.5) drops to integer 2x. That changes both the game-space
    /// mapping and the size of the letterbox bars.
    #[test]
    fn screen_to_game_pixel_perfect_floors_scale() {
        let (sw, sh) = (800, 600);
        let res = crate::config::Resolution::DEFAULT;
        let (free, _) = screen_to_game(400.0, 300.0, sw, sh, res, false);
        let (pp, _) = screen_to_game(400.0, 300.0, sw, sh, res, true);
        assert_eq!(free, 160);
        assert_eq!(pp, 160, "center stays mapped to game center either way");
        // Off-center: free scale = 2.5, pp scale = 2.0, so a 100px
        // window offset yields different game offsets.
        let (free_x, _) = screen_to_game(500.0, 300.0, sw, sh, res, false);
        let (pp_x, _) = screen_to_game(500.0, 300.0, sw, sh, res, true);
        assert_eq!(free_x, 200);
        assert_eq!(pp_x, 210);
    }

    /// Different game resolution exercises the parameterized math.
    #[test]
    fn screen_to_game_at_custom_resolution() {
        // 480x270 game in a 1920x1080 window: clean 4x scale.
        let (sw, sh) = (1920, 1080);
        let res = crate::config::Resolution { w: 480.0, h: 270.0 };
        assert_eq!(screen_to_game(960.0, 540.0, sw, sh, res, false), (240, 135));
    }
}
