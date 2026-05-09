//! Key Config capture flow. Pico-8-style: one action highlighted at
//! a time, the next supported keypress is captured. Backspace undoes
//! the previous capture; Delete resets every override and bails out;
//! Esc cancels without persisting. Captures stage into a clone
//! of the current keymap, and only on completion does the menu emit
//! `PauseAction::SetKeymap` so the session writes once.

use super::PauseMenu;
use super::View;
use super::inputs::{KeyConfigInputs, MenuInputs};
use super::{ACTION_COUNT, PauseAction};
use crate::input::ACTION_NAMES;
use crate::keymap::{self, Keymap};
use crate::palette;
use crate::palette::Pal;
use sola_raylib::prelude::*;

/// In-flight Key Config capture. Mutated as the player presses keys;
/// emitted via `PauseAction::SetKeymap` on completion.
#[derive(Debug, Clone)]
pub(super) struct KeyConfigState {
    pub staging: Keymap,
    /// Index (0..ACTION_COUNT) of the action currently awaiting a key.
    pub action_index: usize,
}

/// Keys that capture refuses to bind: menu controls (Esc/Enter), the
/// reset gesture (Delete), the undo gesture (Backspace), and keys with
/// system meaning (F-keys, modifiers).
pub(super) fn is_reserved_key(k: KeyboardKey) -> bool {
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

impl PauseMenu {
    pub(super) fn handle_key_config(
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

    pub(super) fn draw_key_config<D: RaylibDraw>(
        &self,
        d: &mut D,
        font: &Font,
        mut y: f32,
        res: crate::config::Resolution,
    ) {
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
        let prompt_x = ((res.w - prompt_m.x) * 0.5).round();
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
        let item_x = ((res.w - widest) * 0.5).round();

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

        // Footer with the three keyboard hotkeys. At the default
        // 320-wide game it fits on one line; at narrower configs
        // (vertical or sub-default widths) it stacks vertically so
        // the actions stay readable instead of clipping off-screen.
        let one_liner = "ESC CANCEL  -  BKSP UNDO  -  DEL RESET";
        let one_liner_w = font.measure_text(one_liner, size, 0.0).x;
        let margin = 8.0;
        if one_liner_w + margin * 2.0 <= res.w {
            let footer_x = ((res.w - one_liner_w) * 0.5).round();
            let footer_y = res.h - size - 8.0;
            d.draw_text_ex(
                font,
                one_liner,
                Vector2::new(footer_x, footer_y),
                size,
                0.0,
                color,
            );
        } else {
            let lines = ["ESC CANCEL", "BKSP UNDO", "DEL RESET"];
            let stacked_h = size * lines.len() as f32 + 2.0 * (lines.len() - 1) as f32;
            let mut fy = res.h - stacked_h - 4.0;
            for line in lines {
                let m = font.measure_text(line, size, 0.0);
                let fx = ((res.w - m.x) * 0.5).round();
                d.draw_text_ex(font, line, Vector2::new(fx, fy), size, 0.0, color);
                fy += size + 2.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
