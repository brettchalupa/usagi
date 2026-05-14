//! Top-level pause view: Continue, Settings, Clear Save Data, Reset
//! Game, Quit. Vertical list with the active row marked by an
//! oscillating indicator. Tweakable options (volumes, fullscreen,
//! input mapping) live under the Settings sub-menu so the Top stays
//! short.
//!
//! Side-effecting items (save clear, reset, quit) emit a `PauseAction`;
//! the session applies them.

use super::PauseMenu;
use super::View;
use super::inputs::MenuInputs;
use super::{PauseAction, draw_indicator, item_x_for};
use crate::palette;
use crate::palette::Pal;
use sola_raylib::prelude::*;

// Quit is hidden on web because the emscripten main loop can't
// actually exit (it's `emscripten_set_main_loop_arg`, driven by the
// browser), so the item would do nothing if we showed it.
#[cfg(not(target_os = "emscripten"))]
pub(super) const TOP_COUNT: usize = 5;
#[cfg(target_os = "emscripten")]
pub(super) const TOP_COUNT: usize = 4;
pub(super) const ITEM_CONTINUE: usize = 0;
pub(super) const ITEM_SETTINGS: usize = 1;
pub(super) const ITEM_CLEAR: usize = 2;
pub(super) const ITEM_RESET: usize = 3;
#[cfg(not(target_os = "emscripten"))]
pub(super) const ITEM_QUIT: usize = 4;

impl PauseMenu {
    pub(super) fn handle_top(
        &mut self,
        inputs: MenuInputs,
        _settings: &crate::settings::Settings,
    ) -> Option<PauseAction> {
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
        if inputs.btn1 {
            match self.top_selected {
                ITEM_CONTINUE => {
                    self.open = false;
                    return Some(PauseAction::Resume);
                }
                ITEM_SETTINGS => {
                    self.view = View::SettingsMenu;
                    self.settings_menu_selected = 0;
                }
                ITEM_CLEAR => {
                    self.view = View::ConfirmClearSave;
                    self.confirm_selected = 0;
                }
                ITEM_RESET => {
                    self.open = false;
                    return Some(PauseAction::ResetGame);
                }
                #[cfg(not(target_os = "emscripten"))]
                ITEM_QUIT => return Some(PauseAction::Quit),
                _ => {}
            }
        }
        None
    }

    pub(super) fn draw_top<D: RaylibDraw>(
        &self,
        d: &mut D,
        font: &Font,
        mut y: f32,
        res: crate::config::Resolution,
    ) {
        let size = crate::font::MONOGRAM_SIZE as f32;
        let line_h = size + 4.0;
        let item_x = item_x_for(res);

        let mut labels: Vec<&'static str> =
            vec!["Continue", "Settings", "Clear Save Data", "Reset Game"];
        // `cfg!` (not `#[cfg]`) keeps `mut` used on every target; the
        // optimizer drops the dead branch on web.
        if cfg!(not(target_os = "emscripten")) {
            labels.push("Quit");
        }
        debug_assert_eq!(labels.len(), TOP_COUNT);

        for (i, text) in labels.iter().enumerate() {
            d.draw_text_ex(
                font,
                text,
                Vector2::new(item_x, y),
                size,
                0.0,
                palette::color(Pal::White),
            );
            if i == self.top_selected {
                draw_indicator(d, self.time, item_x, y + size * 0.5);
            }
            y += line_h;
        }
    }
}
