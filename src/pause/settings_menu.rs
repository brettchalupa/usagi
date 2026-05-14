//! Settings sub-menu under the Top view. Bundles the engine-level
//! tweakables (audio volumes, fullscreen, input config) so the Top
//! list stays short: Continue + destructive actions + Quit. Mirrors
//! `top.rs`'s structure (vertical list, indicator, volume bars on
//! Music/SFX rows) since the interaction pattern is identical.

use super::PauseAction;
use super::PauseMenu;
use super::View;
use super::draw_indicator;
use super::inputs::MenuInputs;
use super::volume::{draw_volume_bars, step_volume};
use crate::palette;
use crate::palette::Pal;
use crate::settings::Settings;
use sola_raylib::prelude::*;

pub(super) const SETTINGS_COUNT: usize = 4;
pub(super) const SETTINGS_ITEM_MUSIC: usize = 0;
pub(super) const SETTINGS_ITEM_SFX: usize = 1;
pub(super) const SETTINGS_ITEM_FULLSCREEN: usize = 2;
pub(super) const SETTINGS_ITEM_INPUT: usize = 3;

impl PauseMenu {
    pub(super) fn handle_settings_menu(
        &mut self,
        inputs: MenuInputs,
        settings: &Settings,
    ) -> Option<PauseAction> {
        if inputs.btn2 {
            self.view = View::Top;
            return None;
        }
        if inputs.up {
            self.settings_menu_selected = if self.settings_menu_selected == 0 {
                SETTINGS_COUNT - 1
            } else {
                self.settings_menu_selected - 1
            };
        }
        if inputs.down {
            self.settings_menu_selected = (self.settings_menu_selected + 1) % SETTINGS_COUNT;
        }
        if inputs.left || inputs.right {
            let dir = if inputs.right { 1 } else { -1 };
            match self.settings_menu_selected {
                SETTINGS_ITEM_MUSIC => {
                    return Some(PauseAction::SetMusicVolume(step_volume(
                        settings.music_volume,
                        dir,
                    )));
                }
                SETTINGS_ITEM_SFX => {
                    return Some(PauseAction::SetSfxVolume(step_volume(
                        settings.sfx_volume,
                        dir,
                    )));
                }
                SETTINGS_ITEM_FULLSCREEN => return Some(PauseAction::ToggleFullscreen),
                _ => {}
            }
        }
        if inputs.btn1 {
            match self.settings_menu_selected {
                SETTINGS_ITEM_FULLSCREEN => return Some(PauseAction::ToggleFullscreen),
                SETTINGS_ITEM_INPUT => {
                    self.view = View::InputMenu;
                    self.input_menu_selected = 0;
                }
                _ => {}
            }
        }
        None
    }

    pub(super) fn draw_settings_menu<D: RaylibDraw>(
        &self,
        d: &mut D,
        font: &Font,
        settings: &Settings,
        mut y: f32,
        res: crate::config::Resolution,
    ) {
        let size = crate::font::MONOGRAM_SIZE as f32;
        let line_h = size + 4.0;
        let item_x = super::item_x_for(res);

        let labels = [
            "Music:".to_string(),
            "SFX:".to_string(),
            format!(
                "Fullscreen: {}",
                if settings.fullscreen { "On" } else { "Off" }
            ),
            "Input".to_string(),
        ];
        debug_assert_eq!(labels.len(), SETTINGS_COUNT);

        for (i, text) in labels.iter().enumerate() {
            d.draw_text_ex(
                font,
                text,
                Vector2::new(item_x, y),
                size,
                0.0,
                palette::color(Pal::White),
            );
            match i {
                SETTINGS_ITEM_MUSIC => {
                    let label_m = font.measure_text(text, size, 0.0);
                    draw_volume_bars(
                        d,
                        font,
                        item_x + label_m.x + 6.0,
                        y,
                        settings.music_volume,
                        res,
                    );
                }
                SETTINGS_ITEM_SFX => {
                    let label_m = font.measure_text(text, size, 0.0);
                    draw_volume_bars(
                        d,
                        font,
                        item_x + label_m.x + 6.0,
                        y,
                        settings.sfx_volume,
                        res,
                    );
                }
                _ => {}
            }
            if i == self.settings_menu_selected {
                draw_indicator(d, self.time, item_x, y + size * 0.5);
            }
            y += line_h;
        }
    }
}
