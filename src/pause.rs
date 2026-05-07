//! Pause menu. Pico-8-style overlay with multiple "scenes" stacked:
//!
//! - **Top** — main item list (volumes, fullscreen, Input sub-menu,
//!   Clear Save, Reset, Quit). See `pause/top.rs`.
//! - **InputMenu** — Input sub-menu (Test / Configure Keys). See
//!   `pause/input_menu.rs`.
//! - **InputTester** — visual D-pad / button tester + binding table.
//!   See `pause/input_tester.rs`.
//! - **KeyConfig** — Pico-8-style sequential key capture. See
//!   `pause/key_config.rs`.
//! - **ConfirmClearSave** — yes/no dialog. See `pause/confirm_clear.rs`.
//!
//! This file owns the public surface (`PauseMenu`, `PauseAction`),
//! the `View` enum that dispatches between scenes, the input bundling
//! (`pause/inputs.rs`) that lets `update_with` be a pure transition,
//! and the integration tests that drive navigation across scenes.
//!
//! Side effects (settings write, fullscreen toggle, `_init`, save
//! clear, quit, keymap write) are emitted as `PauseAction` and applied
//! by the session. That keeps this module session-handle-free and
//! makes the navigation testable without a raylib window.

mod confirm_clear;
mod input_menu;
mod input_tester;
mod inputs;
mod key_config;
mod top;
mod volume;

use crate::input::{AxisEdgeTracker, GamepadFamily};
use crate::keymap::{self, Keymap};
use crate::palette;
use crate::palette::Pal;
use crate::settings::Settings;
use inputs::{KeyConfigInputs, MenuInputs, read_inputs, snapshot_tester};
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
    Quit,
}

/// Internal state machine. Every scene is a `View` variant; the
/// `update_with` dispatcher routes inputs to the matching scene's
/// `handle_*`, and `draw` routes to its `draw_*`.
#[derive(Debug, Clone, Copy, PartialEq)]
enum View {
    Top,
    /// Sub-menu under Input: Test and Configure Keys. Splitting these
    /// out keeps the Tester from intercepting BTN1/BTN2.
    InputMenu,
    InputTester,
    KeyConfig,
    ConfirmClearSave,
}

pub struct PauseMenu {
    pub open: bool,
    last_open: bool,
    view: View,
    top_selected: usize,
    input_menu_selected: usize,
    confirm_selected: usize,
    /// Drives the active-item indicator's sin oscillation.
    time: f32,
    /// `action_down` snapshot for `draw` to light the Tester rects
    /// without holding a raylib handle.
    tester_input: [bool; ACTION_COUNT],
    /// Capture state while in `View::KeyConfig`; `None` otherwise.
    key_config: Option<KeyConfigState>,
}

impl PauseMenu {
    pub fn new() -> Self {
        Self {
            open: false,
            last_open: false,
            view: View::Top,
            top_selected: 0,
            input_menu_selected: 0,
            confirm_selected: 0,
            time: 0.0,
            tester_input: [false; ACTION_COUNT],
            key_config: None,
        }
    }

    pub fn update(
        &mut self,
        rl: &mut RaylibHandle,
        settings: &Settings,
        keymap: &Keymap,
        axes: &AxisEdgeTracker,
        dt: f32,
    ) -> Option<PauseAction> {
        let menu_inputs = read_inputs(rl, keymap, axes);

        // Snapshot the held actions so `draw` doesn't need `rl`.
        self.tester_input = snapshot_tester(rl, keymap);

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
        let kc_inputs = KeyConfigInputs {
            captured_key,
            delete: rl.is_key_pressed(KeyboardKey::KEY_DELETE),
            backspace: rl.is_key_pressed(KeyboardKey::KEY_BACKSPACE),
        };

        self.update_with(menu_inputs, settings, keymap, kc_inputs, dt)
    }

    /// Pure transition; tests drive this without a raylib handle.
    fn update_with(
        &mut self,
        inputs: MenuInputs,
        settings: &Settings,
        keymap: &Keymap,
        kc: KeyConfigInputs,
        dt: f32,
    ) -> Option<PauseAction> {
        self.last_open = self.open;
        self.time += dt;

        if !self.open {
            if inputs.toggle {
                self.open = true;
                self.view = View::Top;
                self.top_selected = 0;
                self.key_config = None;
            }
            return None;
        }

        // Toggle (Esc/Enter/P/Start) climbs one level: Top closes the
        // menu, sub-views return to parent. Consistent so the player
        // never has to learn a per-view rule.
        if inputs.toggle {
            return match self.view {
                View::Top => {
                    self.open = false;
                    self.key_config = None;
                    Some(PauseAction::Resume)
                }
                View::InputMenu => {
                    self.view = View::Top;
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
                View::ConfirmClearSave => {
                    self.view = View::Top;
                    None
                }
            };
        }

        match self.view {
            View::Top => self.handle_top(inputs, settings),
            View::InputMenu => self.handle_input_menu(inputs, keymap),
            View::InputTester => self.handle_input_tester(inputs),
            View::KeyConfig => self.handle_key_config(inputs, kc),
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
        settings: &Settings,
        keymap: &Keymap,
        gamepad_family: GamepadFamily,
        res: crate::config::Resolution,
    ) {
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
            View::InputMenu => "INPUT",
            View::InputTester => "INPUT TEST",
            View::KeyConfig => "KEY CONFIG (KEYBOARD)",
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
            View::Top => self.draw_top(d, font, settings, body_y),
            View::InputMenu => self.draw_input_menu(d, font, body_y),
            View::InputTester => {
                self.draw_input_tester(d, font, keymap, gamepad_family, body_y, res)
            }
            View::KeyConfig => self.draw_key_config(d, font, body_y, res),
            View::ConfirmClearSave => self.draw_confirm_clear(d, font, body_y, res),
        }
    }
}

impl Default for PauseMenu {
    fn default() -> Self {
        Self::new()
    }
}

/// Active-item indicator: a small white dot that oscillates next to
/// the selected row. Lives at the parent level because every list-
/// shaped scene uses it (Top, InputMenu, ConfirmClearSave).
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
    use top::{
        ITEM_CLEAR, ITEM_FULLSCREEN, ITEM_INPUT, ITEM_MUSIC, ITEM_QUIT, ITEM_RESET, TOP_COUNT,
    };

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

    fn step(
        m: &mut PauseMenu,
        s: &Settings,
        k: &Keymap,
        inputs: MenuInputs,
    ) -> Option<PauseAction> {
        m.update_with(inputs, s, k, KeyConfigInputs::default(), 0.016)
    }

    fn capture(
        m: &mut PauseMenu,
        s: &Settings,
        k: &Keymap,
        key: KeyboardKey,
    ) -> Option<PauseAction> {
        let kc = KeyConfigInputs {
            captured_key: Some(key),
            ..Default::default()
        };
        m.update_with(MenuInputs::default(), s, k, kc, 0.016)
    }

    fn delete(m: &mut PauseMenu, s: &Settings, k: &Keymap) -> Option<PauseAction> {
        let kc = KeyConfigInputs {
            delete: true,
            ..Default::default()
        };
        m.update_with(MenuInputs::default(), s, k, kc, 0.016)
    }

    fn backspace(m: &mut PauseMenu, s: &Settings, k: &Keymap) -> Option<PauseAction> {
        let kc = KeyConfigInputs {
            backspace: true,
            ..Default::default()
        };
        m.update_with(MenuInputs::default(), s, k, kc, 0.016)
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
        assert_eq!(m.top_selected, TOP_COUNT - 1);
        step(&mut m, &s, &k, down());
        assert_eq!(m.top_selected, 0);
    }

    #[test]
    fn left_right_on_music_emits_set_music_volume() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        step(&mut m, &s, &k, toggle());
        step(&mut m, &s, &k, down());
        assert_eq!(m.top_selected, ITEM_MUSIC);
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
        step(&mut m, &s, &k, toggle());
        for _ in 0..3 {
            step(&mut m, &s, &k, down());
        }
        assert_eq!(m.top_selected, ITEM_FULLSCREEN);
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
        for _ in 0..ITEM_CLEAR {
            step(&mut m, &s, &k, down());
        }
        assert_eq!(m.top_selected, ITEM_CLEAR);
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
        for _ in 0..ITEM_CLEAR {
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
        for _ in 0..ITEM_CLEAR {
            step(&mut m, &s, &k, down());
        }
        step(&mut m, &s, &k, btn1());
        assert_eq!(m.view, View::ConfirmClearSave);
        assert_eq!(step(&mut m, &s, &k, btn2()), None);
        assert_eq!(m.view, View::Top);
    }

    #[test]
    fn input_lands_on_input_menu_and_btn2_returns_to_top() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        step(&mut m, &s, &k, toggle());
        for _ in 0..ITEM_INPUT {
            step(&mut m, &s, &k, down());
        }
        step(&mut m, &s, &k, btn1());
        assert_eq!(m.view, View::InputMenu);
        // Default selection is Test.
        assert_eq!(m.input_menu_selected, INPUT_ITEM_TEST);
        step(&mut m, &s, &k, btn2());
        assert_eq!(m.view, View::Top);
    }

    #[test]
    fn input_menu_test_enters_tester_and_buttons_are_not_consumed() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        step(&mut m, &s, &k, toggle());
        for _ in 0..ITEM_INPUT {
            step(&mut m, &s, &k, down());
        }
        step(&mut m, &s, &k, btn1()); // Top -> InputMenu
        step(&mut m, &s, &k, btn1()); // InputMenu -> InputTester (Test selected)
        assert_eq!(m.view, View::InputTester);
        // Inside the tester, BTN1/BTN2 should NOT change view: they
        // are testable inputs. Only toggle (Esc/Enter/P/Start) exits.
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
        for _ in 0..ITEM_RESET {
            step(&mut m, &s, &k, down());
        }
        assert_eq!(step(&mut m, &s, &k, btn1()), Some(PauseAction::ResetGame));
        assert!(!m.open, "Reset Game should close the menu");

        step(&mut m, &s, &k, toggle());
        for _ in 0..ITEM_QUIT {
            step(&mut m, &s, &k, down());
        }
        assert_eq!(m.top_selected, ITEM_QUIT);
        assert_eq!(step(&mut m, &s, &k, btn1()), Some(PauseAction::Quit));
    }

    #[test]
    fn toggle_climbs_one_level() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let k = Keymap::default();
        step(&mut m, &s, &k, toggle()); // open: Top
        for _ in 0..ITEM_INPUT {
            step(&mut m, &s, &k, down());
        }
        step(&mut m, &s, &k, btn1()); // Top -> InputMenu
        assert_eq!(m.view, View::InputMenu);
        // Toggle from InputMenu returns to Top (no Resume).
        let action = step(&mut m, &s, &k, toggle());
        assert_eq!(action, None);
        assert_eq!(m.view, View::Top);
        // Toggle from Top closes the whole menu.
        let action = step(&mut m, &s, &k, toggle());
        assert!(!m.open);
        assert_eq!(action, Some(PauseAction::Resume));
    }

    fn open_to_key_config(m: &mut PauseMenu, s: &Settings, k: &Keymap) {
        step(m, s, k, toggle());
        for _ in 0..ITEM_INPUT {
            step(m, s, k, down());
        }
        // Top -> InputMenu
        step(m, s, k, btn1());
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
}
