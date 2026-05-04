//! Pause menu. Currently very simple but a foundation a menu pause overlay
//! (volume, input remap, registered hooks); right now it's just a black screen
//! that pauses the game's drawing and updating until the player closes it.

use crate::input::{self, ACTION_BTN2, MAX_GAMEPADS};
use crate::palette;
use crate::palette::Pal;
use crate::settings::Settings;
use crate::{GAME_HEIGHT, GAME_WIDTH};
use sola_raylib::prelude::*;

pub struct PauseMenu {
    pub open: bool,
    last_open: bool,
}

impl PauseMenu {
    pub fn new() -> Self {
        Self {
            open: false,
            last_open: false,
        }
    }

    /// Handles input for opening the Pause Menu and processing input when open
    pub fn update(&mut self, rl: &RaylibHandle) {
        self.last_open = self.open;
        // Enter alone toggles, but Alt+Enter is reserved for fullscreen.
        let alt_held =
            rl.is_key_down(KeyboardKey::KEY_LEFT_ALT) || rl.is_key_down(KeyboardKey::KEY_RIGHT_ALT);
        let toggle = rl.is_key_pressed(KeyboardKey::KEY_ESCAPE)
            || rl.is_key_pressed(KeyboardKey::KEY_P)
            || (rl.is_key_pressed(KeyboardKey::KEY_ENTER) && !alt_held)
            || gamepad_start_pressed(rl);

        if self.open {
            if toggle || input::action_pressed(rl, ACTION_BTN2) {
                self.open = false;
            }
        } else if toggle {
            self.open = true;
        }
    }

    /// Renders the pause menu overlay into the active texture-mode
    /// draw handle. `settings` is read-only here; when interactive
    /// editing lands, swap to `&mut`.
    pub fn draw<D: RaylibDraw>(&self, d: &mut D, font: &Font, settings: &Settings) {
        d.draw_rectangle(
            0,
            0,
            GAME_WIDTH as i32,
            GAME_HEIGHT as i32,
            palette::color(Pal::Black).alpha(0.7),
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
        let title_m = font.measure_text("PAUSED", size, 0.0);
        let title_x = ((GAME_WIDTH - title_m.x) * 0.5).round();
        let title_y = 20.;
        d.draw_text_ex(
            font,
            "PAUSED",
            Vector2::new(title_x, title_y),
            size,
            0.0,
            palette::color(Pal::White),
        );

        let volume_pct = (settings.volume.clamp(0.0, 1.0) * 100.0).round() as i32;
        let volume_line = format!("Volume: {volume_pct}%");
        let volume_m = font.measure_text(&volume_line, size, 0.0);
        let volume_x = ((GAME_WIDTH - volume_m.x) * 0.5).round();
        let volume_y = title_y + size + 8.0;
        d.draw_text_ex(
            font,
            &volume_line,
            Vector2::new(volume_x, volume_y),
            size,
            0.0,
            palette::color(Pal::White),
        );

        let fs_label = if settings.fullscreen { "On" } else { "Off" };
        let fs_line = format!("Fullscreen: {fs_label}");
        let fs_m = font.measure_text(&fs_line, size, 0.0);
        let fs_x = ((GAME_WIDTH - fs_m.x) * 0.5).round();
        let fs_y = volume_y + size + 4.0;
        d.draw_text_ex(
            font,
            &fs_line,
            Vector2::new(fs_x, fs_y),
            size,
            0.0,
            palette::color(Pal::White),
        );
    }

    pub fn just_opened(&self) -> bool {
        self.open && !self.last_open
    }

    pub fn just_closed(&self) -> bool {
        !self.open && self.last_open
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
