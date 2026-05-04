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

fn button_label(b: GamepadButton) -> &'static str {
    match b {
        GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_LEFT => "DPad-L",
        GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_RIGHT => "DPad-R",
        GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_UP => "DPad-U",
        GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_DOWN => "DPad-D",
        GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN => "Pad-A",
        GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_RIGHT => "Pad-B",
        GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_UP => "Pad-Y",
        GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_LEFT => "Pad-X",
        GamepadButton::GAMEPAD_BUTTON_LEFT_TRIGGER_1 => "L-Bumper",
        GamepadButton::GAMEPAD_BUTTON_RIGHT_TRIGGER_1 => "R-Bumper",
        _ => "?",
    }
}

fn axis_label(axis: GamepadAxis, sign: i8) -> &'static str {
    match (axis, sign.signum()) {
        (GamepadAxis::GAMEPAD_AXIS_LEFT_X, -1) => "Stick-L",
        (GamepadAxis::GAMEPAD_AXIS_LEFT_X, 1) => "Stick-R",
        (GamepadAxis::GAMEPAD_AXIS_LEFT_Y, -1) => "Stick-U",
        (GamepadAxis::GAMEPAD_AXIS_LEFT_Y, 1) => "Stick-D",
        _ => "?",
    }
}

/// Per-action binding split into keyboard and gamepad strings for the
/// pause menu's Input Test table. Override-replaces-keyboard mirrors
/// `action_pressed`.
pub fn binding_columns(keymap: &Keymap) -> [(&'static str, String, String); 7] {
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
        for btn in b.buttons {
            gp.push(button_label(*btn).to_string());
        }
        for (axis, sign) in b.axes {
            gp.push(axis_label(*axis, *sign).to_string());
        }
        out[i].1 = kb;
        out[i].2 = gp.join(", ");
    }
    out
}

/// Single canonical key for `action`: override if set, else the first
/// default from `BINDINGS`. Wraps a future `input.mapped_key` Lua API.
#[allow(dead_code)]
pub fn mapped_key(action: u32, keymap: &Keymap) -> Option<KeyboardKey> {
    if let Some(k) = keymap.override_for(action) {
        return Some(k);
    }
    binding(action)?.keys.first().copied()
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
/// suppressed so a key only fires its current owner. Gamepad and
/// axes always use the static defaults.
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
/// using `action_down`. Override semantics match `action_down`.
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
        for btn in b.buttons {
            if rl.is_gamepad_button_pressed(pad, *btn) {
                return true;
            }
        }
    }
    false
}

/// Inverts the screen-to-game render transform so a window-pixel mouse
/// position becomes game-pixel coords. Pure (no raylib handle), so tests
/// can exercise the math directly. May return values outside
/// `0..GAME_WIDTH` / `0..GAME_HEIGHT` when the cursor is over the
/// letterbox bars; games can detect that with a simple bounds check.
pub fn screen_to_game(
    mouse_x: f32,
    mouse_y: f32,
    screen_w: i32,
    screen_h: i32,
    pixel_perfect: bool,
) -> (i32, i32) {
    let (scale, ox, oy) = crate::render::game_view_transform(screen_w, screen_h, pixel_perfect);
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
    mouse_left_down: bool,
    mouse_right_down: bool,
    mouse_left_pressed: bool,
    mouse_right_pressed: bool,
    mouse_x: i32,
    mouse_y: i32,
}

impl InputState {
    /// Polls raylib once and rolls the result into a snapshot. Called
    /// at the top of each frame, before user Lua runs.
    pub fn sample(rl: &RaylibHandle, pixel_perfect: bool, keymap: &Keymap) -> Self {
        let mut down = 0u32;
        let mut pressed = 0u32;
        for (i, _) in BINDINGS.iter().enumerate() {
            let action = (i + 1) as u32;
            if action_down(rl, keymap, action) {
                down |= 1 << i;
            }
            if action_pressed(rl, keymap, action) {
                pressed |= 1 << i;
            }
        }
        let m = rl.get_mouse_position();
        let sw = rl.get_screen_width();
        let sh = rl.get_screen_height();
        let (mx, my) = screen_to_game(m.x, m.y, sw, sh, pixel_perfect);
        Self {
            actions_down: down,
            actions_pressed: pressed,
            mouse_left_down: rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT),
            mouse_right_down: rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_RIGHT),
            mouse_left_pressed: rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT),
            mouse_right_pressed: rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_RIGHT),
            mouse_x: mx,
            mouse_y: my,
        }
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
        let cols = binding_columns(&keymap);
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
        let cols = binding_columns(&keymap);
        let (_, kb, gp) = &cols[ACTION_LEFT as usize - 1];
        assert_eq!(kb, "W");
        assert!(!gp.contains('W'));
        assert!(
            gp.contains("DPad-L"),
            "gamepad column must survive keyboard override: {gp}"
        );
    }

    #[test]
    fn mapped_key_returns_override_when_set_else_first_default() {
        let mut keymap = Keymap::default();
        assert_eq!(
            mapped_key(ACTION_LEFT, &keymap),
            Some(KeyboardKey::KEY_LEFT)
        );
        keymap.overrides[ACTION_LEFT as usize - 1] = Some(KeyboardKey::KEY_W);
        assert_eq!(mapped_key(ACTION_LEFT, &keymap), Some(KeyboardKey::KEY_W));
        // Unknown action.
        assert_eq!(mapped_key(99, &keymap), None);
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
        assert_eq!(screen_to_game(640.0, 360.0, sw, sh, false), (160, 90));
        assert_eq!(screen_to_game(0.0, 0.0, sw, sh, false), (0, 0));
        // Pixel just past the right edge: outside the game viewport
        // (game is 320 wide, so 320 itself is one past the last pixel).
        assert_eq!(
            screen_to_game(1280.0, 720.0, sw, sh, false),
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
        let (cx, cy) = screen_to_game(400.0, 300.0, sw, sh, false);
        assert_eq!((cx, cy), (160, 90));
        // Click on the top letterbox bar: y should be negative.
        let (_, top_y) = screen_to_game(400.0, 10.0, sw, sh, false);
        assert!(top_y < 0, "expected negative y for top bar, got {top_y}");
    }

    /// Pixel-perfect mode floors the scale, so 800x600 (width-limited
    /// scale 2.5) drops to integer 2x. That changes both the game-space
    /// mapping and the size of the letterbox bars.
    #[test]
    fn screen_to_game_pixel_perfect_floors_scale() {
        let (sw, sh) = (800, 600);
        let (free, _) = screen_to_game(400.0, 300.0, sw, sh, false);
        let (pp, _) = screen_to_game(400.0, 300.0, sw, sh, true);
        assert_eq!(free, 160);
        assert_eq!(pp, 160, "center stays mapped to game center either way");
        // Off-center: free scale = 2.5, pp scale = 2.0, so a 100px
        // window offset yields different game offsets.
        let (free_x, _) = screen_to_game(500.0, 300.0, sw, sh, false);
        let (pp_x, _) = screen_to_game(500.0, 300.0, sw, sh, true);
        assert_eq!(free_x, 200);
        assert_eq!(pp_x, 210);
    }
}
