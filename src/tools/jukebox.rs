//! Jukebox tool: lists WAVs from `<project>/sfx/` (auto-plays on selection)
//! and music streams from `<project>/music/` (manual play/stop).

use super::{HINT_Y, PANEL_H, PANEL_W, PANEL_X, PANEL_Y};
use crate::assets::MusicLibrary;
use crate::tools::theme;
use sola_raylib::prelude::*;
use std::collections::HashMap;
use std::path::Path;

/// Which list keyboard nav (up/down/W/S/space/enter) targets. Tab toggles;
/// clicking a list also auto-switches focus to it.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum FocusList {
    Sfx,
    Music,
}

pub(super) struct State {
    pub names: Vec<String>,
    pub scroll: i32,
    pub active: i32,
    pub focus: i32,
    /// Tracks the last-played sfx index so we can auto-play on selection
    /// change (matches Pico-8 / most sfx editor UX).
    pub last_played: i32,

    pub music_names: Vec<String>,
    pub music_scroll: i32,
    pub music_active: i32,
    pub music_focus: i32,

    pub focus_list: FocusList,
}

impl State {
    pub fn new(sounds: &HashMap<String, Sound<'_>>, music_names: Vec<String>) -> Self {
        Self {
            names: sorted_names(sounds),
            scroll: 0,
            active: -1,
            focus: -1,
            last_played: -1,
            music_names,
            music_scroll: 0,
            music_active: -1,
            music_focus: -1,
            focus_list: FocusList::Sfx,
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

    pub fn refresh_music_names(&mut self, names: Vec<String>) {
        self.music_names = names;
        let n = self.music_names.len() as i32;
        if self.music_active >= n {
            self.music_active = if n > 0 { n - 1 } else { -1 };
        }
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
    music: &mut MusicLibrary<'_>,
) {
    if rl.is_key_pressed(KeyboardKey::KEY_TAB) {
        state.focus_list = match state.focus_list {
            FocusList::Sfx => FocusList::Music,
            FocusList::Music => FocusList::Sfx,
        };
    }

    let up = rl.is_key_pressed(KeyboardKey::KEY_UP) || rl.is_key_pressed(KeyboardKey::KEY_W);
    let down = rl.is_key_pressed(KeyboardKey::KEY_DOWN) || rl.is_key_pressed(KeyboardKey::KEY_S);
    let activate =
        rl.is_key_pressed(KeyboardKey::KEY_SPACE) || rl.is_key_pressed(KeyboardKey::KEY_ENTER);

    match state.focus_list {
        FocusList::Sfx => {
            if state.names.is_empty() {
                return;
            }
            let n = state.names.len() as i32;
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
            if activate
                && state.active >= 0
                && let Some(name) = state.names.get(state.active as usize)
                && let Some(sound) = sounds.get(name)
            {
                sound.play();
            }
        }
        FocusList::Music => {
            if state.music_names.is_empty() {
                return;
            }
            let n = state.music_names.len() as i32;
            if up {
                state.music_active = if state.music_active <= 0 {
                    n - 1
                } else {
                    state.music_active - 1
                };
            }
            if down {
                state.music_active = if state.music_active < 0 || state.music_active >= n - 1 {
                    0
                } else {
                    state.music_active + 1
                };
            }
            if activate
                && state.music_active >= 0
                && let Some(name) = state.music_names.get(state.music_active as usize)
            {
                music.play(name);
            }
        }
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

#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    d: &mut RaylibDrawHandle,
    font: &Font,
    state: &mut State,
    sounds: &HashMap<String, Sound<'_>>,
    music: &mut MusicLibrary<'_>,
    project_path: Option<&str>,
    sfx_dir: Option<&Path>,
    music_dir: Option<&Path>,
) {
    // Tools window is 1280x720 desktop-size, so the 12 px game-canvas
    // font reads as tiny. Bitmap monogram + POINT filter scales
    // crisply at integer multiples; 2x (24 px) is comfortable for
    // tools at desktop resolution.
    const SMALL: f32 = (crate::font::MONOGRAM_SIZE * 2) as f32;

    d.gui_panel(
        Rectangle::new(PANEL_X, PANEL_Y, PANEL_W, PANEL_H),
        "Jukebox",
    );

    let info_x = 30.0;
    let mut info_y = PANEL_Y + 30.0;
    let project_line = match project_path {
        Some(p) => format!("project: {}", p),
        None => "no project. Run `usagi tools path/to/project`.".into(),
    };
    d.draw_text_ex(
        font,
        &project_line,
        Vector2::new(info_x, info_y),
        SMALL,
        0.0,
        theme::TEXT,
    );
    info_y += 22.0;
    if let Some(dir) = sfx_dir {
        d.draw_text_ex(
            font,
            &format!("sfx: {}", dir.display()),
            Vector2::new(info_x, info_y),
            SMALL,
            0.0,
            theme::TEXT_DIM,
        );
        info_y += 22.0;
    }
    if let Some(dir) = music_dir {
        d.draw_text_ex(
            font,
            &format!("music: {}", dir.display()),
            Vector2::new(info_x, info_y),
            SMALL,
            0.0,
            theme::TEXT_DIM,
        );
    }

    // Two-column layout: SFX on the left, music on the right.
    let col_w = 580.0;
    let col_gap = 40.0;
    let left_x = 30.0;
    let right_x = left_x + col_w + col_gap;

    let header_y = PANEL_Y + 110.0;
    let list_y = header_y + 30.0;
    let list_h = HINT_Y - list_y - 90.0;
    let buttons_y = list_y + list_h + 10.0;
    let label_y = buttons_y + 50.0;

    d.draw_text_ex(
        font,
        &format!("SFX [{}]", state.names.len()),
        Vector2::new(left_x, header_y),
        SMALL,
        0.0,
        theme::TEXT,
    );
    d.draw_text_ex(
        font,
        &format!("Music [{}]", state.music_names.len()),
        Vector2::new(right_x, header_y),
        SMALL,
        0.0,
        theme::TEXT,
    );

    // SFX column. Snapshot the active index before list_view_ex so we
    // can detect a mouse-click selection (post-draw the index changes
    // and we flip keyboard focus to that list automatically).
    let prev_sfx_active = state.active;
    let prev_music_active = state.music_active;
    d.gui_list_view_ex(
        Rectangle::new(left_x, list_y, col_w, list_h),
        state.names.iter(),
        &mut state.scroll,
        &mut state.active,
        &mut state.focus,
    );
    if state.names.is_empty() {
        d.draw_text_ex(
            font,
            "no .wav files found",
            Vector2::new(left_x + 10.0, list_y + 20.0),
            SMALL,
            0.0,
            theme::TEXT_MUTED,
        );
    }
    if d.gui_button(Rectangle::new(left_x, buttons_y, 140.0, 40.0), "Play")
        && state.active >= 0
        && let Some(name) = state.names.get(state.active as usize)
        && let Some(sound) = sounds.get(name)
    {
        sound.play();
    }
    if state.active >= 0
        && let Some(name) = state.names.get(state.active as usize)
    {
        d.draw_text_ex(
            font,
            &format!("selected: {}", name),
            Vector2::new(left_x + 160.0, buttons_y + 10.0),
            SMALL,
            0.0,
            theme::TEXT,
        );
    }

    // Music column.
    d.gui_list_view_ex(
        Rectangle::new(right_x, list_y, col_w, list_h),
        state.music_names.iter(),
        &mut state.music_scroll,
        &mut state.music_active,
        &mut state.music_focus,
    );

    // Auto-flip keyboard focus to whichever list the user just clicked.
    if state.active != prev_sfx_active && state.active >= 0 {
        state.focus_list = FocusList::Sfx;
    }
    if state.music_active != prev_music_active && state.music_active >= 0 {
        state.focus_list = FocusList::Music;
    }

    // Pink outline around the focused list to advertise where keyboard
    // input goes. raygui's list_view doesn't expose a "focused" mode,
    // so we draw the indicator on top.
    let focused_rect = match state.focus_list {
        FocusList::Sfx => Rectangle::new(left_x - 3.0, list_y - 3.0, col_w + 6.0, list_h + 6.0),
        FocusList::Music => Rectangle::new(right_x - 3.0, list_y - 3.0, col_w + 6.0, list_h + 6.0),
    };
    d.draw_rectangle_lines_ex(focused_rect, 3.0, theme::ACCENT);
    if state.music_names.is_empty() {
        d.draw_text_ex(
            font,
            "no music files found",
            Vector2::new(right_x + 10.0, list_y + 20.0),
            SMALL,
            0.0,
            theme::TEXT_MUTED,
        );
    }
    if d.gui_button(Rectangle::new(right_x, buttons_y, 140.0, 40.0), "Play")
        && state.music_active >= 0
        && let Some(name) = state.music_names.get(state.music_active as usize)
    {
        music.play(name);
    }
    if d.gui_button(
        Rectangle::new(right_x + 150.0, buttons_y, 140.0, 40.0),
        "Stop",
    ) {
        music.stop();
    }
    let playing_line = match music.current() {
        Some(name) => format!("playing: {}", name),
        None => "playing: -".into(),
    };
    d.draw_text_ex(
        font,
        &playing_line,
        Vector2::new(right_x, label_y),
        SMALL,
        0.0,
        theme::TEXT,
    );

    d.draw_text_ex(
        font,
        "Tab: switch list   up/down or W/S: select   space/enter: play   click: select+play",
        Vector2::new(30.0, HINT_Y),
        SMALL,
        0.0,
        theme::TEXT_MUTED,
    );
}
