//! Pause menu. Pico-8-style overlay: top-level item list (volumes,
//! fullscreen, Input sub-menu, Clear Save, Reset Game, Quit) plus the
//! Input Test + Key Config flows.
//!
//! UI lives here; side effects (settings write, fullscreen toggle,
//! `_init`, save clear, quit, keymap write) are dispatched via the
//! returned `PauseAction`. Keeps this module from holding a session
//! god-handle and makes navigation testable as a pure transition.

use crate::input::{
    self, ACTION_BTN1, ACTION_BTN2, ACTION_BTN3, ACTION_DOWN, ACTION_LEFT, ACTION_NAMES,
    ACTION_RIGHT, ACTION_UP, GamepadFamily, MAX_GAMEPADS, binding_columns,
};
use crate::keymap::{self, Keymap};
use crate::palette;
use crate::palette::Pal;
use crate::settings::Settings;
use crate::{GAME_HEIGHT, GAME_WIDTH};
use sola_raylib::prelude::*;

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

const INPUT_MENU_COUNT: usize = 2;
const INPUT_ITEM_TEST: usize = 0;
const INPUT_ITEM_CONFIGURE: usize = 1;

/// In-flight Key Config capture. Mutated as the player presses keys;
/// emitted via `PauseAction::SetKeymap` on completion.
#[derive(Debug, Clone)]
struct KeyConfigState {
    staging: Keymap,
    /// Index (0..ACTION_COUNT) of the action currently awaiting a key.
    action_index: usize,
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

const ACTION_COUNT: usize = 7;

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
        dt: f32,
    ) -> Option<PauseAction> {
        let family = input::current_gamepad_family(rl);
        let inputs = read_inputs(rl, keymap, family);

        // Snapshot for the Tester rects so `draw` doesn't need `rl`.
        for i in 0..ACTION_COUNT {
            self.tester_input[i] = input::action_down(rl, keymap, family, (i + 1) as u32);
        }

        // Only drain raylib's key queue while capturing, so presses on
        // other views aren't silently consumed.
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

        self.update_with(inputs, settings, keymap, kc_inputs, dt)
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
                ITEM_INPUT => {
                    self.view = View::InputMenu;
                    self.input_menu_selected = 0;
                }
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

    fn handle_input_menu(&mut self, inputs: MenuInputs, keymap: &Keymap) -> Option<PauseAction> {
        if inputs.btn2 {
            self.view = View::Top;
            return None;
        }
        if inputs.up || inputs.down {
            self.input_menu_selected = (self.input_menu_selected + 1) % INPUT_MENU_COUNT;
        }
        if inputs.btn1 {
            match self.input_menu_selected {
                INPUT_ITEM_TEST => self.view = View::InputTester,
                INPUT_ITEM_CONFIGURE => {
                    self.view = View::KeyConfig;
                    self.key_config = Some(KeyConfigState {
                        staging: keymap.clone(),
                        action_index: 0,
                    });
                }
                _ => {}
            }
        }
        None
    }

    fn handle_input_tester(&mut self, _inputs: MenuInputs) -> Option<PauseAction> {
        // Action buttons aren't consumed here: they're being tested.
        // Only the toggle key (handled centrally) returns to InputMenu.
        None
    }

    fn handle_key_config(
        &mut self,
        _inputs: MenuInputs,
        kc: KeyConfigInputs,
    ) -> Option<PauseAction> {
        // DEL: Pico-8-style "reset to defaults" + exit. Single emit so
        // the session writes once.
        if kc.delete {
            self.view = View::InputTester;
            self.key_config = None;
            return Some(PauseAction::SetKeymap(Keymap::default()));
        }
        // BKSP undoes the last capture: step back, clear that slot.
        // No-op at action 0.
        if kc.backspace {
            if let Some(state) = self.key_config.as_mut()
                && state.action_index > 0
            {
                state.action_index -= 1;
                state.staging.overrides[state.action_index] = None;
            }
            return None;
        }
        let key = kc.captured_key?;
        let state = self.key_config.as_mut()?;
        if state.action_index >= ACTION_COUNT {
            return None;
        }
        // Exclusive mappings: reject if this key is already in another
        // slot. Player stays on the current action until they pick a
        // free key (or Backspace to revisit the conflicting slot).
        let already_used = state
            .staging
            .overrides
            .iter()
            .enumerate()
            .any(|(i, slot)| i != state.action_index && *slot == Some(key));
        if already_used {
            return None;
        }
        state.staging.overrides[state.action_index] = Some(key);
        state.action_index += 1;
        if state.action_index >= ACTION_COUNT {
            // Done. Drop the player on the Tester so they can verify
            // their new bindings live.
            let staged = state.staging.clone();
            self.view = View::InputTester;
            self.key_config = None;
            return Some(PauseAction::SetKeymap(staged));
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

    pub fn draw<D: RaylibDraw>(
        &self,
        d: &mut D,
        font: &Font,
        settings: &Settings,
        keymap: &Keymap,
        gamepad_family: GamepadFamily,
    ) {
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
            View::InputMenu => "INPUT",
            View::InputTester => "INPUT TEST",
            View::KeyConfig => "KEY CONFIG (KEYBOARD)",
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

        let body_y = title_y + size + 8.0;
        match self.view {
            View::Top => self.draw_top(d, font, settings, body_y),
            View::InputMenu => self.draw_input_menu(d, font, body_y),
            View::InputTester => self.draw_input_tester(d, font, keymap, gamepad_family, body_y),
            View::KeyConfig => self.draw_key_config(d, font, body_y),
            View::ConfirmClearSave => self.draw_confirm_clear(d, font, body_y),
        }
    }

    fn draw_input_menu<D: RaylibDraw>(&self, d: &mut D, font: &Font, mut y: f32) {
        let size = crate::font::MONOGRAM_SIZE as f32;
        let line_h = size + 6.0;
        let item_x = 32.0_f32;
        let labels = ["Test", "Configure Keys"];
        for (i, text) in labels.iter().enumerate() {
            d.draw_text_ex(
                font,
                text,
                Vector2::new(item_x, y),
                size,
                0.0,
                palette::color(Pal::White),
            );
            if i == self.input_menu_selected {
                self.draw_indicator(d, item_x, y + size * 0.5);
            }
            y += line_h;
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

    fn draw_input_tester<D: RaylibDraw>(
        &self,
        d: &mut D,
        font: &Font,
        keymap: &Keymap,
        gamepad_family: GamepadFamily,
        body_y: f32,
    ) {
        let size = crate::font::MONOGRAM_SIZE as f32;
        let white = palette::color(Pal::White);
        let black = palette::color(Pal::Black);

        // BTN cells are larger than D-pad cells so a centered "1"/"2"/
        // "3" digit fits without clipping. Cluster centers above the
        // mapping list.
        let dpad_cell = 10.0_f32;
        let btn_cell = 12.0_f32;
        let gap = 2.0_f32;
        let dpad_w = dpad_cell * 3.0 + gap * 2.0;
        let btn_w = btn_cell * 3.0 + gap * 2.0;
        let cluster_gap = 16.0_f32;
        let cluster_total = dpad_w + cluster_gap + btn_w;
        let dpad_x = ((GAME_WIDTH - cluster_total) * 0.5).round();
        let dpad_y = body_y;

        let draw_box = |d: &mut D, x: f32, y: f32, w: f32, on: bool| {
            if on {
                d.draw_rectangle(x as i32, y as i32, w as i32, w as i32, white);
            } else {
                d.draw_rectangle_lines(x as i32, y as i32, w as i32, w as i32, white);
            }
        };

        // D-pad layout:
        //   . U .
        //   L . R
        //   . D .
        let dpad_mid_x = dpad_x + dpad_cell + gap;
        let dpad_mid_y = dpad_y + dpad_cell + gap;
        draw_box(
            d,
            dpad_mid_x,
            dpad_y,
            dpad_cell,
            self.tester_input[ACTION_UP as usize - 1],
        );
        draw_box(
            d,
            dpad_x,
            dpad_mid_y,
            dpad_cell,
            self.tester_input[ACTION_LEFT as usize - 1],
        );
        draw_box(
            d,
            dpad_x + (dpad_cell + gap) * 2.0,
            dpad_mid_y,
            dpad_cell,
            self.tester_input[ACTION_RIGHT as usize - 1],
        );
        draw_box(
            d,
            dpad_mid_x,
            dpad_y + (dpad_cell + gap) * 2.0,
            dpad_cell,
            self.tester_input[ACTION_DOWN as usize - 1],
        );

        // Buttons: row vertically centered against the D-pad so the
        // cluster reads like a gamepad face. Numbered for clarity.
        let btn_x = dpad_x + dpad_w + cluster_gap;
        let dpad_h = dpad_cell * 3.0 + gap * 2.0;
        let btn_y = dpad_y + (dpad_h - btn_cell) * 0.5;
        let btn_cells = [
            (btn_x, ACTION_BTN1, "1"),
            (btn_x + btn_cell + gap, ACTION_BTN2, "2"),
            (btn_x + (btn_cell + gap) * 2.0, ACTION_BTN3, "3"),
        ];
        for (cx, action, label) in btn_cells {
            let on = self.tester_input[action as usize - 1];
            draw_box(d, cx, btn_y, btn_cell, on);
            // Centered digit; black on filled, white on outlined so
            // it always contrasts.
            let label_m = font.measure_text(label, size, 0.0);
            let tx = cx + (btn_cell - label_m.x) * 0.5;
            let ty = btn_y + (btn_cell - size) * 0.5;
            d.draw_text_ex(
                font,
                label,
                Vector2::new(tx.round(), ty.round()),
                size,
                0.0,
                if on { black } else { white },
            );
        }

        // 3-column mapping table (action / keyboard / gamepad) so
        // "where's BTN1 on my keyboard?" reads at a glance.
        let cluster_bottom = dpad_y + dpad_h;
        let list_line_h = size;
        let mut list_y = cluster_bottom + 6.0;
        let name_x = 48.0_f32;
        let kb_x = 92.0_f32;
        let gp_x = 144.0_f32;
        for (name, kb, gp) in binding_columns(keymap, gamepad_family).iter() {
            d.draw_text_ex(font, name, Vector2::new(name_x, list_y), size, 0.0, white);
            d.draw_text_ex(font, kb, Vector2::new(kb_x, list_y), size, 0.0, white);
            d.draw_text_ex(font, gp, Vector2::new(gp_x, list_y), size, 0.0, white);
            list_y += list_line_h;
        }

        // Action buttons aren't consumed by the Tester, so the only
        // way back is toggle. Mention both Esc and Start so gamepad-
        // only players see a path out.
        let footer = "ESC OR START TO BACK";
        let footer_m = font.measure_text(footer, size, 0.0);
        let footer_x = ((GAME_WIDTH - footer_m.x) * 0.5).round();
        let footer_y = GAME_HEIGHT - size - 4.0;
        d.draw_text_ex(
            font,
            footer,
            Vector2::new(footer_x, footer_y),
            size,
            0.0,
            white,
        );
    }

    fn draw_key_config<D: RaylibDraw>(&self, d: &mut D, font: &Font, mut y: f32) {
        let size = crate::font::MONOGRAM_SIZE as f32;
        let line_h = size + 2.0;
        let color = palette::color(Pal::White);

        let Some(state) = self.key_config.as_ref() else {
            // Defensive: if state ever desyncs, show a clear message
            // instead of a blank pane.
            d.draw_text_ex(
                font,
                "(no capture in progress)",
                Vector2::new(32.0, y),
                size,
                0.0,
                color,
            );
            return;
        };

        let prompt = if state.action_index < ACTION_COUNT {
            format!("Press key for: {}", ACTION_NAMES[state.action_index])
        } else {
            "Capture complete".to_string()
        };
        let prompt_m = font.measure_text(&prompt, size, 0.0);
        let prompt_x = ((GAME_WIDTH - prompt_m.x) * 0.5).round();
        d.draw_text_ex(font, &prompt, Vector2::new(prompt_x, y), size, 0.0, color);
        y += line_h * 1.5;

        // Center the staged list by measuring the widest row and
        // parking the column there.
        let entries: Vec<(usize, &'static str, &'static str)> = ACTION_NAMES
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let label = state
                    .staging
                    .overrides
                    .get(i)
                    .copied()
                    .flatten()
                    .and_then(keymap::key_label)
                    .unwrap_or("--");
                (i, *name, label)
            })
            .collect();
        let widest = entries
            .iter()
            .map(|(_, name, label)| font.measure_text(&format!("{name}: {label}"), size, 0.0).x)
            .fold(0.0_f32, f32::max);
        let item_x = ((GAME_WIDTH - widest) * 0.5).round();

        for (i, name, label) in entries {
            let line = format!("{name}: {label}");
            // Highlight the current row so the eye snaps to it
            // without parsing the header.
            if i == state.action_index {
                d.draw_rectangle(
                    item_x as i32 - 4,
                    y as i32 - 1,
                    widest as i32 + 8,
                    line_h as i32,
                    palette::color(Pal::White).alpha(0.25),
                );
            }
            d.draw_text_ex(font, &line, Vector2::new(item_x, y), size, 0.0, color);
            y += line_h;
        }

        let footer = "ESC CANCEL  -  BKSP UNDO  -  DEL RESET";
        let footer_m = font.measure_text(footer, size, 0.0);
        let footer_x = ((GAME_WIDTH - footer_m.x) * 0.5).round();
        let footer_y = GAME_HEIGHT - size - 8.0;
        d.draw_text_ex(
            font,
            footer,
            Vector2::new(footer_x, footer_y),
            size,
            0.0,
            color,
        );
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

/// Raw key state for Key Config capture/undo. Bundled to keep
/// `update_with`'s arity reasonable.
#[derive(Default, Clone, Copy)]
struct KeyConfigInputs {
    captured_key: Option<KeyboardKey>,
    delete: bool,
    backspace: bool,
}

fn read_inputs(rl: &RaylibHandle, keymap: &Keymap, family: GamepadFamily) -> MenuInputs {
    // Enter alone toggles, but Alt+Enter is reserved for fullscreen.
    let alt_held =
        rl.is_key_down(KeyboardKey::KEY_LEFT_ALT) || rl.is_key_down(KeyboardKey::KEY_RIGHT_ALT);
    let toggle = rl.is_key_pressed(KeyboardKey::KEY_ESCAPE)
        || rl.is_key_pressed(KeyboardKey::KEY_P)
        || (rl.is_key_pressed(KeyboardKey::KEY_ENTER) && !alt_held)
        || gamepad_start_pressed(rl);
    MenuInputs {
        up: input::action_pressed(rl, keymap, family, ACTION_UP),
        down: input::action_pressed(rl, keymap, family, ACTION_DOWN),
        left: input::action_pressed(rl, keymap, family, ACTION_LEFT),
        right: input::action_pressed(rl, keymap, family, ACTION_RIGHT),
        btn1: input::action_pressed(rl, keymap, family, ACTION_BTN1),
        btn2: input::action_pressed(rl, keymap, family, ACTION_BTN2),
        toggle,
    }
}

/// Keys that capture refuses to bind: menu controls (Esc/Enter), the
/// reset gesture (Delete), the undo gesture (Backspace), and keys with
/// system meaning (F-keys, modifiers).
fn is_reserved_key(k: KeyboardKey) -> bool {
    matches!(
        k,
        KeyboardKey::KEY_ESCAPE
            | KeyboardKey::KEY_ENTER
            | KeyboardKey::KEY_DELETE
            | KeyboardKey::KEY_BACKSPACE
            | KeyboardKey::KEY_LEFT_SHIFT
            | KeyboardKey::KEY_RIGHT_SHIFT
            | KeyboardKey::KEY_LEFT_CONTROL
            | KeyboardKey::KEY_RIGHT_CONTROL
            | KeyboardKey::KEY_LEFT_ALT
            | KeyboardKey::KEY_RIGHT_ALT
            | KeyboardKey::KEY_LEFT_SUPER
            | KeyboardKey::KEY_RIGHT_SUPER
            | KeyboardKey::KEY_F1
            | KeyboardKey::KEY_F2
            | KeyboardKey::KEY_F3
            | KeyboardKey::KEY_F4
            | KeyboardKey::KEY_F5
            | KeyboardKey::KEY_F6
            | KeyboardKey::KEY_F7
            | KeyboardKey::KEY_F8
            | KeyboardKey::KEY_F9
            | KeyboardKey::KEY_F10
            | KeyboardKey::KEY_F11
            | KeyboardKey::KEY_F12
    )
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

    #[test]
    fn reserved_keys_are_skipped_in_capture_filter() {
        // The raw read in `update` filters reserved keys out, but the
        // `is_reserved_key` predicate is the contract. Sanity-check it
        // covers the menu's must-not-bind keys.
        assert!(is_reserved_key(KeyboardKey::KEY_ESCAPE));
        assert!(is_reserved_key(KeyboardKey::KEY_ENTER));
        assert!(is_reserved_key(KeyboardKey::KEY_DELETE));
        assert!(is_reserved_key(KeyboardKey::KEY_F5));
        assert!(is_reserved_key(KeyboardKey::KEY_LEFT_SHIFT));
        assert!(!is_reserved_key(KeyboardKey::KEY_W));
        assert!(!is_reserved_key(KeyboardKey::KEY_SPACE));
    }
}
