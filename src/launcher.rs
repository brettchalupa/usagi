//! No-args launcher window. Running the binary with no subcommand (the
//! double-click / `.desktop` case) opens a small window with the Usagi
//! bunny icon and a prompt to drop a `main.lua` or project folder onto it
//! to start dev mode.

use crate::error::{Error, Result};
use crate::palette::{Pal, engine_color};
use sola_raylib::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;

const WIN_W: i32 = 720;
const WIN_H: i32 = 420;
const ICON_PX: f32 = 64.0;
const TEXT_PX: f32 = (crate::font::MONOGRAM_SIZE * 2) as f32;
const PROMPT: &str = "Drop your main.lua onto the window to start dev mode";

pub fn run() -> Result<()> {
    let bg = engine_color(Pal::DarkBlue);
    let fg = engine_color(Pal::White);
    let err_color = engine_color(Pal::Red);

    let log_level = if std::env::var_os("USAGI_RAYLIB_VERBOSE").is_some() {
        TraceLogLevel::LOG_INFO
    } else {
        TraceLogLevel::LOG_WARNING
    };
    let (mut rl, thread) = sola_raylib::init()
        .size(WIN_W, WIN_H)
        .title("Usagi")
        .log_level(log_level)
        .vsync()
        .build();
    crate::icon::apply(&mut rl);
    rl.set_target_fps(60);

    let font = crate::font::load_bundled(&mut rl, &thread);
    let icon = Image::load_image_from_mem(".png", crate::icon::ICON_PNG)
        .ok()
        .and_then(|img| rl.load_texture_from_image(&thread, &img).ok());

    let mut launch: Option<PathBuf> = None;
    let mut error: Option<String> = None;

    while !rl.window_should_close() {
        if rl.is_file_dropped() {
            let dropped = rl.load_dropped_files();
            if let Some(first) = dropped.paths().first() {
                match resolve_project(Path::new(first)) {
                    Ok(project) => launch = Some(project),
                    Err(msg) => error = Some(msg),
                }
            }
        }

        {
            let mut d = rl.begin_drawing(&thread);
            d.clear_background(bg);

            let mut y = 120.0;
            if let Some(tex) = &icon {
                let src_w = tex.width() as f32;
                let src_h = tex.height() as f32;
                d.draw_texture_pro(
                    tex,
                    Rectangle::new(0.0, 0.0, src_w, src_h),
                    Rectangle::new((WIN_W as f32 - ICON_PX) * 0.5, y, ICON_PX, ICON_PX),
                    Vector2::zero(),
                    0.0,
                    Color::WHITE,
                );
                y += ICON_PX + 28.0;
            }

            draw_centered(&mut d, &font, PROMPT, y, TEXT_PX, fg);
            if let Some(msg) = &error {
                draw_centered(&mut d, &font, msg, y + TEXT_PX + 12.0, TEXT_PX, err_color);
            }
        }

        if launch.is_some() {
            break;
        }
    }

    drop(icon);
    drop(font);
    drop(rl);

    match launch {
        Some(project) => spawn_dev(&project),
        None => Ok(()),
    }
}

/// Centers `text` horizontally in the window at vertical position `y`.
fn draw_centered(
    d: &mut RaylibDrawHandle,
    font: &Font,
    text: &str,
    y: f32,
    size: f32,
    color: Color,
) {
    let w = font.measure_text(text, size, 0.0).x;
    d.draw_text_ex(
        font,
        text,
        Vector2::new((WIN_W as f32 - w) * 0.5, y),
        size,
        0.0,
        color,
    );
}

/// Resolves a dropped path to something `usagi dev` can run: a `.lua`
/// file as-is, or a directory that contains `main.lua`. Absolute so the
/// re-exec'd process doesn't depend on the launcher's cwd. Returns a
/// user-facing message on anything else, shown in the launcher window.
fn resolve_project(dropped: &Path) -> std::result::Result<PathBuf, String> {
    let path = std::fs::canonicalize(dropped).unwrap_or_else(|_| dropped.to_path_buf());
    if path.is_dir() {
        if path.join("main.lua").is_file() {
            Ok(path)
        } else {
            Err("That folder has no main.lua".to_string())
        }
    } else if path.extension().and_then(|e| e.to_str()) == Some("lua") {
        Ok(path)
    } else {
        Err("Drop a .lua file or a folder with main.lua".to_string())
    }
}

/// Re-exec this binary as `usagi dev <path>` and return so the launcher
/// exits. Spawned detached; the dev session outlives this process.
fn spawn_dev(project: &Path) -> Result<()> {
    let exe =
        std::env::current_exe().map_err(|e| Error::Cli(format!("locating usagi binary: {e}")))?;
    Command::new(exe)
        .arg("dev")
        .arg(project)
        .spawn()
        .map_err(|e| Error::Cli(format!("launching dev mode: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn folder_with_main_lua_resolves_to_that_folder() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("main.lua"), b"-- main").unwrap();

        let resolved = resolve_project(dir.path()).unwrap();
        // canonicalized, so compare against the canonical form.
        assert_eq!(resolved, fs::canonicalize(dir.path()).unwrap());
    }

    #[test]
    fn folder_without_main_lua_is_rejected() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("other.lua"), b"-- not main").unwrap();

        assert!(resolve_project(dir.path()).is_err());
    }

    #[test]
    fn lua_file_resolves_to_the_file() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("game.lua");
        fs::write(&file, b"-- game").unwrap();

        let resolved = resolve_project(&file).unwrap();
        assert_eq!(resolved, fs::canonicalize(&file).unwrap());
    }

    #[test]
    fn non_lua_file_is_rejected() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("notes.txt");
        fs::write(&file, b"nope").unwrap();

        assert!(resolve_project(&file).is_err());
    }
}
