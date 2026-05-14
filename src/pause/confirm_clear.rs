//! Confirm-clear dialog under the Top view's Clear Save Data row.
//! Defaults to "No, cancel" so accidental Enter presses don't wipe
//! the player's save.

use super::PauseAction;
use super::PauseMenu;
use super::View;
use super::inputs::MenuInputs;
use super::{draw_indicator, item_x_for};
use crate::palette;
use crate::palette::Pal;
use sola_raylib::prelude::*;

impl PauseMenu {
    pub(super) fn handle_confirm_clear(&mut self, inputs: MenuInputs) -> Option<PauseAction> {
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

    pub(super) fn draw_confirm_clear<D: RaylibDraw>(
        &self,
        d: &mut D,
        font: &Font,
        mut y: f32,
        res: crate::config::Resolution,
    ) {
        let size = crate::font::MONOGRAM_SIZE as f32;
        let line_h = size + 6.0;
        let item_x = item_x_for(res);

        let prompt = "Wipe all save data for this game?";
        let prompt_m = font.measure_text(prompt, size, 0.0);
        let prompt_x = ((res.w - prompt_m.x) * 0.5).round();
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
                draw_indicator(d, self.time, item_x, y + size * 0.5);
            }
            y += line_h;
        }
    }
}
