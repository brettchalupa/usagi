//! Input sub-menu under the Top view: lists "Test" and "Configure
//! Keys" so the Tester scene can stay free of Configure shortcuts
//! that would be confusing to test against.

use super::PauseAction;
use super::PauseMenu;
use super::View;
use super::draw_indicator;
use super::inputs::MenuInputs;
use super::key_config::KeyConfigState;
use crate::keymap::Keymap;
use crate::palette;
use crate::palette::Pal;
use sola_raylib::prelude::*;

pub(super) const INPUT_MENU_COUNT: usize = 2;
pub(super) const INPUT_ITEM_TEST: usize = 0;
pub(super) const INPUT_ITEM_CONFIGURE: usize = 1;

impl PauseMenu {
    pub(super) fn handle_input_menu(
        &mut self,
        inputs: MenuInputs,
        keymap: &Keymap,
    ) -> Option<PauseAction> {
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

    pub(super) fn draw_input_menu<D: RaylibDraw>(&self, d: &mut D, font: &Font, mut y: f32) {
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
                draw_indicator(d, self.time, item_x, y + size * 0.5);
            }
            y += line_h;
        }
    }
}
