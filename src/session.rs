//! The interactive game session: runs the raylib event loop, drives the
//! Lua VM, handles live reload (if `dev` is true), and renders.

use crate::api::{record_err, setup_api};
use crate::assets::{SfxLibrary, SpriteSheet, load_script};
use crate::input::key_from_u32;
use crate::palette::palette;
use crate::render::{draw_error_overlay, draw_render_target};
use crate::{GAME_HEIGHT, GAME_WIDTH};

use mlua::prelude::*;
use sola_raylib::prelude::*;

/// Runs a Usagi game session. When `dev` is true, the script, sprite sheet,
/// and sfx are live-reloaded on filesystem change; otherwise the session is
/// read-only after startup. F5 always resets state via `_init()`.
pub fn run(script_path: &str, dev: bool) -> crate::Result<()> {
    let sprites_path = std::path::Path::new(script_path).with_file_name("sprites.png");
    let sfx_dir = std::path::Path::new(script_path).with_file_name("sfx");

    let (mut rl, thread) = sola_raylib::init()
        .size((GAME_WIDTH * 2.) as i32, (GAME_HEIGHT * 2.) as i32)
        .highdpi()
        .resizable()
        .title("USAGI")
        .build();
    rl.set_target_fps(60);
    let mut rt: RenderTexture2D = rl
        .load_render_texture(&thread, GAME_WIDTH as u32, GAME_HEIGHT as u32)
        .unwrap();

    let lua = Lua::new();
    setup_api(&lua)?;

    // Latest Lua error, if any. Rendered as an on-screen overlay; cleared on
    // successful reload or F5 reset.
    let mut last_error: Option<String> = None;

    record_err(
        &mut last_error,
        "initial load",
        load_script(&lua, script_path),
    );

    if let Ok(init) = lua.globals().get::<LuaFunction>("_init") {
        record_err(&mut last_error, "_init", init.call::<()>(()));
    }
    let mut update: Option<LuaFunction> = lua.globals().get("_update").ok();
    let mut draw: Option<LuaFunction> = lua.globals().get("_draw").ok();
    let mut last_modified = std::fs::metadata(script_path)
        .and_then(|m| m.modified())
        .ok();

    let mut sprites = SpriteSheet::load(&mut rl, &thread, &sprites_path);

    // Audio is optional. If the device can't be initialised, games still run;
    // sfx.play just no-ops via SfxLibrary::empty.
    let audio = RaylibAudio::init_audio_device()
        .map_err(|e| eprintln!("[usagi] audio init failed: {}", e))
        .ok();
    let mut sfx = match &audio {
        Some(a) => SfxLibrary::load(a, &sfx_dir),
        None => SfxLibrary::empty(),
    };

    // FPS overlay: on by default in dev mode, off in run mode. Toggle with `~`.
    let mut show_fps = dev;

    while !rl.window_should_close() {
        // Live reload is gated on `dev`. In `run` mode the session is
        // read-only after startup; file changes on disk are ignored.
        if dev {
            // Script reload: re-exec on mtime change. State is preserved
            // (no _init call); F5 is the explicit reset. Errors are logged
            // and the previous callbacks keep running so a half-saved file
            // can't kill the session.
            if let Ok(modified) = std::fs::metadata(script_path).and_then(|m| m.modified())
                && Some(modified) != last_modified
            {
                last_modified = Some(modified);
                match load_script(&lua, script_path) {
                    Ok(()) => {
                        println!("[usagi] reloaded {}", script_path);
                        update = lua.globals().get("_update").ok();
                        draw = lua.globals().get("_draw").ok();
                        last_error = None;
                    }
                    Err(e) => {
                        let msg = format!("reload: {}", e);
                        eprintln!("[usagi] {}", msg);
                        last_error = Some(msg);
                    }
                }
            }

            // Sprite sheet reload. Drop of the previous Texture2D frees GPU.
            if sprites.reload_if_changed(&mut rl, &thread, &sprites_path) {
                println!("[usagi] reloaded {}", sprites_path.display());
            }

            // SFX reload. Scan is cheap (just stats); we only pay for
            // new_sound when the manifest actually differs.
            if let Some(a) = &audio
                && sfx.reload_if_changed(a, &sfx_dir)
            {
                println!("[usagi] reloaded sfx ({} sound(s))", sfx.len());
            }
        }

        // Alt+Enter toggles borderless fullscreen. Using is_key_down for alt
        // and is_key_pressed for enter avoids retriggering while alt is held.
        if rl.is_key_pressed(KeyboardKey::KEY_ENTER)
            && (rl.is_key_down(KeyboardKey::KEY_LEFT_ALT)
                || rl.is_key_down(KeyboardKey::KEY_RIGHT_ALT))
        {
            rl.toggle_borderless_windowed();
        }

        // Dev shortcut: `~` (grave/tilde key) toggles the FPS overlay.
        if rl.is_key_pressed(KeyboardKey::KEY_GRAVE) {
            show_fps = !show_fps;
        }

        // Dev shortcut: F5 runs _init() to wipe game state. Always available,
        // in both `run` and `dev`, since it's a one-off action.
        if rl.is_key_pressed(KeyboardKey::KEY_F5)
            && let Ok(init) = lua.globals().get::<LuaFunction>("_init")
        {
            match init.call::<()>(()) {
                Ok(()) => {
                    println!("[usagi] reset (F5)");
                    last_error = None;
                }
                Err(e) => {
                    let msg = format!("_init: {}", e);
                    eprintln!("[usagi] {}", msg);
                    last_error = Some(msg);
                }
            }
        }

        let dt = rl.get_frame_time();
        let screen_w = rl.get_screen_width();
        let screen_h = rl.get_screen_height();
        let fps = rl.get_fps();

        // Update phase. Input and sfx closures borrow rl and the sounds map
        // respectively; errors from user Lua are logged so a broken _update
        // doesn't kill the session.
        if let Some(ref update_fn) = update {
            let rl_ref = &rl;
            let sfx_ref = &sfx;
            record_err(
                &mut last_error,
                "_update",
                lua.scope(|scope| {
                    let input_tbl: LuaTable = lua.globals().get("input")?;
                    let pressed = scope.create_function(|_, key: u32| {
                        Ok(key_from_u32(key).is_some_and(|k| rl_ref.is_key_pressed(k)))
                    })?;
                    input_tbl.set("pressed", pressed)?;
                    let down = scope.create_function(|_, key: u32| {
                        Ok(key_from_u32(key).is_some_and(|k| rl_ref.is_key_down(k)))
                    })?;
                    input_tbl.set("down", down)?;

                    let sfx_tbl: LuaTable = lua.globals().get("sfx")?;
                    let play = scope.create_function(|_, name: String| {
                        sfx_ref.play(&name);
                        Ok(())
                    })?;
                    sfx_tbl.set("play", play)?;

                    update_fn.call::<()>(dt)?;
                    Ok(())
                }),
            );
        }

        // Draw phase. gfx.* share d_rt via RefCell (multiple draw fns need
        // mut access). Errors are logged; the partial RT still gets blitted
        // so the window stays alive.
        {
            let mut d_rt = rl.begin_texture_mode(&thread, &mut rt);
            if let Some(ref draw_fn) = draw {
                let d_rt_cell = std::cell::RefCell::new(&mut d_rt);
                let sprites_ref = sprites.texture();
                let sfx_ref = &sfx;
                record_err(
                    &mut last_error,
                    "_draw",
                    lua.scope(|scope| {
                        let gfx_tbl: LuaTable = lua.globals().get("gfx")?;
                        let clear = scope.create_function(|_, c: i32| {
                            d_rt_cell.borrow_mut().clear_background(palette(c));
                            Ok(())
                        })?;
                        let text =
                            scope.create_function(|_, (s, x, y, c): (String, f32, f32, i32)| {
                                d_rt_cell.borrow_mut().draw_text(
                                    &s,
                                    x.round() as i32,
                                    y.round() as i32,
                                    8,
                                    palette(c),
                                );
                                Ok(())
                            })?;
                        let rect = scope.create_function(
                            |_, (x, y, w, h, c): (f32, f32, f32, f32, i32)| {
                                d_rt_cell.borrow_mut().draw_rectangle(
                                    x.round() as i32,
                                    y.round() as i32,
                                    w.round() as i32,
                                    h.round() as i32,
                                    palette(c),
                                );
                                Ok(())
                            },
                        )?;
                        let spr = scope.create_function(|_, (idx, x, y): (i32, f32, f32)| {
                            // 1-based indexing to match Lua conventions
                            // (ipairs, t[1], string.sub). Sprite 1 is the
                            // top-left cell of the sheet.
                            if idx < 1 {
                                return Ok(());
                            }
                            let idx0 = idx - 1;
                            if let Some(tex) = sprites_ref {
                                const CELL: i32 = 16;
                                let cols = tex.width / CELL;
                                if cols <= 0 {
                                    return Ok(());
                                }
                                let col = idx0 % cols;
                                let row = idx0 / cols;
                                if row * CELL >= tex.height {
                                    return Ok(());
                                }
                                let source = Rectangle {
                                    x: (col * CELL) as f32,
                                    y: (row * CELL) as f32,
                                    width: CELL as f32,
                                    height: CELL as f32,
                                };
                                let pos = Vector2::new(x.round(), y.round());
                                d_rt_cell.borrow_mut().draw_texture_rec(
                                    tex,
                                    source,
                                    pos,
                                    Color::WHITE,
                                );
                            }
                            Ok(())
                        })?;
                        gfx_tbl.set("clear", clear)?;
                        gfx_tbl.set("text", text)?;
                        gfx_tbl.set("rect", rect)?;
                        gfx_tbl.set("spr", spr)?;

                        let sfx_tbl: LuaTable = lua.globals().get("sfx")?;
                        let play = scope.create_function(|_, name: String| {
                            sfx_ref.play(&name);
                            Ok(())
                        })?;
                        sfx_tbl.set("play", play)?;

                        draw_fn.call::<()>(dt)?;
                        Ok(())
                    }),
                );
            }
            if show_fps {
                d_rt.draw_text(&format!("FPS: {}", fps), 0, 0, 8, Color::GREEN);
            }
        }

        // Blit render target to screen, then overlay any active Lua error.
        {
            let mut d = rl.begin_drawing(&thread);
            d.clear_background(Color::BLACK);
            draw_render_target(&mut d, &mut rt, screen_w, screen_h, true);
            if let Some(ref err) = last_error {
                draw_error_overlay(&mut d, err, screen_w, screen_h);
            }
        }
    }
    Ok(())
}
