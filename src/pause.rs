//! Pause menu. Pico-8-style overlay with multiple "scenes" stacked:
//!
//! - **Top** — main item list (Continue, Settings sub-menu, Clear
//!   Save, Reset, Quit). See `pause/top.rs`.
//! - **SettingsMenu** — Settings sub-menu (Music, SFX, Fullscreen,
//!   Input). See `pause/settings_menu.rs`.
//! - **InputMenu** — Input sub-menu (Test Input / Configure Keys /
//!   Configure Gamepad). See `pause/input_menu.rs`.
//! - **InputTester** — visual D-pad / button tester + binding table.
//!   See `pause/input_tester.rs`.
//! - **KeyConfig** — Pico-8-style sequential key capture. See
//!   `pause/key_config.rs`.
//! - **PadConfig** — Pico-8-style sequential gamepad button capture
//!   (BTN1/BTN2/BTN3 only; directionals stay on dpad + stick). See
//!   `pause/pad_config.rs`.
//! - **ConfirmClearSave** — yes/no dialog. See `pause/confirm_clear.rs`.
//!
//! This file owns the public surface (`PauseMenu`, `PauseAction`),
//! the `View` enum that dispatches between scenes, the input bundling
//! (`pause/inputs.rs`) that lets `update_with` be a pure transition,
//! and the integration tests that drive navigation across scenes.
//!
//! Side effects (settings write, fullscreen toggle, `_init`, save
//! clear, quit, keymap write, pad_map write) are emitted as
//! `PauseAction` and applied by the session. That keeps this module
//! session-handle-free and makes the navigation testable without a
//! raylib window.

mod confirm_clear;
mod input_menu;
mod input_tester;
mod inputs;
mod key_config;
mod pad_config;
mod settings_menu;
mod top;
mod volume;

use crate::input::{AxisEdgeTracker, GamepadFamily};
use crate::keymap::{self, Keymap};
use crate::pad_map::PadMap;
use crate::palette;
use crate::palette::Pal;
#[cfg(test)]
use crate::settings::Settings;
use inputs::{
    CaptureInputs, KeyConfigInputs, MenuInputs, PadConfigInputs, first_bindable_button_pressed,
    gamepad_select_pressed, read_inputs, snapshot_tester,
};
pub use inputs::{Maps, PauseFrame};
use key_config::{KeyConfigState, is_reserved_key};
use sola_raylib::prelude::*;

/// Number of abstract input actions (LEFT, RIGHT, UP, DOWN, BTN1,
/// BTN2, BTN3). Used to size the Tester snapshot and the Key Config
/// capture loop.
pub(crate) const ACTION_COUNT: usize = 7;

/// Transitions emitted by the menu and applied by the session.
/// Anything touching the session, audio, or disk goes through here.
#[derive(Debug, Clone, PartialEq)]
pub enum PauseAction {
    Resume,
    SetMusicVolume(f32),
    SetSfxVolume(f32),
    ToggleFullscreen,
    ResetGame,
    ClearSave,
    SetKeymap(Keymap),
    SetGamepadMap(PadMap),
    /// Player selected a Lua-registered `usagi.menu_item` row. The
    /// index points into the snapshot of menu_item labels that was
    /// passed into `update` for the frame, which mirrors the
    /// session-side menu_items store.
    FireMenuItem(usize),
    /// Native-only. Web hides the menu item (emscripten's main loop
    /// can't exit), so the variant is never constructed there.
    #[cfg_attr(target_os = "emscripten", allow(dead_code))]
    Quit,
}

/// Internal state machine. Every scene is a `View` variant; the
/// `update_with` dispatcher routes inputs to the matching scene's
/// `handle_*`, and `draw` routes to its `draw_*`.
#[derive(Debug, Clone, Copy, PartialEq)]
enum View {
    Top,
    /// Sub-menu under Top: bundles every "tweakable" so the Top list
    /// stays short (Continue + destructive actions + Quit). Holds
    /// Music, SFX, Fullscreen, and the Input sub-menu entry.
    SettingsMenu,
    /// Sub-menu under Settings: Test Input, Configure Keys, Configure
    /// Gamepad. Splitting these out keeps the Tester from intercepting
    /// BTN1/BTN2.
    InputMenu,
    InputTester,
    KeyConfig,
    PadConfig,
    ConfirmClearSave,
}

/// In-flight Pad Config capture. Mutated as the player presses gamepad
/// buttons; emitted via `PauseAction::SetGamepadMap` on completion.
/// Stored at the parent module so `pad_config.rs` and the integration
/// tests can both reach it.
#[derive(Debug, Clone)]
pub(crate) struct PadConfigState {
    pub staging: PadMap,
    /// Index into `pad_map::PAD_ACTIONS` (0..PAD_OVERRIDE_COUNT) of the
    /// action currently awaiting a button press.
    pub action_index: usize,
}

pub struct PauseMenu {
    pub open: bool,
    last_open: bool,
    view: View,
    top_selected: usize,
    settings_menu_selected: usize,
    input_menu_selected: usize,
    confirm_selected: usize,
    /// Drives the active-item indicator's sin oscillation.
    time: f32,
    /// `action_down` snapshot for `draw` to light the Tester rects
    /// without holding a raylib handle.
    tester_input: [bool; ACTION_COUNT],
    /// Capture state while in `View::KeyConfig`; `None` otherwise.
    key_config: Option<KeyConfigState>,
    /// Capture state while in `View::PadConfig`; `None` otherwise.
    pad_config: Option<PadConfigState>,
}

impl PauseMenu {
    pub fn new() -> Self {
        Self {
            open: false,
            last_open: false,
            view: View::Top,
            top_selected: 0,
            settings_menu_selected: 0,
            input_menu_selected: 0,
            confirm_selected: 0,
            time: 0.0,
            tester_input: [false; ACTION_COUNT],
            key_config: None,
            pad_config: None,
        }
    }

    pub fn update(
        &mut self,
        rl: &mut RaylibHandle,
        frame: PauseFrame<'_>,
        axes: &AxisEdgeTracker,
        dt: f32,
    ) -> Option<PauseAction> {
        let menu_inputs = read_inputs(rl, frame.maps.keymap, frame.maps.pad_map, axes, self.open);

        // Snapshot the held actions so `draw` doesn't need `rl`.
        self.tester_input = snapshot_tester(rl, frame.maps.keymap, frame.maps.pad_map);

        // Only drain raylib's key queue while capturing, so presses
        // on other views aren't silently consumed.
        let mut captured_key: Option<KeyboardKey> = None;
        if self.view == View::KeyConfig {
            // Take the first supported, non-reserved key; drop the rest.
            while let Some(k) = rl.get_key_pressed() {
                if is_reserved_key(k) {
                    continue;
                }
                if keymap::key_label(k).is_some() {
                    captured_key = Some(k);
                    break;
                }
            }
        }
        let kc = KeyConfigInputs {
            captured_key,
            delete: rl.is_key_pressed(KeyboardKey::KEY_DELETE),
            backspace: rl.is_key_pressed(KeyboardKey::KEY_BACKSPACE),
        };

        // Only scan gamepad buttons while in PadConfig so presses on
        // other views aren't silently consumed. Select on the pad
        // acts as a Backspace-equivalent so a gamepad-only player can
        // undo without reaching for the keyboard.
        let mut captured_button: Option<GamepadButton> = None;
        let mut pad_select = false;
        if self.view == View::PadConfig {
            captured_button = first_bindable_button_pressed(rl);
            pad_select = gamepad_select_pressed(rl);
        }
        let pc = PadConfigInputs {
            captured_button,
            delete: rl.is_key_pressed(KeyboardKey::KEY_DELETE),
            backspace: rl.is_key_pressed(KeyboardKey::KEY_BACKSPACE) || pad_select,
        };

        self.update_with(menu_inputs, frame, CaptureInputs { kc, pc }, dt)
    }

    /// Pure transition; tests drive this without a raylib handle.
    fn update_with(
        &mut self,
        inputs: MenuInputs,
        frame: PauseFrame<'_>,
        cap: CaptureInputs,
        dt: f32,
    ) -> Option<PauseAction> {
        let PauseFrame {
            settings,
            maps,
            menu_items,
        } = frame;
        let CaptureInputs { kc, pc } = cap;
        self.last_open = self.open;
        self.time += dt;

        if !self.open {
            if inputs.toggle {
                self.open = true;
                self.view = View::Top;
                self.top_selected = 0;
                self.key_config = None;
                self.pad_config = None;
            }
            return None;
        }

        // Pad Config claims gamepad Start as its reset gesture (the
        // gamepad-side counterpart to keyboard DEL): wipe every override
        // and drop back to the Tester. Has to fire before the central
        // toggle dispatcher, since `read_inputs` folds Start into
        // `toggle` too. Esc / P in Pad Config still cancel without
        // resetting because `start_press` is gamepad-only.
        if inputs.start_press && self.view == View::PadConfig {
            self.view = View::InputTester;
            self.pad_config = None;
            return Some(PauseAction::SetGamepadMap(PadMap::default()));
        }

        // Toggle (Esc/P/Start) climbs one level: Top closes the menu,
        // sub-views return to parent. Consistent so the player never
        // has to learn a per-view rule. (Enter only opens; once
        // inside, it routes to `btn1` instead of climbing.)
        if inputs.toggle {
            return match self.view {
                View::Top => {
                    self.open = false;
                    self.key_config = None;
                    self.pad_config = None;
                    Some(PauseAction::Resume)
                }
                View::SettingsMenu => {
                    self.view = View::Top;
                    None
                }
                View::InputMenu => {
                    self.view = View::SettingsMenu;
                    None
                }
                View::InputTester => {
                    self.view = View::InputMenu;
                    None
                }
                View::KeyConfig => {
                    self.view = View::InputMenu;
                    self.key_config = None;
                    None
                }
                View::PadConfig => {
                    self.view = View::InputMenu;
                    self.pad_config = None;
                    None
                }
                View::ConfirmClearSave => {
                    self.view = View::Top;
                    None
                }
            };
        }

        match self.view {
            View::Top => self.handle_top(inputs, menu_items),
            View::SettingsMenu => self.handle_settings_menu(inputs, settings),
            View::InputMenu => self.handle_input_menu(inputs, maps.keymap, maps.pad_map),
            View::InputTester => self.handle_input_tester(inputs),
            View::KeyConfig => self.handle_key_config(inputs, kc),
            View::PadConfig => self.handle_pad_config(inputs, pc),
            View::ConfirmClearSave => self.handle_confirm_clear(inputs),
        }
    }

    pub fn just_opened(&self) -> bool {
        self.open && !self.last_open
    }

    pub fn just_closed(&self) -> bool {
        !self.open && self.last_open
    }

    pub fn draw<D: RaylibDraw>(
        &self,
        d: &mut D,
        font: &Font,
        frame: PauseFrame<'_>,
        gamepad_family: GamepadFamily,
        res: crate::config::Resolution,
    ) {
        let PauseFrame {
            settings,
            maps,
            menu_items,
        } = frame;
        d.draw_rectangle(
            0,
            0,
            res.w as i32,
            res.h as i32,
            palette::color(Pal::Black).alpha(0.8),
        );
        let border_padding = 4;
        d.draw_rectangle_lines(
            border_padding,
            border_padding,
            res.w as i32 - border_padding * 2,
            res.h as i32 - border_padding * 2,
            palette::color(Pal::White),
        );

        let size = crate::font::MONOGRAM_SIZE as f32;
        let title = match self.view {
            View::Top => "PAUSED",
            View::SettingsMenu => "SETTINGS",
            View::InputMenu => "INPUT",
            View::InputTester => "INPUT TEST",
            View::KeyConfig => "KEYBOARD CONFIG",
            View::PadConfig => "GAMEPAD CONFIG",
            View::ConfirmClearSave => "CLEAR SAVE?",
        };
        let title_m = font.measure_text(title, size, 0.0);
        let title_x = ((res.w - title_m.x) * 0.5).round();
        let title_y = 16.0;
        d.draw_text_ex(
            font,
            title,
            Vector2::new(title_x, title_y),
            size,
            0.0,
            palette::color(Pal::White),
        );

        let body_y = title_y + size + 8.0;
        match self.view {
            View::Top => self.draw_top(d, font, menu_items, body_y, res),
            View::SettingsMenu => self.draw_settings_menu(d, font, settings, body_y, res),
            View::InputMenu => self.draw_input_menu(d, font, body_y, res),
            View::InputTester => self.draw_input_tester(d, font, maps, gamepad_family, body_y, res),
            View::KeyConfig => self.draw_key_config(d, font, body_y, res),
            View::PadConfig => self.draw_pad_config(d, font, gamepad_family, body_y, res),
            View::ConfirmClearSave => self.draw_confirm_clear(d, font, body_y, res),
        }
    }
}

impl Default for PauseMenu {
    fn default() -> Self {
        Self::new()
    }
}

/// Left-margin for vertical-list scenes. Proportional to the game
/// width so the menu reads at low resolutions (128x128 prototypes,
/// vertical games) without clipping into the indicator on the left or
/// running into the right edge with long labels. Clamped to a floor
/// so it doesn't crowd the indicator at extreme widths.
fn item_x_for(res: crate::config::Resolution) -> f32 {
    (res.w * 0.1).max(8.0).round()
}

/// Active-item indicator: a small white dot that oscillates next to
/// the selected row. Lives at the parent level because every list-
/// shaped scene uses it (Top, SettingsMenu, InputMenu,
/// ConfirmClearSave).
fn draw_indicator<D: RaylibDraw>(d: &mut D, time: f32, item_x: f32, center_y: f32) {
    let amplitude = 1.5_f32;
    let speed = 6.0_f32;
    let osc = (time * speed).sin() * amplitude;
    let cx = item_x - 8.0 + osc;
    d.draw_circle(cx as i32, center_y as i32, 2.0, palette::color(Pal::White));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::ACTION_LEFT;
    use input_menu::INPUT_ITEM_TEST;
    use settings_menu::{SETTINGS_ITEM_FULLSCREEN, SETTINGS_ITEM_INPUT, SETTINGS_ITEM_MUSIC};
    use top::{item_clear, item_quit, item_reset, item_settings, top_count};

    // Convenience aliases: existing tests assume no menu items are
    // registered, so the named row constants resolve to their no-menu
    // positions. New tests that exercise menu items compute offsets
    // off the per-call menu_item count instead.
    fn item_settings_0() -> usize {
        item_settings(0)
    }
    fn item_clear_0() -> usize {
        item_clear(0)
    }
    fn item_reset_0() -> usize {
        item_reset(0)
    }
    #[cfg(not(target_os = "emscripten"))]
    fn item_quit_0() -> usize {
        item_quit(0)
    }
    fn top_count_0() -> usize {
        top_count(0)
    }

    fn toggle() -> MenuInputs {
        MenuInputs {
            toggle: true,
            ..Default::default()
        }
    }

    fn down() -> MenuInputs {
        MenuInputs {
            down: true,
            ..Default::default()
        }
    }

    fn up() -> MenuInputs {
        MenuInputs {
            up: true,
            ..Default::default()
        }
    }

    fn btn1() -> MenuInputs {
        MenuInputs {
            btn1: true,
            ..Default::default()
        }
    }

    fn btn2() -> MenuInputs {
        MenuInputs {
            btn2: true,
            ..Default::default()
        }
    }

    fn left() -> MenuInputs {
        MenuInputs {
            left: true,
            ..Default::default()
        }
    }

    fn right() -> MenuInputs {
        MenuInputs {
            right: true,
            ..Default::default()
        }
    }

    /// Gamepad Start press. `read_inputs` folds Start into `toggle` so
    /// it climbs in views that don't override it, and also surfaces
    /// `start_press` so Pad Config can claim it as the reset gesture.
    fn start() -> MenuInputs {
        MenuInputs {
            toggle: true,
            start_press: true,
            ..Default::default()
        }
    }

    fn frame<'a>(
        s: &'a Settings,
        k: &'a Keymap,
        p: &'a PadMap,
        menu_items: &'a [String],
    ) -> PauseFrame<'a> {
        PauseFrame {
            settings: s,
            maps: Maps {
                keymap: k,
                pad_map: p,
            },
            menu_items,
        }
    }

    fn step(
        m: &mut PauseMenu,
        s: &Settings,
        k: &Keymap,
        inputs: MenuInputs,
    ) -> Option<PauseAction> {
        let p = PadMap::default();
        m.update_with(
            inputs,
            frame(s, k, &p, &[]),
            CaptureInputs::default(),
            0.016,
        )
    }

    fn step_with_pad(
        m: &mut PauseMenu,
        s: &Settings,
        k: &Keymap,
        p: &PadMap,
        inputs: MenuInputs,
    ) -> Option<PauseAction> {
        m.update_with(inputs, frame(s, k, p, &[]), CaptureInputs::default(), 0.016)
    }

    fn step_with_menu_items(
        m: &mut PauseMenu,
        s: &Settings,
        k: &Keymap,
        menu_items: &[String],
        inputs: MenuInputs,
    ) -> Option<PauseAction> {
        let p = PadMap::default();
        m.update_with(
            inputs,
            frame(s, k, &p, menu_items),
            CaptureInputs::default(),
            0.016,
        )
    }

    fn capture(
        m: &mut PauseMenu,
        s: &Settings,
        k: &Keymap,
        key: KeyboardKey,
    ) -> Option<PauseAction> {
        let p = PadMap::default();
        let cap = CaptureInputs {
            kc: KeyConfigInputs {
                captured_key: Some(key),
                ..Default::default()
            },
            ..Default::default()
        };
        m.update_with(MenuInputs::default(), frame(s, k, &p, &[]), cap, 0.016)
    }

    fn capture_button(
        m: &mut PauseMenu,
        s: &Settings,
        k: &Keymap,
        p: &PadMap,
        btn: GamepadButton,
    ) -> Option<PauseAction> {
        let cap = CaptureInputs {
            pc: PadConfigInputs {
                captured_button: Some(btn),
                ..Default::default()
            },
            ..Default::default()
        };
        m.update_with(MenuInputs::default(), frame(s, k, p, &[]), cap, 0.016)
    }

    fn delete(m: &mut PauseMenu, s: &Settings, k: &Keymap) -> Option<PauseAction> {
        let p = PadMap::default();
        let cap = CaptureInputs {
            kc: KeyConfigInputs {
                delete: true,
                ..Default::default()
            },
            ..Default::default()
        };
        m.update_with(MenuInputs::default(), frame(s, k, &p, &[]), cap, 0.016)
    }

    fn pad_delete(m: &mut PauseMenu, s: &Settings, k: &Keymap, p: &PadMap) -> Option<PauseAction> {
        let cap = CaptureInputs {
            pc: PadConfigInputs {
                delete: true,
                ..Default::default()
            },
            ..Default::default()
        };
        m.update_with(MenuInputs::default(), frame(s, k, p, &[]), cap, 0.016)
    }

    fn pad_backspace(
        m: &mut PauseMenu,
        s: &Settings,
        k: &Keymap,
        p: &PadMap,
    ) -> Option<PauseAction> {
        let cap = CaptureInputs {
            pc: PadConfigInputs {
                backspace: true,
                ..Default::default()
            },
            ..Default::default()
        };
        m.update_with(MenuInputs::default(), frame(s, k, p, &[]), cap, 0.016)
    }

    fn backspace(m: &mut PauseMenu, s: &Settings, k: &Keymap) -> Option<PauseAction> {
        let p = PadMap::default();
        let cap = CaptureInputs {
            kc: KeyConfigInputs {
                backspace: true,
                ..Default::default()
            },
            ..Default::default()
        };
        m.update_with(MenuInputs::default(), frame(s, k, &p, &[]), cap, 0.016)
    }

    #[test]
    fn toggle_opens_and_closes_menu() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        let action = step(&mut m, &s, &k, toggle());
        assert!(m.open);
        assert_eq!(m.view, View::Top);
        assert_eq!(action, None);
        let action = step(&mut m, &s, &k, toggle());
        assert!(!m.open);
        assert_eq!(action, Some(PauseAction::Resume));
    }

    #[test]
    fn down_then_up_wraps_through_top_items() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        step(&mut m, &s, &k, toggle());
        step(&mut m, &s, &k, up());
        assert_eq!(m.top_selected, top_count_0() - 1);
        step(&mut m, &s, &k, down());
        assert_eq!(m.top_selected, 0);
    }

    /// Helper: open the menu, walk to Settings, enter the Settings
    /// sub-menu, walk to the requested item, and stop there.
    fn enter_settings_at(m: &mut PauseMenu, s: &Settings, k: &Keymap, target: usize) {
        step(m, s, k, toggle());
        for _ in 0..item_settings_0() {
            step(m, s, k, down());
        }
        step(m, s, k, btn1());
        assert_eq!(m.view, View::SettingsMenu);
        for _ in 0..target {
            step(m, s, k, down());
        }
        assert_eq!(m.settings_menu_selected, target);
    }

    #[test]
    fn left_right_on_music_emits_set_music_volume() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        enter_settings_at(&mut m, &s, &k, SETTINGS_ITEM_MUSIC);
        match step(&mut m, &s, &k, right()) {
            Some(PauseAction::SetMusicVolume(v)) => assert!((v - 1.0).abs() < 1e-5),
            other => panic!("expected SetMusicVolume, got {other:?}"),
        }
        match step(&mut m, &s, &k, left()) {
            Some(PauseAction::SetMusicVolume(v)) => assert!((v - 0.6).abs() < 1e-5),
            other => panic!("expected SetMusicVolume, got {other:?}"),
        }
    }

    #[test]
    fn left_right_on_fullscreen_emits_toggle() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        enter_settings_at(&mut m, &s, &k, SETTINGS_ITEM_FULLSCREEN);
        assert_eq!(
            step(&mut m, &s, &k, right()),
            Some(PauseAction::ToggleFullscreen)
        );
        assert_eq!(
            step(&mut m, &s, &k, left()),
            Some(PauseAction::ToggleFullscreen)
        );
        assert_eq!(
            step(&mut m, &s, &k, btn1()),
            Some(PauseAction::ToggleFullscreen)
        );
    }

    #[test]
    fn confirm_clear_defaults_to_no_and_cancels_on_btn1() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        step(&mut m, &s, &k, toggle());
        for _ in 0..item_clear_0() {
            step(&mut m, &s, &k, down());
        }
        assert_eq!(m.top_selected, item_clear_0());
        assert_eq!(step(&mut m, &s, &k, btn1()), None);
        assert_eq!(m.view, View::ConfirmClearSave);
        assert_eq!(m.confirm_selected, 0);
        assert_eq!(step(&mut m, &s, &k, btn1()), None);
        assert_eq!(m.view, View::Top);
    }

    #[test]
    fn confirm_clear_yes_emits_clear_save() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        step(&mut m, &s, &k, toggle());
        for _ in 0..item_clear_0() {
            step(&mut m, &s, &k, down());
        }
        step(&mut m, &s, &k, btn1());
        step(&mut m, &s, &k, down());
        assert_eq!(m.confirm_selected, 1);
        assert_eq!(step(&mut m, &s, &k, btn1()), Some(PauseAction::ClearSave));
        assert_eq!(m.view, View::Top);
    }

    #[test]
    fn btn2_in_confirm_returns_to_top() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        step(&mut m, &s, &k, toggle());
        for _ in 0..item_clear_0() {
            step(&mut m, &s, &k, down());
        }
        step(&mut m, &s, &k, btn1());
        assert_eq!(m.view, View::ConfirmClearSave);
        assert_eq!(step(&mut m, &s, &k, btn2()), None);
        assert_eq!(m.view, View::Top);
    }

    /// Helper: open the menu and land in InputMenu (Top -> Settings ->
    /// Input -> InputMenu).
    fn enter_input_menu(m: &mut PauseMenu, s: &Settings, k: &Keymap) {
        enter_settings_at(m, s, k, SETTINGS_ITEM_INPUT);
        step(m, s, k, btn1());
        assert_eq!(m.view, View::InputMenu);
    }

    #[test]
    fn input_lands_on_input_menu_and_btn2_returns_to_settings() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        enter_input_menu(&mut m, &s, &k);
        // Default selection is Test Input.
        assert_eq!(m.input_menu_selected, INPUT_ITEM_TEST);
        step(&mut m, &s, &k, btn2());
        assert_eq!(m.view, View::SettingsMenu);
    }

    #[test]
    fn input_menu_test_enters_tester_and_buttons_are_not_consumed() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        enter_input_menu(&mut m, &s, &k);
        step(&mut m, &s, &k, btn1()); // InputMenu -> InputTester (Test selected)
        assert_eq!(m.view, View::InputTester);
        // Inside the tester, BTN1/BTN2 should NOT change view: they
        // are testable inputs. Only toggle (Esc/P/Start) exits.
        step(&mut m, &s, &k, btn1());
        assert_eq!(m.view, View::InputTester);
        step(&mut m, &s, &k, btn2());
        assert_eq!(m.view, View::InputTester);
        // Toggle returns to InputMenu (one level up), not all the way
        // out of the menu.
        let action = step(&mut m, &s, &k, toggle());
        assert_eq!(action, None);
        assert_eq!(m.view, View::InputMenu);
        assert!(m.open);
    }

    #[test]
    fn reset_and_quit_emit_their_actions() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        step(&mut m, &s, &k, toggle());
        for _ in 0..item_reset_0() {
            step(&mut m, &s, &k, down());
        }
        assert_eq!(step(&mut m, &s, &k, btn1()), Some(PauseAction::ResetGame));
        assert!(!m.open, "Reset Game should close the menu");

        step(&mut m, &s, &k, toggle());
        for _ in 0..item_quit_0() {
            step(&mut m, &s, &k, down());
        }
        assert_eq!(m.top_selected, item_quit_0());
        assert_eq!(step(&mut m, &s, &k, btn1()), Some(PauseAction::Quit));
    }

    #[test]
    fn toggle_climbs_one_level() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        enter_input_menu(&mut m, &s, &k);
        // Toggle from InputMenu returns to SettingsMenu.
        let action = step(&mut m, &s, &k, toggle());
        assert_eq!(action, None);
        assert_eq!(m.view, View::SettingsMenu);
        // From SettingsMenu, toggle returns to Top.
        let action = step(&mut m, &s, &k, toggle());
        assert_eq!(action, None);
        assert_eq!(m.view, View::Top);
        // Toggle from Top closes the whole menu.
        let action = step(&mut m, &s, &k, toggle());
        assert!(!m.open);
        assert_eq!(action, Some(PauseAction::Resume));
    }

    fn open_to_key_config(m: &mut PauseMenu, s: &Settings, k: &Keymap) {
        enter_input_menu(m, s, k);
        // InputMenu: select "Configure Keys" (item 1) and confirm.
        step(m, s, k, down());
        step(m, s, k, btn1());
    }

    #[test]
    fn entering_key_config_seeds_staging_from_current_keymap() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let mut k = Keymap::default();
        k.overrides[ACTION_LEFT as usize - 1] = Some(KeyboardKey::KEY_W);
        open_to_key_config(&mut m, &s, &k);
        assert_eq!(m.view, View::KeyConfig);
        let state = m.key_config.as_ref().expect("key_config initialized");
        assert_eq!(state.action_index, 0);
        assert_eq!(
            state.staging.overrides[ACTION_LEFT as usize - 1],
            Some(KeyboardKey::KEY_W)
        );
    }

    #[test]
    fn capturing_seven_keys_emits_set_keymap_and_returns_to_tester() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        open_to_key_config(&mut m, &s, &k);

        let captures = [
            KeyboardKey::KEY_A,
            KeyboardKey::KEY_D,
            KeyboardKey::KEY_W,
            KeyboardKey::KEY_S,
            KeyboardKey::KEY_J,
            KeyboardKey::KEY_K,
            KeyboardKey::KEY_L,
        ];

        for key in &captures[..6] {
            assert_eq!(capture(&mut m, &s, &k, *key), None);
        }
        let final_action = capture(&mut m, &s, &k, captures[6]);
        match final_action {
            Some(PauseAction::SetKeymap(km)) => {
                for (i, key) in captures.iter().enumerate() {
                    assert_eq!(km.overrides[i], Some(*key));
                }
            }
            other => panic!("expected SetKeymap, got {other:?}"),
        }
        assert_eq!(m.view, View::InputTester);
        assert!(m.key_config.is_none());
    }

    #[test]
    fn toggle_during_key_config_cancels_capture_only() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        open_to_key_config(&mut m, &s, &k);
        capture(&mut m, &s, &k, KeyboardKey::KEY_W);
        let action = step(&mut m, &s, &k, toggle());
        assert_eq!(action, None, "toggle in KeyConfig should not emit anything");
        // Cancel returns to the parent InputMenu, not the Tester:
        // toggle is "go up one level" everywhere.
        assert_eq!(m.view, View::InputMenu);
        assert!(m.key_config.is_none());
        assert!(m.open, "menu stays open; only the capture was abandoned");
    }

    #[test]
    fn duplicate_key_press_is_rejected_and_keeps_player_on_current_action() {
        // Mashing the same key shouldn't silently advance: previous
        // capture wins, the duplicate press is a no-op, and the
        // active action stays put until the player picks another key.
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        open_to_key_config(&mut m, &s, &k);
        // First W: assigned to LEFT, advance to RIGHT.
        capture(&mut m, &s, &k, KeyboardKey::KEY_W);
        let state = m.key_config.as_ref().unwrap();
        assert_eq!(state.action_index, 1);
        assert_eq!(state.staging.overrides[0], Some(KeyboardKey::KEY_W));
        // Second W on RIGHT: rejected. Stay on RIGHT, slot 1 untouched.
        let action = capture(&mut m, &s, &k, KeyboardKey::KEY_W);
        assert_eq!(action, None);
        let state = m.key_config.as_ref().unwrap();
        assert_eq!(state.action_index, 1);
        assert_eq!(state.staging.overrides[1], None);
    }

    #[test]
    fn backspace_undoes_last_capture_and_steps_back_one_action() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        open_to_key_config(&mut m, &s, &k);
        capture(&mut m, &s, &k, KeyboardKey::KEY_A);
        capture(&mut m, &s, &k, KeyboardKey::KEY_D);
        // Backspace from RIGHT->UP transition: undo D, return to RIGHT.
        let action = backspace(&mut m, &s, &k);
        assert_eq!(action, None);
        let state = m.key_config.as_ref().unwrap();
        assert_eq!(state.action_index, 1);
        assert_eq!(state.staging.overrides[0], Some(KeyboardKey::KEY_A));
        assert_eq!(state.staging.overrides[1], None);
    }

    #[test]
    fn backspace_at_first_action_is_a_noop() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        open_to_key_config(&mut m, &s, &k);
        let action = backspace(&mut m, &s, &k);
        assert_eq!(action, None);
        let state = m.key_config.as_ref().unwrap();
        assert_eq!(state.action_index, 0);
        assert!(state.staging.overrides.iter().all(|s| s.is_none()));
    }

    #[test]
    fn delete_during_key_config_emits_default_keymap() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let mut k = Keymap::default();
        k.overrides[0] = Some(KeyboardKey::KEY_W);
        open_to_key_config(&mut m, &s, &k);
        // Stage a partial capture, then DEL: result is full reset, not
        // the partial staging.
        capture(&mut m, &s, &k, KeyboardKey::KEY_A);
        match delete(&mut m, &s, &k) {
            Some(PauseAction::SetKeymap(km)) => {
                assert_eq!(km, Keymap::default());
            }
            other => panic!("expected SetKeymap(default), got {other:?}"),
        }
        assert_eq!(m.view, View::InputTester);
        assert!(m.key_config.is_none());
    }

    fn open_to_pad_config(m: &mut PauseMenu, s: &Settings, k: &Keymap, p: &PadMap) {
        // Top -> Settings -> Input -> InputMenu (Test selected by default).
        step_with_pad(m, s, k, p, toggle());
        for _ in 0..item_settings_0() {
            step_with_pad(m, s, k, p, down());
        }
        step_with_pad(m, s, k, p, btn1());
        for _ in 0..SETTINGS_ITEM_INPUT {
            step_with_pad(m, s, k, p, down());
        }
        step_with_pad(m, s, k, p, btn1());
        // InputMenu: down twice to reach "Configure Gamepad", confirm.
        step_with_pad(m, s, k, p, down());
        step_with_pad(m, s, k, p, down());
        step_with_pad(m, s, k, p, btn1());
    }

    #[test]
    fn entering_pad_config_seeds_staging_from_current_pad_map() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        let mut p = PadMap::default();
        p.overrides[0] = Some(GamepadButton::GAMEPAD_BUTTON_RIGHT_TRIGGER_1);
        open_to_pad_config(&mut m, &s, &k, &p);
        assert_eq!(m.view, View::PadConfig);
        let state = m.pad_config.as_ref().expect("pad_config initialized");
        assert_eq!(state.action_index, 0);
        assert_eq!(
            state.staging.overrides[0],
            Some(GamepadButton::GAMEPAD_BUTTON_RIGHT_TRIGGER_1)
        );
    }

    #[test]
    fn capturing_three_buttons_emits_set_gamepad_map_and_returns_to_tester() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        let p = PadMap::default();
        open_to_pad_config(&mut m, &s, &k, &p);

        let captures = [
            GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN,
            GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_RIGHT,
            GamepadButton::GAMEPAD_BUTTON_RIGHT_TRIGGER_1,
        ];

        for btn in &captures[..2] {
            assert_eq!(capture_button(&mut m, &s, &k, &p, *btn), None);
        }
        let final_action = capture_button(&mut m, &s, &k, &p, captures[2]);
        match final_action {
            Some(PauseAction::SetGamepadMap(pm)) => {
                for (i, btn) in captures.iter().enumerate() {
                    assert_eq!(pm.overrides[i], Some(*btn));
                }
            }
            other => panic!("expected SetGamepadMap, got {other:?}"),
        }
        assert_eq!(m.view, View::InputTester);
        assert!(m.pad_config.is_none());
    }

    #[test]
    fn duplicate_button_press_during_pad_config_is_rejected() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        let p = PadMap::default();
        open_to_pad_config(&mut m, &s, &k, &p);
        // First face-down: bound to BTN1, advance to BTN2.
        capture_button(
            &mut m,
            &s,
            &k,
            &p,
            GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN,
        );
        let state = m.pad_config.as_ref().unwrap();
        assert_eq!(state.action_index, 1);
        assert_eq!(
            state.staging.overrides[0],
            Some(GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN)
        );
        // Second face-down on BTN2: rejected. Stay on BTN2, slot 1
        // untouched.
        let action = capture_button(
            &mut m,
            &s,
            &k,
            &p,
            GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN,
        );
        assert_eq!(action, None);
        let state = m.pad_config.as_ref().unwrap();
        assert_eq!(state.action_index, 1);
        assert_eq!(state.staging.overrides[1], None);
    }

    #[test]
    fn pad_backspace_undoes_last_capture() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        let p = PadMap::default();
        open_to_pad_config(&mut m, &s, &k, &p);
        capture_button(
            &mut m,
            &s,
            &k,
            &p,
            GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN,
        );
        capture_button(
            &mut m,
            &s,
            &k,
            &p,
            GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_RIGHT,
        );
        // Undo: clear slot 1, return to BTN2.
        assert_eq!(pad_backspace(&mut m, &s, &k, &p), None);
        let state = m.pad_config.as_ref().unwrap();
        assert_eq!(state.action_index, 1);
        assert_eq!(
            state.staging.overrides[0],
            Some(GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN)
        );
        assert_eq!(state.staging.overrides[1], None);
    }

    #[test]
    fn pad_delete_emits_default_gamepad_map() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        let mut p = PadMap::default();
        p.overrides[0] = Some(GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_RIGHT);
        open_to_pad_config(&mut m, &s, &k, &p);
        capture_button(
            &mut m,
            &s,
            &k,
            &p,
            GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN,
        );
        match pad_delete(&mut m, &s, &k, &p) {
            Some(PauseAction::SetGamepadMap(pm)) => {
                assert_eq!(pm, PadMap::default());
            }
            other => panic!("expected SetGamepadMap(default), got {other:?}"),
        }
        assert_eq!(m.view, View::InputTester);
        assert!(m.pad_config.is_none());
    }

    #[test]
    fn toggle_during_pad_config_cancels_capture_only() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        let p = PadMap::default();
        open_to_pad_config(&mut m, &s, &k, &p);
        capture_button(
            &mut m,
            &s,
            &k,
            &p,
            GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN,
        );
        let action = step_with_pad(&mut m, &s, &k, &p, toggle());
        assert_eq!(action, None);
        assert_eq!(m.view, View::InputMenu);
        assert!(m.pad_config.is_none());
        assert!(m.open);
    }

    /// Reaching PadConfig from the Tester requires the third menu
    /// item. Guards against off-by-one if more items are added later.
    #[test]
    fn pad_config_is_the_third_input_menu_item() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        let p = PadMap::default();
        // Open + walk through Settings -> Input -> InputMenu, then
        // down twice to land on INPUT_ITEM_CONFIGURE_PAD.
        step_with_pad(&mut m, &s, &k, &p, toggle());
        for _ in 0..item_settings_0() {
            step_with_pad(&mut m, &s, &k, &p, down());
        }
        step_with_pad(&mut m, &s, &k, &p, btn1());
        for _ in 0..SETTINGS_ITEM_INPUT {
            step_with_pad(&mut m, &s, &k, &p, down());
        }
        step_with_pad(&mut m, &s, &k, &p, btn1());
        step_with_pad(&mut m, &s, &k, &p, down());
        step_with_pad(&mut m, &s, &k, &p, down());
        assert_eq!(m.input_menu_selected, input_menu::INPUT_ITEM_CONFIGURE_PAD,);
        step_with_pad(&mut m, &s, &k, &p, btn1());
        assert_eq!(m.view, View::PadConfig);
    }

    #[test]
    fn start_during_pad_config_resets_and_drops_to_tester() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        let mut p = PadMap::default();
        p.overrides[0] = Some(GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_RIGHT);
        open_to_pad_config(&mut m, &s, &k, &p);
        // Stage a partial capture, then gamepad Start: result is full
        // reset (the gamepad-side counterpart to keyboard DEL).
        capture_button(
            &mut m,
            &s,
            &k,
            &p,
            GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN,
        );
        match step_with_pad(&mut m, &s, &k, &p, start()) {
            Some(PauseAction::SetGamepadMap(pm)) => {
                assert_eq!(pm, PadMap::default());
            }
            other => panic!("expected SetGamepadMap(default), got {other:?}"),
        }
        assert_eq!(m.view, View::InputTester);
        assert!(m.pad_config.is_none());
    }

    /// Esc (or P) in PadConfig still cancels without resetting: only
    /// the gamepad's Start triggers the destructive reset gesture.
    /// `toggle()` here simulates Esc since it sets `toggle: true`
    /// without `start_press`.
    #[test]
    fn esc_in_pad_config_climbs_one_level_without_resetting() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        let mut p = PadMap::default();
        p.overrides[0] = Some(GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_RIGHT);
        open_to_pad_config(&mut m, &s, &k, &p);
        let action = step_with_pad(&mut m, &s, &k, &p, toggle());
        assert_eq!(action, None, "Esc must not emit a destructive reset");
        assert_eq!(m.view, View::InputMenu);
        assert!(m.pad_config.is_none());
    }

    /// Sanity-check that Start in every non-PadConfig view still climbs
    /// one level (matching today's behavior). Only PadConfig overrides
    /// the gamepad Start.
    #[test]
    fn start_in_non_pad_views_keeps_climb_semantics() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        let p = PadMap::default();
        // From Top: Start closes the menu.
        step_with_pad(&mut m, &s, &k, &p, toggle());
        assert_eq!(
            step_with_pad(&mut m, &s, &k, &p, start()),
            Some(PauseAction::Resume),
        );
        assert!(!m.open);

        // From SettingsMenu: Start climbs to Top (one level).
        step_with_pad(&mut m, &s, &k, &p, toggle());
        for _ in 0..item_settings_0() {
            step_with_pad(&mut m, &s, &k, &p, down());
        }
        step_with_pad(&mut m, &s, &k, &p, btn1());
        assert_eq!(m.view, View::SettingsMenu);
        assert_eq!(step_with_pad(&mut m, &s, &k, &p, start()), None);
        assert_eq!(m.view, View::Top);
    }

    fn btn1_top_at(
        m: &mut PauseMenu,
        s: &Settings,
        k: &Keymap,
        menu_items: &[String],
        row: usize,
    ) -> Option<PauseAction> {
        // Open the menu, walk to `row` in Top, and press BTN1.
        step_with_menu_items(m, s, k, menu_items, toggle());
        for _ in 0..row {
            step_with_menu_items(m, s, k, menu_items, down());
        }
        assert_eq!(m.top_selected, row);
        step_with_menu_items(m, s, k, menu_items, btn1())
    }

    #[test]
    fn registered_menu_item_sits_between_continue_and_settings() {
        // With one registered item, Continue is row 0, the menu item is
        // row 1, and Settings shifts to row 2.
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        let items = vec!["Title Screen".to_string()];

        // BTN1 at row 1 fires the menu item, doesn't open Settings.
        assert_eq!(
            btn1_top_at(&mut m, &s, &k, &items, 1),
            Some(PauseAction::FireMenuItem(0)),
        );
        // Menu stays open (the session is the one that toggles open on
        // FireMenuItem; the pure transition doesn't auto-close).
        assert!(m.open);
        assert_eq!(m.view, View::Top);

        // BTN1 at row 2 still lands on Settings (shifted by 1).
        let mut m = PauseMenu::new();
        assert_eq!(btn1_top_at(&mut m, &s, &k, &items, 2), None);
        assert_eq!(m.view, View::SettingsMenu);
    }

    #[test]
    fn multiple_registered_items_fire_with_correct_indices() {
        let s = Settings::default();
        let k = Keymap::default();
        let items = vec![
            "Title Screen".to_string(),
            "Skip Level".to_string(),
            "Restart Run".to_string(),
        ];

        // Three items: rows 1, 2, 3.
        for (row, expected_idx) in [(1, 0), (2, 1), (3, 2)] {
            let mut m = PauseMenu::new();
            assert_eq!(
                btn1_top_at(&mut m, &s, &k, &items, row),
                Some(PauseAction::FireMenuItem(expected_idx)),
            );
        }
    }

    #[test]
    fn top_row_count_grows_with_registered_items() {
        // Wrap-around respects the new row count, so down() from the
        // last static row wraps to Continue past the menu items.
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        let items = vec!["A".to_string(), "B".to_string()];

        step_with_menu_items(&mut m, &s, &k, &items, toggle());
        // up() from Continue wraps to the last row (Quit on native).
        step_with_menu_items(&mut m, &s, &k, &items, up());
        let expected_last = top::top_count(items.len()) - 1;
        assert_eq!(m.top_selected, expected_last);
        // down() wraps back to Continue.
        step_with_menu_items(&mut m, &s, &k, &items, down());
        assert_eq!(m.top_selected, 0);
    }
}
