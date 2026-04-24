//! Usagi tools window. Hosts the shell (fixed 1280x720 window, tab bar,
//! shared toast, asset loading + live reload); individual tools live in
//! sibling modules and expose a small `State` + `handle_input` + `draw`
//! API.

mod jukebox;
mod tilepicker;

use crate::assets::{load_sfx, load_sprites, scan_sfx};
use mlua::prelude::*;
use sola_raylib::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Fixed window size. Designed against 1280x720 for now; nothing in the
/// layout is dynamic.
pub(super) const WINDOW_W: f32 = 1280.;
pub(super) const WINDOW_H: f32 = 720.;

/// Shared panel geometry. Each tool draws into this panel.
pub(super) const PANEL_X: f32 = 20.;
pub(super) const PANEL_Y: f32 = 70.;
pub(super) const PANEL_W: f32 = WINDOW_W - 2.0 * PANEL_X;
pub(super) const PANEL_H: f32 = WINDOW_H - PANEL_Y - 20.0;
pub(super) const HINT_Y: f32 = PANEL_Y + PANEL_H - 24.0;

const TOAST_SECS: f32 = 2.5;

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Jukebox,
    TilePicker,
}

pub(super) struct Toast {
    pub timer: f32,
    pub message: String,
}

impl Toast {
    pub fn new(message: String) -> Self {
        Self {
            timer: TOAST_SECS,
            message,
        }
    }
}

struct State {
    active: Tool,
    jukebox: jukebox::State,
    tilepicker: tilepicker::State,
    toast: Option<Toast>,
}

pub fn run(project_path: Option<&str>) -> LuaResult<()> {
    let project_dir = project_path.and_then(resolve_project_dir);
    let sfx_dir = project_dir.as_ref().map(|d| d.join("sfx"));
    let sprites_path = project_dir.as_ref().map(|d| d.join("sprites.png"));

    let (mut rl, thread) = sola_raylib::init()
        .size(WINDOW_W as i32, WINDOW_H as i32)
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

    let mut sprites: Option<Texture2D> = sprites_path
        .as_ref()
        .and_then(|p| load_sprites(&mut rl, &thread, p));
    let mut sprites_mtime = sprites_path
        .as_ref()
        .and_then(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());

    let mut state = State {
        active: Tool::Jukebox,
        jukebox: jukebox::State::new(&sounds),
        tilepicker: tilepicker::State::new(),
        toast: None,
    };

    while !rl.window_should_close() {
        let dt = rl.get_frame_time();

        if let Some(toast) = &mut state.toast {
            toast.timer -= dt;
            if toast.timer <= 0.0 {
                state.toast = None;
            }
        }

        if let (Some(a), Some(dir)) = (&audio, &sfx_dir) {
            let new_manifest = scan_sfx(dir);
            if new_manifest != sfx_manifest {
                sfx_manifest = new_manifest;
                sounds = load_sfx(a, dir);
                state.jukebox.refresh_names(&sounds);
                println!("[usagi] jukebox reloaded sfx ({} sound(s))", sounds.len());
            }
        }

        if let Some(p) = &sprites_path
            && let Ok(modified) = std::fs::metadata(p).and_then(|m| m.modified())
            && Some(modified) != sprites_mtime
        {
            sprites_mtime = Some(modified);
            sprites = load_sprites(&mut rl, &thread, p);
            println!("[usagi] tools reloaded {}", p.display());
        }

        // Global tab shortcuts. Applied before per-tool input so switching
        // takes effect on the same frame.
        if rl.is_key_pressed(KeyboardKey::KEY_ONE) {
            state.active = Tool::Jukebox;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_TWO) {
            state.active = Tool::TilePicker;
        }

        match state.active {
            Tool::Jukebox => jukebox::handle_input(&rl, &mut state.jukebox, &sounds),
            Tool::TilePicker => {
                if let Some(msg) =
                    tilepicker::handle_input(&mut rl, &mut state.tilepicker, sprites.as_ref(), dt)
                {
                    state.toast = Some(Toast::new(msg));
                }
            }
        }

        {
            let mut d = rl.begin_drawing(&thread);
            d.clear_background(Color::RAYWHITE);

            if d.gui_button(Rectangle::new(20., 20., 130., 30.), "Jukebox [1]") {
                state.active = Tool::Jukebox;
            }
            if d.gui_button(Rectangle::new(160., 20., 150., 30.), "TilePicker [2]") {
                state.active = Tool::TilePicker;
            }

            match state.active {
                Tool::Jukebox => jukebox::draw(
                    &mut d,
                    &mut state.jukebox,
                    &sounds,
                    project_path,
                    sfx_dir.as_deref(),
                ),
                Tool::TilePicker => tilepicker::draw(
                    &mut d,
                    &state.tilepicker,
                    sprites.as_ref(),
                    sprites_path.as_deref(),
                ),
            }

            if let Some(toast) = &state.toast {
                draw_toast(&mut d, &toast.message);
            }
        }

        // Auto-play on selection change (covers mouse click into the
        // list_view which we can't intercept until after the draw returns).
        if state.active == Tool::Jukebox {
            jukebox::auto_play(&mut state.jukebox, &sounds);
        }
    }

    Ok(())
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

fn draw_toast(d: &mut RaylibDrawHandle, message: &str) {
    let w = 360.0;
    let h = 48.0;
    let x = WINDOW_W - w - 20.0;
    let y = WINDOW_H - h - 20.0;
    d.gui_panel(Rectangle::new(x, y, w, h), "");
    d.draw_text(
        message,
        (x + 12.0) as i32,
        (y + 16.0) as i32,
        14,
        Color::BLACK,
    );
}
