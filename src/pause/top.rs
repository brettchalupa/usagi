//! Top-level pause view: Continue, [Lua-registered menu items],
//! Settings, Clear Save Data, Reset Game, Quit. Vertical list with
//! the active row marked by an oscillating indicator. Tweakable
//! options (volumes, fullscreen, input mapping) live under the
//! Settings sub-menu so the Top stays short.
//!
//! The Lua-registered items between Continue and Settings come from
//! `usagi.menu_item` (see `menu_items.rs`) and shift every following
//! row's index by however many are registered.
//!
//! Side-effecting items (save clear, reset, quit, custom Lua items)
//! emit a `PauseAction`; the session applies them.

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
const STATIC_TOP_COUNT: usize = 5;
#[cfg(target_os = "emscripten")]
const STATIC_TOP_COUNT: usize = 4;

/// Resolves the current row count, given the number of Lua-registered
/// menu items. Custom items sit between Continue and Settings, so the
/// total grows linearly with the registration count.
pub(super) fn top_count(menu_item_count: usize) -> usize {
    STATIC_TOP_COUNT + menu_item_count
}

/// Logical kinds of Top rows. Returned by `resolve_row` so the
/// handler doesn't have to do offset math at every branch.
pub(super) enum TopRow {
    Continue,
    MenuItem(usize),
    Settings,
    Clear,
    Reset,
    #[cfg(not(target_os = "emscripten"))]
    Quit,
}

fn resolve_row(selected: usize, menu_item_count: usize) -> Option<TopRow> {
    if selected == 0 {
        return Some(TopRow::Continue);
    }
    if selected <= menu_item_count {
        return Some(TopRow::MenuItem(selected - 1));
    }
    let after_menu = selected - menu_item_count - 1;
    match after_menu {
        0 => Some(TopRow::Settings),
        1 => Some(TopRow::Clear),
        2 => Some(TopRow::Reset),
        #[cfg(not(target_os = "emscripten"))]
        3 => Some(TopRow::Quit),
        _ => None,
    }
}

// Helpers for tests so they can index by intent ("Settings",
// "Reset Game") without knowing the menu-item count. Each helper
// assumes the test environment has no menu items registered (the
// default), but accepts a `menu_item_count` for the cases that do.
#[cfg(test)]
pub(super) fn item_settings(menu_item_count: usize) -> usize {
    1 + menu_item_count
}
#[cfg(test)]
pub(super) fn item_clear(menu_item_count: usize) -> usize {
    2 + menu_item_count
}
#[cfg(test)]
pub(super) fn item_reset(menu_item_count: usize) -> usize {
    3 + menu_item_count
}
#[cfg(all(test, not(target_os = "emscripten")))]
pub(super) fn item_quit(menu_item_count: usize) -> usize {
    4 + menu_item_count
}

impl PauseMenu {
    pub(super) fn handle_top(
        &mut self,
        inputs: MenuInputs,
        menu_items: &[String],
    ) -> Option<PauseAction> {
        let total = top_count(menu_items.len());
        if inputs.btn2 {
            self.open = false;
            return Some(PauseAction::Resume);
        }
        if inputs.up {
            self.top_selected = if self.top_selected == 0 {
                total - 1
            } else {
                self.top_selected - 1
            };
        }
        if inputs.down {
            self.top_selected = (self.top_selected + 1) % total;
        }
        if inputs.btn1 {
            let row = resolve_row(self.top_selected, menu_items.len())?;
            return match row {
                TopRow::Continue => {
                    self.open = false;
                    Some(PauseAction::Resume)
                }
                TopRow::MenuItem(idx) => Some(PauseAction::FireMenuItem(idx)),
                TopRow::Settings => {
                    self.view = View::SettingsMenu;
                    self.settings_menu_selected = 0;
                    None
                }
                TopRow::Clear => {
                    self.view = View::ConfirmClearSave;
                    self.confirm_selected = 0;
                    None
                }
                TopRow::Reset => {
                    self.open = false;
                    Some(PauseAction::ResetGame)
                }
                #[cfg(not(target_os = "emscripten"))]
                TopRow::Quit => Some(PauseAction::Quit),
            };
        }
        None
    }

    pub(super) fn draw_top<D: RaylibDraw>(
        &self,
        d: &mut D,
        font: &Font,
        menu_items: &[String],
        mut y: f32,
        res: crate::config::Resolution,
    ) {
        let size = crate::font::MONOGRAM_SIZE as f32;
        let line_h = size + 4.0;
        let item_x = item_x_for(res);

        let mut labels: Vec<&str> = Vec::with_capacity(top_count(menu_items.len()));
        labels.push("Continue");
        for item in menu_items {
            labels.push(item.as_str());
        }
        labels.push("Settings");
        labels.push("Clear Save Data");
        labels.push("Reset Game");
        if cfg!(not(target_os = "emscripten")) {
            labels.push("Quit");
        }
        debug_assert_eq!(labels.len(), top_count(menu_items.len()));

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_row_with_no_menu_items_matches_static_layout() {
        assert!(matches!(resolve_row(0, 0), Some(TopRow::Continue)));
        assert!(matches!(resolve_row(1, 0), Some(TopRow::Settings)));
        assert!(matches!(resolve_row(2, 0), Some(TopRow::Clear)));
        assert!(matches!(resolve_row(3, 0), Some(TopRow::Reset)));
        #[cfg(not(target_os = "emscripten"))]
        assert!(matches!(resolve_row(4, 0), Some(TopRow::Quit)));
    }

    #[test]
    fn resolve_row_shifts_static_rows_when_menu_items_registered() {
        // 2 menu items: rows 1 and 2 are MenuItem(0)/MenuItem(1),
        // Settings moves to row 3, etc.
        assert!(matches!(resolve_row(0, 2), Some(TopRow::Continue)));
        assert!(matches!(resolve_row(1, 2), Some(TopRow::MenuItem(0))));
        assert!(matches!(resolve_row(2, 2), Some(TopRow::MenuItem(1))));
        assert!(matches!(resolve_row(3, 2), Some(TopRow::Settings)));
        assert!(matches!(resolve_row(4, 2), Some(TopRow::Clear)));
    }
}
