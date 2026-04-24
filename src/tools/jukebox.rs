//! Jukebox tool: lists WAVs from `<project>/sfx/`, auto-plays on selection,
//! replays on space/enter.

use super::{HINT_Y, PANEL_H, PANEL_W, PANEL_X, PANEL_Y};
use sola_raylib::prelude::*;
use std::collections::HashMap;
use std::path::Path;

pub(super) struct State {
    pub names: Vec<String>,
    pub scroll: i32,
    pub active: i32,
    pub focus: i32,
    /// Tracks the last-played index so we can auto-play on selection change
    /// (matches Pico-8 / most sfx editor UX).
    pub last_played: i32,
}

impl State {
    pub fn new(sounds: &HashMap<String, Sound<'_>>) -> Self {
        Self {
            names: sorted_names(sounds),
            scroll: 0,
            active: -1,
            focus: -1,
            last_played: -1,
        }
    }

    pub fn refresh_names(&mut self, sounds: &HashMap<String, Sound<'_>>) {
        self.names = sorted_names(sounds);
        let n = self.names.len() as i32;
        if self.active >= n {
            self.active = if n > 0 { n - 1 } else { -1 };
        }
        self.last_played = -1;
    }
}

fn sorted_names(sounds: &HashMap<String, Sound<'_>>) -> Vec<String> {
    let mut names: Vec<String> = sounds.keys().cloned().collect();
    names.sort();
    names
}

pub(super) fn handle_input(
    rl: &RaylibHandle,
    state: &mut State,
    sounds: &HashMap<String, Sound<'_>>,
) {
    if state.names.is_empty() {
        return;
    }
    let n = state.names.len() as i32;
    let up = rl.is_key_pressed(KeyboardKey::KEY_UP) || rl.is_key_pressed(KeyboardKey::KEY_W);
    let down = rl.is_key_pressed(KeyboardKey::KEY_DOWN) || rl.is_key_pressed(KeyboardKey::KEY_S);
    if up {
        state.active = if state.active <= 0 {
            n - 1
        } else {
            state.active - 1
        };
    }
    if down {
        state.active = if state.active < 0 || state.active >= n - 1 {
            0
        } else {
            state.active + 1
        };
    }

    // Space/Enter: replay current selection without changing the index.
    let replay = (rl.is_key_pressed(KeyboardKey::KEY_SPACE)
        || rl.is_key_pressed(KeyboardKey::KEY_ENTER))
        && state.active >= 0;
    if replay
        && let Some(name) = state.names.get(state.active as usize)
        && let Some(sound) = sounds.get(name)
    {
        sound.play();
    }
}

/// Plays the active sound if the selection changed since the last call.
/// Called after draw so mouse-click selections in list_view_ex are covered.
pub(super) fn auto_play(state: &mut State, sounds: &HashMap<String, Sound<'_>>) {
    if state.active >= 0
        && state.active != state.last_played
        && let Some(name) = state.names.get(state.active as usize)
        && let Some(sound) = sounds.get(name)
    {
        sound.play();
        state.last_played = state.active;
    }
}

pub(super) fn draw(
    d: &mut RaylibDrawHandle,
    state: &mut State,
    sounds: &HashMap<String, Sound<'_>>,
    project_path: Option<&str>,
    sfx_dir: Option<&Path>,
) {
    d.gui_panel(
        Rectangle::new(PANEL_X, PANEL_Y, PANEL_W, PANEL_H),
        "Jukebox",
    );

    match project_path {
        Some(p) => d.draw_text(
            &format!("project: {}", p),
            30,
            (PANEL_Y + 30.0) as i32,
            14,
            Color::DARKGRAY,
        ),
        None => d.draw_text(
            "no project. Run `usagi tools path/to/project`.",
            30,
            (PANEL_Y + 30.0) as i32,
            14,
            Color::DARKGRAY,
        ),
    }
    if let Some(dir) = sfx_dir {
        d.draw_text(
            &format!("sfx: {}", dir.display()),
            30,
            (PANEL_Y + 50.0) as i32,
            12,
            Color::GRAY,
        );
    }

    let list_x = 30.0;
    let list_y = PANEL_Y + 80.0;
    let list_w = 460.0;
    let list_h = HINT_Y - list_y - 16.0;
    d.gui_list_view_ex(
        Rectangle::new(list_x, list_y, list_w, list_h),
        state.names.iter(),
        &mut state.scroll,
        &mut state.active,
        &mut state.focus,
    );

    if state.names.is_empty() {
        d.draw_text(
            "no .wav files found",
            (list_x + 10.0) as i32,
            (list_y + 20.0) as i32,
            14,
            Color::new(140, 140, 140, 255),
        );
    }

    if state.active >= 0
        && let Some(name) = state.names.get(state.active as usize)
    {
        let right_x = list_x + list_w + 30.0;
        d.draw_text(
            name,
            right_x as i32,
            (list_y + 10.0) as i32,
            22,
            Color::BLACK,
        );
        if d.gui_button(Rectangle::new(right_x, list_y + 54.0, 140.0, 40.0), "Play")
            && let Some(sound) = sounds.get(name)
        {
            sound.play();
        }
    }

    d.draw_text(
        "up/down or W/S: select   space/enter: replay   click: select+play",
        30,
        HINT_Y as i32,
        12,
        Color::new(140, 140, 140, 255),
    );
}
