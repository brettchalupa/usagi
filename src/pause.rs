//! Pause menu. Pico-8-style overlay with a navigable item list,
//! per-channel volume bars, fullscreen toggle, a read-only Configure
//! Input view, a Clear Save Data confirm dialog, Reset Game, and Quit.
//!
//! UI lives here; side effects (writing settings, toggling fullscreen,
//! calling `_init`, clearing save, quitting) are dispatched by the
//! session via the returned `PauseAction`. That keeps this module from
//! needing a god-handle to the rest of the engine and makes the
//! navigation logic testable as a pure transition.

use crate::input::{
    self, ACTION_BTN1, ACTION_BTN2, ACTION_DOWN, ACTION_LEFT, ACTION_RIGHT, ACTION_UP,
    MAX_GAMEPADS, binding_descriptions,
};
use crate::palette;
use crate::palette::Pal;
use crate::settings::Settings;
use crate::{GAME_HEIGHT, GAME_WIDTH};
use sola_raylib::prelude::*;

/// Side-effecting transitions emitted by the menu and applied by the
/// session. The menu itself only mutates its own state (selection,
/// view); everything that touches the session, audio, or disk goes
/// through one of these.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PauseAction {
    Resume,
    SetMusicVolume(f32),
    SetSfxVolume(f32),
    ToggleFullscreen,
    ResetGame,
    ClearSave,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum View {
    Top,
    InputBindings,
    ConfirmClearSave,
}

const TOP_COUNT: usize = 8;
const ITEM_CONTINUE: usize = 0;
const ITEM_MUSIC: usize = 1;
const ITEM_SFX: usize = 2;
const ITEM_FULLSCREEN: usize = 3;
const ITEM_INPUT: usize = 4;
const ITEM_CLEAR: usize = 5;
const ITEM_RESET: usize = 6;
const ITEM_QUIT: usize = 7;

const VOLUME_STEPS: f32 = 5.0;
const VOLUME_STEP: f32 = 1.0 / VOLUME_STEPS;

pub struct PauseMenu {
    pub open: bool,
    last_open: bool,
    view: View,
    top_selected: usize,
    confirm_selected: usize,
    /// Drives the active-item indicator's sin oscillation.
    time: f32,
}

impl PauseMenu {
    pub fn new() -> Self {
        Self {
            open: false,
            last_open: false,
            view: View::Top,
            top_selected: 0,
            confirm_selected: 0,
            time: 0.0,
        }
    }

    pub fn update(
        &mut self,
        rl: &RaylibHandle,
        settings: &Settings,
        dt: f32,
    ) -> Option<PauseAction> {
        let inputs = read_inputs(rl);
        self.update_with(inputs, settings, dt)
    }

    /// Pure transition: computes `(state', action)` from `(state, inputs)`.
    /// Tests drive this directly without needing a raylib handle.
    fn update_with(
        &mut self,
        inputs: MenuInputs,
        settings: &Settings,
        dt: f32,
    ) -> Option<PauseAction> {
        self.last_open = self.open;
        self.time += dt;

        if !self.open {
            if inputs.toggle {
                self.open = true;
                self.view = View::Top;
                self.top_selected = 0;
            }
            return None;
        }

        // Toggle keys close the menu from any view, mirroring today's
        // "press Esc/Enter/P/Start to dismiss" behavior. Submenus get a
        // dedicated BTN2 to step back without leaving the menu.
        if inputs.toggle {
            self.open = false;
            self.view = View::Top;
            return Some(PauseAction::Resume);
        }

        match self.view {
            View::Top => self.handle_top(inputs, settings),
            View::InputBindings => self.handle_input_view(inputs),
            View::ConfirmClearSave => self.handle_confirm_clear(inputs),
        }
    }

    fn handle_top(&mut self, inputs: MenuInputs, settings: &Settings) -> Option<PauseAction> {
        if inputs.btn2 {
            self.open = false;
            return Some(PauseAction::Resume);
        }
        if inputs.up {
            self.top_selected = if self.top_selected == 0 {
                TOP_COUNT - 1
            } else {
                self.top_selected - 1
            };
        }
        if inputs.down {
            self.top_selected = (self.top_selected + 1) % TOP_COUNT;
        }
        if inputs.left || inputs.right {
            let dir = if inputs.right { 1 } else { -1 };
            match self.top_selected {
                ITEM_MUSIC => {
                    return Some(PauseAction::SetMusicVolume(step_volume(
                        settings.music_volume,
                        dir,
                    )));
                }
                ITEM_SFX => {
                    return Some(PauseAction::SetSfxVolume(step_volume(
                        settings.sfx_volume,
                        dir,
                    )));
                }
                ITEM_FULLSCREEN => return Some(PauseAction::ToggleFullscreen),
                _ => {}
            }
        }
        if inputs.btn1 {
            match self.top_selected {
                ITEM_CONTINUE => {
                    self.open = false;
                    return Some(PauseAction::Resume);
                }
                ITEM_FULLSCREEN => return Some(PauseAction::ToggleFullscreen),
                ITEM_INPUT => self.view = View::InputBindings,
                ITEM_CLEAR => {
                    self.view = View::ConfirmClearSave;
                    self.confirm_selected = 0;
                }
                ITEM_RESET => {
                    self.open = false;
                    return Some(PauseAction::ResetGame);
                }
                ITEM_QUIT => return Some(PauseAction::Quit),
                _ => {}
            }
        }
        None
    }

    fn handle_input_view(&mut self, inputs: MenuInputs) -> Option<PauseAction> {
        // Any confirm/back returns to Top. Up/Down are no-ops here:
        // the only "live" item is Back.
        if inputs.btn1 || inputs.btn2 {
            self.view = View::Top;
        }
        None
    }

    fn handle_confirm_clear(&mut self, inputs: MenuInputs) -> Option<PauseAction> {
        if inputs.btn2 {
            self.view = View::Top;
            return None;
        }
        if inputs.up || inputs.down {
            self.confirm_selected = (self.confirm_selected + 1) % 2;
        }
        if inputs.btn1 {
            let confirmed = self.confirm_selected == 1;
            self.view = View::Top;
            if confirmed {
                return Some(PauseAction::ClearSave);
            }
        }
        None
    }

    pub fn just_opened(&self) -> bool {
        self.open && !self.last_open
    }

    pub fn just_closed(&self) -> bool {
        !self.open && self.last_open
    }

    pub fn draw<D: RaylibDraw>(&self, d: &mut D, font: &Font, settings: &Settings) {
        d.draw_rectangle(
            0,
            0,
            GAME_WIDTH as i32,
            GAME_HEIGHT as i32,
            palette::color(Pal::Black).alpha(0.8),
        );
        let border_padding = 4;
        d.draw_rectangle_lines(
            border_padding,
            border_padding,
            GAME_WIDTH as i32 - border_padding * 2,
            GAME_HEIGHT as i32 - border_padding * 2,
            palette::color(Pal::White),
        );

        let size = crate::font::MONOGRAM_SIZE as f32;
        let title = match self.view {
            View::Top => "PAUSED",
            View::InputBindings => "INPUT",
            View::ConfirmClearSave => "CLEAR SAVE?",
        };
        let title_m = font.measure_text(title, size, 0.0);
        let title_x = ((GAME_WIDTH - title_m.x) * 0.5).round();
        let title_y = 16.0;
        d.draw_text_ex(
            font,
            title,
            Vector2::new(title_x, title_y),
            size,
            0.0,
            palette::color(Pal::White),
        );

        match self.view {
            View::Top => self.draw_top(d, font, settings, title_y + size + 8.0),
            View::InputBindings => self.draw_input_view(d, font, title_y + size + 8.0),
            View::ConfirmClearSave => self.draw_confirm_clear(d, font, title_y + size + 8.0),
        }
    }

    fn draw_top<D: RaylibDraw>(&self, d: &mut D, font: &Font, settings: &Settings, mut y: f32) {
        let size = crate::font::MONOGRAM_SIZE as f32;
        let line_h = size + 4.0;
        // Pull text away from the left edge enough that the indicator
        // circle has somewhere to sit.
        let item_x = 32.0_f32;

        let labels: [String; TOP_COUNT] = [
            "Continue".to_string(),
            "Music:".to_string(),
            "SFX:".to_string(),
            format!(
                "Fullscreen: {}",
                if settings.fullscreen { "On" } else { "Off" }
            ),
            "Input".to_string(),
            "Clear Save Data".to_string(),
            "Reset Game".to_string(),
            "Quit".to_string(),
        ];

        for (i, text) in labels.iter().enumerate() {
            d.draw_text_ex(
                font,
                text,
                Vector2::new(item_x, y),
                size,
                0.0,
                palette::color(Pal::White),
            );
            // Music / SFX rows: render bars after the label so the
            // selection cursor lines up with the row, not the label edge.
            match i {
                ITEM_MUSIC => {
                    let label_m = font.measure_text(text, size, 0.0);
                    draw_volume_bars(d, font, item_x + label_m.x + 6.0, y, settings.music_volume);
                }
                ITEM_SFX => {
                    let label_m = font.measure_text(text, size, 0.0);
                    draw_volume_bars(d, font, item_x + label_m.x + 6.0, y, settings.sfx_volume);
                }
                _ => {}
            }
            if i == self.top_selected {
                self.draw_indicator(d, item_x, y + size * 0.5);
            }
            y += line_h;
        }
    }

    fn draw_input_view<D: RaylibDraw>(&self, d: &mut D, font: &Font, mut y: f32) {
        let size = crate::font::MONOGRAM_SIZE as f32;
        let line_h = size + 2.0;
        // Match the top view's item_x so the Back item's indicator
        // circle has clearance from the white frame border on the left.
        let item_x = 32.0_f32;

        for (name, body) in binding_descriptions().iter() {
            let line = format!("{name}: {body}");
            d.draw_text_ex(
                font,
                &line,
                Vector2::new(item_x, y),
                size,
                0.0,
                palette::color(Pal::White),
            );
            y += line_h;
        }
        y += 4.0;
        let back = "< Back";
        d.draw_text_ex(
            font,
            back,
            Vector2::new(item_x, y),
            size,
            0.0,
            palette::color(Pal::White),
        );
        self.draw_indicator(d, item_x, y + size * 0.5);
    }

    fn draw_confirm_clear<D: RaylibDraw>(&self, d: &mut D, font: &Font, mut y: f32) {
        let size = crate::font::MONOGRAM_SIZE as f32;
        let line_h = size + 6.0;
        let item_x = 32.0_f32;

        let prompt = "Wipe all save data for this game?";
        let prompt_m = font.measure_text(prompt, size, 0.0);
        let prompt_x = ((GAME_WIDTH - prompt_m.x) * 0.5).round();
        d.draw_text_ex(
            font,
            prompt,
            Vector2::new(prompt_x, y),
            size,
            0.0,
            palette::color(Pal::White),
        );
        y += line_h * 1.5;

        let labels = ["No, cancel", "Yes, clear save data"];
        for (i, text) in labels.iter().enumerate() {
            d.draw_text_ex(
                font,
                text,
                Vector2::new(item_x, y),
                size,
                0.0,
                palette::color(Pal::White),
            );
            if i == self.confirm_selected {
                self.draw_indicator(d, item_x, y + size * 0.5);
            }
            y += line_h;
        }
    }

    fn draw_indicator<D: RaylibDraw>(&self, d: &mut D, item_x: f32, center_y: f32) {
        let amplitude = 1.5_f32;
        let speed = 6.0_f32;
        let osc = (self.time * speed).sin() * amplitude;
        let cx = item_x - 8.0 + osc;
        d.draw_circle(cx as i32, center_y as i32, 2.0, palette::color(Pal::White));
    }
}

impl Default for PauseMenu {
    fn default() -> Self {
        Self::new()
    }
}

fn step_volume(current: f32, dir: i32) -> f32 {
    let snapped = (current * VOLUME_STEPS).round() / VOLUME_STEPS;
    let next = snapped + dir as f32 * VOLUME_STEP;
    next.clamp(0.0, 1.0)
}

fn volume_bars_filled(v: f32) -> usize {
    (v.clamp(0.0, 1.0) * VOLUME_STEPS).round() as usize
}

fn draw_volume_bars<D: RaylibDraw>(d: &mut D, font: &Font, x: f32, y: f32, v: f32) {
    let cell_w = 6.0_f32;
    let cell_h = (crate::font::MONOGRAM_SIZE as f32 * 0.7).round();
    let gap = 2.0_f32;
    let cell_top = y + (crate::font::MONOGRAM_SIZE as f32 - cell_h) * 0.5;
    let filled = volume_bars_filled(v);
    let total = VOLUME_STEPS as usize;
    let color = palette::color(Pal::White);
    for i in 0..total {
        let cx = x + (i as f32) * (cell_w + gap);
        if i < filled {
            d.draw_rectangle(
                cx as i32,
                cell_top as i32,
                cell_w as i32,
                cell_h as i32,
                color,
            );
        } else {
            d.draw_rectangle_lines(
                cx as i32,
                cell_top as i32,
                cell_w as i32,
                cell_h as i32,
                color,
            );
        }
    }
    let pct = (v.clamp(0.0, 1.0) * 100.0).round() as i32;
    let pct_text = format!("{pct}%");
    let bars_w = (total as f32) * cell_w + ((total - 1) as f32) * gap;
    d.draw_text_ex(
        font,
        &pct_text,
        Vector2::new(x + bars_w + 6.0, y),
        crate::font::MONOGRAM_SIZE as f32,
        0.0,
        color,
    );
}

#[derive(Default, Clone, Copy)]
struct MenuInputs {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
    btn1: bool,
    btn2: bool,
    /// Open/close key (Esc/Enter-without-Alt/P/gamepad Start).
    toggle: bool,
}

fn read_inputs(rl: &RaylibHandle) -> MenuInputs {
    // Enter alone toggles, but Alt+Enter is reserved for fullscreen.
    let alt_held =
        rl.is_key_down(KeyboardKey::KEY_LEFT_ALT) || rl.is_key_down(KeyboardKey::KEY_RIGHT_ALT);
    let toggle = rl.is_key_pressed(KeyboardKey::KEY_ESCAPE)
        || rl.is_key_pressed(KeyboardKey::KEY_P)
        || (rl.is_key_pressed(KeyboardKey::KEY_ENTER) && !alt_held)
        || gamepad_start_pressed(rl);
    MenuInputs {
        up: input::action_pressed(rl, ACTION_UP),
        down: input::action_pressed(rl, ACTION_DOWN),
        left: input::action_pressed(rl, ACTION_LEFT),
        right: input::action_pressed(rl, ACTION_RIGHT),
        btn1: input::action_pressed(rl, ACTION_BTN1),
        btn2: input::action_pressed(rl, ACTION_BTN2),
        toggle,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn step_volume_walks_six_levels() {
        let mut v = 0.0;
        for expected in [0.2, 0.4, 0.6, 0.8, 1.0, 1.0] {
            v = step_volume(v, 1);
            assert!((v - expected).abs() < 1e-5, "got {v} expected {expected}");
        }
        for expected in [0.8, 0.6, 0.4, 0.2, 0.0, 0.0] {
            v = step_volume(v, -1);
            assert!((v - expected).abs() < 1e-5, "got {v} expected {expected}");
        }
    }

    #[test]
    fn step_volume_snaps_offgrid_value() {
        // 0.55 should snap to 0.6 before stepping; +1 → 0.8.
        let v = step_volume(0.55, 1);
        assert!((v - 0.8).abs() < 1e-5);
    }

    #[test]
    fn volume_bars_filled_maps_each_step() {
        assert_eq!(volume_bars_filled(0.0), 0);
        assert_eq!(volume_bars_filled(0.2), 1);
        assert_eq!(volume_bars_filled(0.4), 2);
        assert_eq!(volume_bars_filled(0.6), 3);
        assert_eq!(volume_bars_filled(0.8), 4);
        assert_eq!(volume_bars_filled(1.0), 5);
    }

    #[test]
    fn toggle_opens_and_closes_menu() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        let action = m.update_with(toggle(), &s, 0.016);
        assert!(m.open);
        assert_eq!(m.view, View::Top);
        assert_eq!(action, None);
        let action = m.update_with(toggle(), &s, 0.016);
        assert!(!m.open);
        assert_eq!(action, Some(PauseAction::Resume));
    }

    #[test]
    fn down_then_up_wraps_through_top_items() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        m.update_with(toggle(), &s, 0.016);
        // Up from the first item should wrap to the last.
        m.update_with(up(), &s, 0.016);
        assert_eq!(m.top_selected, TOP_COUNT - 1);
        // Down from the last wraps back to 0.
        m.update_with(down(), &s, 0.016);
        assert_eq!(m.top_selected, 0);
    }

    #[test]
    fn left_right_on_music_emits_set_music_volume() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        m.update_with(toggle(), &s, 0.016);
        m.update_with(down(), &s, 0.016);
        assert_eq!(m.top_selected, ITEM_MUSIC);
        let action = m.update_with(right(), &s, 0.016);
        match action {
            Some(PauseAction::SetMusicVolume(v)) => {
                assert!((v - 1.0).abs() < 1e-5);
            }
            other => panic!("expected SetMusicVolume, got {other:?}"),
        }
        let action = m.update_with(left(), &s, 0.016);
        match action {
            Some(PauseAction::SetMusicVolume(v)) => {
                assert!((v - 0.6).abs() < 1e-5);
            }
            other => panic!("expected SetMusicVolume, got {other:?}"),
        }
    }

    #[test]
    fn left_right_on_fullscreen_emits_toggle() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        m.update_with(toggle(), &s, 0.016);
        for _ in 0..3 {
            m.update_with(down(), &s, 0.016);
        }
        assert_eq!(m.top_selected, ITEM_FULLSCREEN);
        assert_eq!(
            m.update_with(right(), &s, 0.016),
            Some(PauseAction::ToggleFullscreen)
        );
        assert_eq!(
            m.update_with(left(), &s, 0.016),
            Some(PauseAction::ToggleFullscreen)
        );
        assert_eq!(
            m.update_with(btn1(), &s, 0.016),
            Some(PauseAction::ToggleFullscreen)
        );
    }

    #[test]
    fn confirm_clear_defaults_to_no_and_cancels_on_btn1() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        m.update_with(toggle(), &s, 0.016);
        for _ in 0..ITEM_CLEAR {
            m.update_with(down(), &s, 0.016);
        }
        assert_eq!(m.top_selected, ITEM_CLEAR);
        assert_eq!(m.update_with(btn1(), &s, 0.016), None);
        assert_eq!(m.view, View::ConfirmClearSave);
        assert_eq!(m.confirm_selected, 0); // No is default
        // BTN1 on No returns to top with no action.
        assert_eq!(m.update_with(btn1(), &s, 0.016), None);
        assert_eq!(m.view, View::Top);
    }

    #[test]
    fn confirm_clear_yes_emits_clear_save() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        m.update_with(toggle(), &s, 0.016);
        for _ in 0..ITEM_CLEAR {
            m.update_with(down(), &s, 0.016);
        }
        m.update_with(btn1(), &s, 0.016);
        // Move to "Yes, clear save data" and confirm.
        m.update_with(down(), &s, 0.016);
        assert_eq!(m.confirm_selected, 1);
        assert_eq!(
            m.update_with(btn1(), &s, 0.016),
            Some(PauseAction::ClearSave)
        );
        assert_eq!(m.view, View::Top);
    }

    #[test]
    fn btn2_in_confirm_returns_to_top() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        m.update_with(toggle(), &s, 0.016);
        for _ in 0..ITEM_CLEAR {
            m.update_with(down(), &s, 0.016);
        }
        m.update_with(btn1(), &s, 0.016);
        assert_eq!(m.view, View::ConfirmClearSave);
        assert_eq!(m.update_with(btn2(), &s, 0.016), None);
        assert_eq!(m.view, View::Top);
    }

    #[test]
    fn input_view_back_via_btn1_or_btn2() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        m.update_with(toggle(), &s, 0.016);
        for _ in 0..ITEM_INPUT {
            m.update_with(down(), &s, 0.016);
        }
        m.update_with(btn1(), &s, 0.016);
        assert_eq!(m.view, View::InputBindings);
        m.update_with(btn1(), &s, 0.016);
        assert_eq!(m.view, View::Top);
        // And BTN2 also exits.
        m.update_with(btn1(), &s, 0.016);
        assert_eq!(m.view, View::InputBindings);
        m.update_with(btn2(), &s, 0.016);
        assert_eq!(m.view, View::Top);
    }

    #[test]
    fn reset_and_quit_emit_their_actions() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        m.update_with(toggle(), &s, 0.016);
        for _ in 0..ITEM_RESET {
            m.update_with(down(), &s, 0.016);
        }
        assert_eq!(
            m.update_with(btn1(), &s, 0.016),
            Some(PauseAction::ResetGame)
        );
        // Reset also resumes the game so the player isn't stuck in
        // the menu staring at the freshly-init'd state.
        assert!(!m.open, "Reset Game should close the menu");

        // Re-open and walk to Quit to confirm it still emits.
        m.update_with(toggle(), &s, 0.016);
        for _ in 0..ITEM_QUIT {
            m.update_with(down(), &s, 0.016);
        }
        assert_eq!(m.top_selected, ITEM_QUIT);
        assert_eq!(m.update_with(btn1(), &s, 0.016), Some(PauseAction::Quit));
    }

    #[test]
    fn toggle_from_submenu_closes_whole_menu() {
        let mut m = PauseMenu::new();
        let s = Settings::default();
        m.update_with(toggle(), &s, 0.016);
        for _ in 0..ITEM_INPUT {
            m.update_with(down(), &s, 0.016);
        }
        m.update_with(btn1(), &s, 0.016);
        assert_eq!(m.view, View::InputBindings);
        let action = m.update_with(toggle(), &s, 0.016);
        assert!(!m.open);
        assert_eq!(action, Some(PauseAction::Resume));
    }
}
