//! Pad Config capture flow. Mirror of `key_config.rs` for the three
//! action buttons (BTN1/BTN2/BTN3). Sequential, Pico-8 style: one
//! action highlighted at a time, the next supported gamepad button
//! press is captured. Directional inputs are deliberately not
//! remappable here; they stay on dpad + left stick.
//!
//! Gestures (keyboard and gamepad both supported so the screen stays
//! usable from either device):
//!   - Captured button advances to next action.
//!   - Backspace OR gamepad Select undoes the previous capture.
//!   - Delete resets every override and bails out. Keyboard-only:
//!     there's no clean gamepad equivalent; players who want a reset
//!     can cancel and start again.
//!   - Esc / Start cancels without persisting (handled centrally in
//!     `pause.rs` as the universal "climb one level" gesture).

use super::PauseAction;
use super::PauseMenu;
use super::View;
use super::inputs::{MenuInputs, PadConfigInputs};
use crate::input::{ACTION_NAMES, GamepadFamily, button_label};
use crate::pad_map::{PAD_ACTIONS, PadMap};
use crate::palette;
use crate::palette::Pal;
use sola_raylib::prelude::*;

impl PauseMenu {
    pub(super) fn handle_pad_config(
        &mut self,
        _inputs: MenuInputs,
        pc: PadConfigInputs,
    ) -> Option<PauseAction> {
        // DEL: Pico-8-style "reset to defaults" + exit. Single emit so
        // the session writes once. Keyboard-only gesture.
        if pc.delete {
            self.view = View::InputTester;
            self.pad_config = None;
            return Some(PauseAction::SetGamepadMap(PadMap::default()));
        }
        // BKSP / Select: undo the last capture. No-op at action 0.
        if pc.backspace {
            if let Some(state) = self.pad_config.as_mut()
                && state.action_index > 0
            {
                state.action_index -= 1;
                state.staging.overrides[state.action_index] = None;
            }
            return None;
        }
        let btn = pc.captured_button?;
        let state = self.pad_config.as_mut()?;
        if state.action_index >= PAD_ACTIONS.len() {
            return None;
        }
        // Exclusive bindings: reject if this button is already in
        // another slot. Player stays on the current action until they
        // pick a free button (or Backspace to revisit the conflicting
        // slot).
        let already_used = state
            .staging
            .overrides
            .iter()
            .enumerate()
            .any(|(i, slot)| i != state.action_index && *slot == Some(btn));
        if already_used {
            return None;
        }
        state.staging.overrides[state.action_index] = Some(btn);
        state.action_index += 1;
        if state.action_index >= PAD_ACTIONS.len() {
            // Done. Drop the player on the Tester so they can verify
            // their new bindings live, same as Key Config.
            let staged = state.staging.clone();
            self.view = View::InputTester;
            self.pad_config = None;
            return Some(PauseAction::SetGamepadMap(staged));
        }
        None
    }

    pub(super) fn draw_pad_config<D: RaylibDraw>(
        &self,
        d: &mut D,
        font: &Font,
        family: GamepadFamily,
        mut y: f32,
        res: crate::config::Resolution,
    ) {
        let size = crate::font::MONOGRAM_SIZE as f32;
        let line_h = size + 2.0;
        let color = palette::color(Pal::White);

        let Some(state) = self.pad_config.as_ref() else {
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

        let prompt = if state.action_index < PAD_ACTIONS.len() {
            let action = PAD_ACTIONS[state.action_index];
            format!("Press button for: {}", ACTION_NAMES[action as usize - 1])
        } else {
            "Capture complete".to_string()
        };
        let prompt_m = font.measure_text(&prompt, size, 0.0);
        let prompt_x = ((res.w - prompt_m.x) * 0.5).round();
        d.draw_text_ex(font, &prompt, Vector2::new(prompt_x, y), size, 0.0, color);
        y += line_h * 1.5;

        let entries: Vec<(usize, &'static str, String)> = PAD_ACTIONS
            .iter()
            .enumerate()
            .map(|(i, action)| {
                let name = ACTION_NAMES[*action as usize - 1];
                let label = match state.staging.overrides.get(i).copied().flatten() {
                    Some(b) => button_label(b, family).to_string(),
                    None => "--".to_string(),
                };
                (i, name, label)
            })
            .collect();
        let widest = entries
            .iter()
            .map(|(_, name, label)| font.measure_text(&format!("{name}: {label}"), size, 0.0).x)
            .fold(0.0_f32, f32::max);
        let item_x = ((res.w - widest) * 0.5).round();

        for (i, name, label) in entries {
            let line = format!("{name}: {label}");
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

        // Same footer pattern as key_config: one-liner when it fits,
        // stacked otherwise. Dpad / Home / stick clicks are silently
        // refused by the capture; the footer documents the useful
        // gestures rather than the rejected ones. Start on the gamepad
        // is the reset gesture here (parallels keyboard DEL); Select is
        // undo (parallels Backspace).
        let one_liner = "ESC CANCEL  -  BKSP/SELECT UNDO  -  DEL/START RESET";
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
            let lines = ["ESC CANCEL", "BKSP/SELECT UNDO", "DEL/START RESET"];
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
