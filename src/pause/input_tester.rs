//! Input Tester scene under Input. Lights up the D-pad / button rects
//! while their actions are held, then lists each action's keyboard
//! and gamepad bindings. Action buttons here are *not* consumed —
//! they're the thing being tested — so the only way out is the
//! universal toggle key (Esc / P / gamepad Start), which climbs
//! back to the Input sub-menu.

use super::PauseAction;
use super::PauseMenu;
use super::inputs::MenuInputs;
use crate::input::{
    ACTION_BTN1, ACTION_BTN2, ACTION_BTN3, ACTION_DOWN, ACTION_LEFT, ACTION_RIGHT, ACTION_UP,
    GamepadFamily, binding_columns,
};
use crate::keymap::Keymap;
use crate::palette;
use crate::palette::Pal;
use sola_raylib::prelude::*;

impl PauseMenu {
    pub(super) fn handle_input_tester(&mut self, _inputs: MenuInputs) -> Option<PauseAction> {
        // Action buttons aren't consumed here: they're being tested.
        // Only the toggle key (handled centrally) returns to InputMenu.
        None
    }

    pub(super) fn draw_input_tester<D: RaylibDraw>(
        &self,
        d: &mut D,
        font: &Font,
        keymap: &Keymap,
        gamepad_family: GamepadFamily,
        body_y: f32,
        res: crate::config::Resolution,
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
        let dpad_x = ((res.w - cluster_total) * 0.5).round();
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
        let footer_x = ((res.w - footer_m.x) * 0.5).round();
        let footer_y = res.h - size - 4.0;
        d.draw_text_ex(
            font,
            footer,
            Vector2::new(footer_x, footer_y),
            size,
            0.0,
            white,
        );
    }
}
