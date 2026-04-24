//! Usagi tools window. A raygui-based multi-tool host. Currently:
//!   - Jukebox: lists sfx from <project>/sfx/, auto-plays on selection.
//!   - TilePicker: stub, coming soon.

use crate::assets::{load_sfx, scan_sfx};
use mlua::prelude::*;
use sola_raylib::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Jukebox,
    TilePicker,
}

struct State {
    active: Tool,
    sfx_names: Vec<String>,
    sfx_scroll: i32,
    sfx_active: i32,
    sfx_focus: i32,
    /// Tracks the last-played index so we can auto-play on selection change
    /// (matches Pico-8 / most sfx editor UX).
    sfx_last_played: i32,
}

pub fn run(project_path: Option<&str>) -> LuaResult<()> {
    let project_dir = project_path.and_then(resolve_project_dir);
    let sfx_dir = project_dir.as_ref().map(|d| d.join("sfx"));

    let (mut rl, thread) = sola_raylib::init()
        .size(640, 480)
        .title("USAGI TOOLS")
        .highdpi()
        .resizable()
        .build();
    rl.set_target_fps(60);

    let audio = RaylibAudio::init_audio_device()
        .map_err(|e| eprintln!("[usagi] audio init failed: {}", e))
        .ok();

    let mut sounds: HashMap<String, Sound<'_>> = HashMap::new();
    let mut sfx_manifest: HashMap<String, SystemTime> = HashMap::new();
    if let (Some(a), Some(dir)) = (&audio, &sfx_dir) {
        sounds = load_sfx(a, dir);
        sfx_manifest = scan_sfx(dir);
    }

    let mut state = State {
        active: Tool::Jukebox,
        sfx_names: sorted_names(&sounds),
        sfx_scroll: 0,
        sfx_active: -1,
        sfx_focus: -1,
        sfx_last_played: -1,
    };

    while !rl.window_should_close() {
        // Live reload of sfx dir: scan, if manifest changed reload the whole
        // map so users can drop new WAVs in and hear them immediately.
        if let (Some(a), Some(dir)) = (&audio, &sfx_dir) {
            let new_manifest = scan_sfx(dir);
            if new_manifest != sfx_manifest {
                sfx_manifest = new_manifest;
                sounds = load_sfx(a, dir);
                state.sfx_names = sorted_names(&sounds);
                let n = state.sfx_names.len() as i32;
                if state.sfx_active >= n {
                    state.sfx_active = if n > 0 { n - 1 } else { -1 };
                }
                state.sfx_last_played = -1;
                println!("[usagi] jukebox reloaded sfx ({} sound(s))", sounds.len());
            }
        }

        // Jukebox keyboard nav: up/down or W/S cycles through the list.
        if state.active == Tool::Jukebox && !state.sfx_names.is_empty() {
            let n = state.sfx_names.len() as i32;
            let up =
                rl.is_key_pressed(KeyboardKey::KEY_UP) || rl.is_key_pressed(KeyboardKey::KEY_W);
            let down =
                rl.is_key_pressed(KeyboardKey::KEY_DOWN) || rl.is_key_pressed(KeyboardKey::KEY_S);
            if up {
                state.sfx_active = if state.sfx_active <= 0 {
                    n - 1
                } else {
                    state.sfx_active - 1
                };
            }
            if down {
                state.sfx_active = if state.sfx_active < 0 || state.sfx_active >= n - 1 {
                    0
                } else {
                    state.sfx_active + 1
                };
            }
        }

        // Space/Enter: replay the current selection. Useful for hearing a
        // sound again without changing the index.
        let replay = (rl.is_key_pressed(KeyboardKey::KEY_SPACE)
            || rl.is_key_pressed(KeyboardKey::KEY_ENTER))
            && state.active == Tool::Jukebox
            && state.sfx_active >= 0;
        if replay
            && let Some(name) = state.sfx_names.get(state.sfx_active as usize)
            && let Some(sound) = sounds.get(name)
        {
            sound.play();
        }

        {
            let mut d = rl.begin_drawing(&thread);
            d.clear_background(Color::RAYWHITE);

            // Tab bar.
            if d.gui_button(Rectangle::new(20., 20., 110., 30.), "Jukebox") {
                state.active = Tool::Jukebox;
            }
            if d.gui_button(Rectangle::new(140., 20., 110., 30.), "TilePicker") {
                state.active = Tool::TilePicker;
            }

            match state.active {
                Tool::Jukebox => draw_jukebox(
                    &mut d,
                    &mut state,
                    &sounds,
                    project_path,
                    sfx_dir.as_deref(),
                ),
                Tool::TilePicker => draw_tilepicker(&mut d),
            }
        }

        // Auto-play on selection change (covers mouse click into the list_view
        // which we can't intercept until after the draw call returns).
        if state.active == Tool::Jukebox
            && state.sfx_active >= 0
            && state.sfx_active != state.sfx_last_played
            && let Some(name) = state.sfx_names.get(state.sfx_active as usize)
            && let Some(sound) = sounds.get(name)
        {
            sound.play();
            state.sfx_last_played = state.sfx_active;
        }
    }

    Ok(())
}

fn sorted_names(sounds: &HashMap<String, Sound<'_>>) -> Vec<String> {
    let mut names: Vec<String> = sounds.keys().cloned().collect();
    names.sort();
    names
}

/// Resolves the `usagi tools <path>` arg to a project directory:
///   - a directory is used directly
///   - anything that resolves via `cli::resolve_script_path` uses its parent dir
///   - otherwise None (tools open with no project loaded)
fn resolve_project_dir(path: &str) -> Option<PathBuf> {
    let p = Path::new(path);
    if p.is_dir() {
        return Some(p.to_path_buf());
    }
    let script = crate::cli::resolve_script_path(path).ok()?;
    Path::new(&script)
        .parent()
        .map(|parent| parent.to_path_buf())
}

fn draw_jukebox(
    d: &mut RaylibDrawHandle,
    state: &mut State,
    sounds: &HashMap<String, Sound<'_>>,
    project_path: Option<&str>,
    sfx_dir: Option<&Path>,
) {
    d.gui_panel(Rectangle::new(20., 70., 600., 380.), "Jukebox");

    match project_path {
        Some(p) => d.draw_text(&format!("project: {}", p), 30, 100, 14, Color::DARKGRAY),
        None => d.draw_text(
            "no project. Run `usagi tools path/to/project`.",
            30,
            100,
            14,
            Color::DARKGRAY,
        ),
    }
    if let Some(dir) = sfx_dir {
        d.draw_text(&format!("sfx: {}", dir.display()), 30, 120, 12, Color::GRAY);
    }

    let list_rect = Rectangle::new(30., 150., 280., 270.);
    d.gui_list_view_ex(
        list_rect,
        state.sfx_names.iter(),
        &mut state.sfx_scroll,
        &mut state.sfx_active,
        &mut state.sfx_focus,
    );

    if state.sfx_names.is_empty() {
        d.draw_text(
            "no .wav files found",
            40,
            170,
            14,
            Color::new(140, 140, 140, 255),
        );
    }

    // Right panel: info + replay button for the selected sound.
    if state.sfx_active >= 0
        && let Some(name) = state.sfx_names.get(state.sfx_active as usize)
    {
        d.draw_text(name, 330, 160, 18, Color::BLACK);
        if d.gui_button(Rectangle::new(330., 200., 120., 36.), "Play")
            && let Some(sound) = sounds.get(name)
        {
            sound.play();
        }
    }

    d.draw_text(
        "up/down or W/S: select   space/enter: replay   click: select+play",
        30,
        430,
        12,
        Color::new(140, 140, 140, 255),
    );
}

fn draw_tilepicker(d: &mut RaylibDrawHandle) {
    d.gui_panel(Rectangle::new(20., 70., 600., 380.), "TilePicker");
    d.draw_text("coming soon", 30, 110, 18, Color::DARKGRAY);
}
