//! Pause menu. Currently very simple but a foundation a menu pause overlay
//! (volume, input remap, registered hooks); right now it's just a black screen
//! that pauses the game's drawing and updating until the player closes it.

use crate::input::{self, ACTION_BTN2, MAX_GAMEPADS};
use crate::{GAME_HEIGHT, GAME_WIDTH};
use sola_raylib::prelude::*;

pub struct PauseMenu {
    pub open: bool,
}

impl PauseMenu {
    pub fn new() -> Self {
        Self { open: false }
    }

    /// Toggles `open` based on this frame's input. Esc / P / gamepad
    /// Start open or close; BTN2 only closes (so the same button players
    /// use to confirm in-game can dismiss the menu without also being a
    /// way to summon it during play).
    pub fn handle_input(&mut self, rl: &RaylibHandle) {
        let toggle = rl.is_key_pressed(KeyboardKey::KEY_ESCAPE)
            || rl.is_key_pressed(KeyboardKey::KEY_P)
            || gamepad_start_pressed(rl);

        if self.open {
            if toggle || input::action_pressed(rl, ACTION_BTN2) {
                self.open = false;
            }
        } else if toggle {
            self.open = true;
        }
    }

    /// Renders the overlay into the active texture-mode draw handle.
    /// Clears the RT to black and draws "PAUSED" centered at the game's
    /// native resolution so it scales identically to in-game text.
    pub fn draw<D: RaylibDraw>(&self, d: &mut D, font: &Font) {
        let bg_w = 200;
        let bg_h = 100;
        let bg_x = GAME_WIDTH as i32 / 2 - bg_w / 2;
        let bg_y = GAME_HEIGHT as i32 / 2 - bg_h / 2;
        d.draw_rectangle(bg_x, bg_y, bg_w, bg_h, Color::BLACK);

        let size = crate::font::MONOGRAM_SIZE as f32;
        let m = font.measure_text("PAUSED", size, 0.0);
        let x = ((GAME_WIDTH - m.x) * 0.5).round();
        let y = ((GAME_HEIGHT - m.y) * 0.5).round();
        d.draw_text_ex(font, "PAUSED", Vector2::new(x, y), size, 0.0, Color::WHITE);
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
