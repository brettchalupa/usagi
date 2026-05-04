//! Top-level pause view: Continue, Music, SFX, Fullscreen, Input,
//! Clear Save Data, Reset Game, Quit. Vertical list with the active
//! row marked by an oscillating indicator.
//!
//! Side-effecting items (volume changes, fullscreen toggle, save
//! clear, reset, quit) emit a `PauseAction`; the session applies them.

use super::PauseMenu;
use super::View;
use super::inputs::MenuInputs;
use super::volume::{draw_volume_bars, step_volume};
use super::{PauseAction, draw_indicator};
use crate::palette;
use crate::palette::Pal;
use crate::settings::Settings;
use sola_raylib::prelude::*;

pub(super) const TOP_COUNT: usize = 8;
pub(super) const ITEM_CONTINUE: usize = 0;
pub(super) const ITEM_MUSIC: usize = 1;
pub(super) const ITEM_SFX: usize = 2;
pub(super) const ITEM_FULLSCREEN: usize = 3;
pub(super) const ITEM_INPUT: usize = 4;
pub(super) const ITEM_CLEAR: usize = 5;
pub(super) const ITEM_RESET: usize = 6;
pub(super) const ITEM_QUIT: usize = 7;

impl PauseMenu {
    pub(super) fn handle_top(
        &mut self,
        inputs: MenuInputs,
        settings: &Settings,
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

    pub(super) fn draw_top<D: RaylibDraw>(
        &self,
        d: &mut D,
        font: &Font,
        settings: &Settings,
        mut y: f32,
    ) {
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
                draw_indicator(d, self.time, item_x, y + size * 0.5);
            }
            y += line_h;
        }
    }
}
