//! The interactive game session: runs the raylib event loop, drives the
//! Lua VM, handles live reload (if `dev` is true), and renders.
//!
//! State lives on a `Session` struct so we can drive frames identically on
//! native (a `while session.frame() {}` loop) and on emscripten (handing
//! the struct to `emscripten_set_main_loop_arg`, which yields to the
//! browser between frames). Avoiding a blocking native loop on emscripten
//! is what lets us drop ASYNCIFY entirely.

use crate::api::{record_err, setup_api};
use crate::assets::{
    SfxLibrary, SpriteSheet, clear_user_modules, freshest_lua_mtime, install_require, load_script,
};
use crate::input;
use crate::palette::palette;
use crate::render::{draw_error_overlay, draw_render_target};
use crate::vfs::VirtualFs;
use crate::{GAME_HEIGHT, GAME_WIDTH};

use mlua::prelude::*;
use sola_raylib::prelude::*;
use std::rc::Rc;
use std::time::SystemTime;

/// Argument tuple for `gfx.sspr_ex`: `(sx, sy, sw, sh, dx, dy, dw, dh,
/// flip_x, flip_y)`. Aliased so the closure signature stays readable.
type SsprExArgs = (f32, f32, f32, f32, f32, f32, f32, f32, bool, bool);

/// Installs `usagi.measure_text(text)` once at session creation. The
/// closure captures a `&'static Font` so it's not tied to a per-frame
/// `lua.scope`; user Lua can call it from `_init` for layout-time
/// pre-measurement, or from `_update` / `_draw` for dynamic strings.
/// Returns two values: width and height in pixels.
fn register_usagi_measure_text(lua: &Lua, font: &'static Font) -> LuaResult<()> {
    let usagi: LuaTable = lua.globals().get("usagi")?;
    let measure = lua.create_function(move |_, s: String| {
        let m = font.measure_text(&s, crate::font::MONOGRAM_SIZE as f32, 0.0);
        Ok((m.x as i32, m.y as i32))
    })?;
    usagi.set("measure_text", measure)?;
    Ok(())
}

/// User-visible engine config returned by `_config()`. Read once before the
/// window opens. All fields are optional; missing fields fall back to
/// engine defaults.
struct Config {
    /// title shown in the window chrome and app switcher
    title: String,
    /// when true, the render target is upscaled at integer multiples (with
    /// black bars on non-multiple window sizes); when false, it stretches
    /// to fill the window. Defaults to `true` for crisp pixel art.
    pixel_perfect: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            title: "Usagi".to_string(),
            pixel_perfect: true,
        }
    }
}

/// Calls the user's `_config()` if defined and reads supported fields out
/// of its return table. `_config()` raising or returning a non-table is
/// surfaced via `last_error` so the user sees it on the overlay; missing
/// fields silently fall back to defaults.
fn read_config(lua: &Lua, last_error: &mut Option<String>) -> Config {
    let mut config = Config::default();
    let Ok(config_fn) = lua.globals().get::<LuaFunction>("_config") else {
        return config;
    };
    match config_fn.call::<LuaTable>(()) {
        Ok(tbl) => {
            if let Ok(t) = tbl.get::<String>("title") {
                config.title = t;
            }
            if let Ok(t) = tbl.get::<bool>("pixel_perfect") {
                config.pixel_perfect = t;
            }
        }
        Err(e) => {
            let msg = format!("_config: {}", e);
            eprintln!("[usagi] {}", msg);
            *last_error = Some(msg);
        }
    }
    config
}

/// All long-lived session state. Constructed once, frame() called once per
/// iteration. Owning everything (rather than holding references) lets us
/// pass a stable pointer to emscripten_set_main_loop_arg.
///
/// Field order matters: structs drop fields in declaration order, so GPU
/// resources (`rt`, `sprites`) must come before `rl`. Otherwise `rl`'s
/// `Drop` calls `CloseWindow` first, killing the GL context, and the
/// subsequent texture unloads segfault.
struct Session {
    // GPU resources: dropped first, while the GL context is still alive.
    rt: RenderTexture2D,
    sprites: SpriteSheet,
    /// Bundled monogram font. Leaked to `'static` so `usagi.measure_text`
    /// can capture a reference in a non-scoped Lua closure (callable
    /// from `_init` / `_update` / `_draw` alike). The font lives for
    /// program lifetime in practice; this matches how `audio` is leaked
    /// and is reclaimed by process exit.
    font: &'static Font,

    lua: Lua,
    update: Option<LuaFunction>,
    draw: Option<LuaFunction>,

    /// `audio` is leaked to give it a `'static` lifetime so `Sound<'static>`
    /// can be stored alongside it in the same struct without self-reference
    /// pain. The audio device lives for program lifetime anyway; this is
    /// not a real leak (process exit reclaims it).
    audio: Option<&'static RaylibAudio>,
    sfx: SfxLibrary<'static>,

    last_error: Option<String>,
    last_modified: Option<SystemTime>,
    show_fps: bool,
    config: Config,

    /// Wall-clock seconds since the session started. Mirrored into the
    /// `usagi.elapsed` Lua field at the start of each frame, before
    /// `_update`. f64 to avoid f32 precision drift over hour-long runs.
    elapsed: f64,

    vfs: Rc<dyn VirtualFs>,
    reload: bool,

    // Raylib handle last: drops after every GPU resource above, so
    // `CloseWindow` runs only once textures/render targets are unloaded.
    thread: RaylibThread,
    rl: RaylibHandle,
}

impl Session {
    fn new(vfs: Rc<dyn VirtualFs>, dev: bool) -> crate::Result<Self> {
        let reload = dev && vfs.supports_reload();

        let lua = Lua::new();
        // Generational GC fits game workloads (lots of short-lived per-frame
        // allocations, small set of long-lived state).
        lua.gc_gen(0, 0);
        setup_api(&lua, dev)?;
        install_require(&lua, vfs.clone())
            .map_err(|e| crate::Error::Cli(format!("installing require: {e}")))?;

        let mut last_error: Option<String> = None;

        record_err(
            &mut last_error,
            "initial load",
            load_script(&lua, vfs.as_ref()),
        );

        let config = read_config(&lua, &mut last_error);

        // `.highdpi()` and `.resizable()` are desktop-only: on emscripten
        // they fight the JS shell's CSS scaling. `.highdpi()` doubles the
        // canvas backing-store via devicePixelRatio. `.resizable()` makes
        // raylib's emscripten resize callback set the canvas backing-store
        // to `window.innerWidth × window.innerHeight` on every resize event
        // (and one fires at page load), stretching the framebuffer to
        // viewport dims and breaking aspect ratio. On web we keep the
        // backing-store at GAME_WIDTH*2 × GAME_HEIGHT*2 and let the shell's
        // CSS upscale via `image-rendering: pixelated`.
        let mut builder = sola_raylib::init();
        builder
            .size((GAME_WIDTH * 2.) as i32, (GAME_HEIGHT * 2.) as i32)
            .title(&config.title);
        #[cfg(not(target_os = "emscripten"))]
        {
            builder.highdpi().resizable();
        }
        let (mut rl, thread) = builder.build();
        // On web, the browser drives the frame rate through
        // `emscripten_set_main_loop_arg` at requestAnimationFrame rate
        // (60 Hz on most monitors). Don't call `set_target_fps`: raylib's
        // implementation uses `emscripten_sleep` for the pacing wait,
        // which requires ASYNCIFY (we deliberately don't link with it).
        #[cfg(not(target_os = "emscripten"))]
        rl.set_target_fps(60);
        // Don't let resizing shrink the window below the game's native
        // resolution: smaller than that and `pixel_perfect` falls below 1×.
        #[cfg(not(target_os = "emscripten"))]
        rl.set_window_min_size(GAME_WIDTH as i32, GAME_HEIGHT as i32);
        // raylib defaults Esc to quit, which is useful while iterating
        // (`usagi dev`) but a foot-gun for shipped games where a player
        // hitting Esc to dismiss a menu would close the game. Keep it in
        // dev, drop it everywhere else.
        if !dev {
            rl.set_exit_key(None);
        }
        let rt: RenderTexture2D = rl
            .load_render_texture(&thread, GAME_WIDTH as u32, GAME_HEIGHT as u32)
            .unwrap();

        // Load the font before `_init` runs so we can register
        // `usagi.measure_text` against a leaked `&'static Font`. That
        // makes the function callable from any callback (including
        // `_init`), not just from inside per-frame scopes.
        let sprites = SpriteSheet::load(&mut rl, &thread, vfs.as_ref());
        let font: &'static Font = &*Box::leak(Box::new(crate::font::load(&mut rl, &thread)));

        register_usagi_measure_text(&lua, font)
            .map_err(|e| crate::Error::Cli(format!("registering usagi.measure_text: {e}")))?;

        if let Ok(init) = lua.globals().get::<LuaFunction>("_init") {
            record_err(&mut last_error, "_init", init.call::<()>(()));
        }
        let update: Option<LuaFunction> = lua.globals().get("_update").ok();
        let draw: Option<LuaFunction> = lua.globals().get("_draw").ok();
        // Baseline includes every module main.lua already required, so
        // the first frame doesn't spuriously reload just because a sibling
        // module's mtime is newer than main.lua's.
        let last_modified = freshest_lua_mtime(&lua, vfs.as_ref());

        let audio: Option<&'static RaylibAudio> = RaylibAudio::init_audio_device()
            .map_err(|e| eprintln!("[usagi] audio init failed: {}", e))
            .ok()
            .map(|a| &*Box::leak(Box::new(a)));

        let sfx = match audio {
            Some(a) => SfxLibrary::load(a, vfs.as_ref()),
            None => SfxLibrary::empty(),
        };

        Ok(Self {
            rt,
            sprites,
            font,
            lua,
            update,
            draw,
            audio,
            sfx,
            last_error,
            last_modified,
            show_fps: false,
            config,
            elapsed: 0.0,
            vfs,
            reload,
            thread,
            rl,
        })
    }

    /// Runs a single frame. Returns false when the user has closed the
    /// window (only meaningful on native — browsers handle close themselves).
    fn frame(&mut self) -> bool {
        if self.rl.window_should_close() {
            return false;
        }

        if self.reload {
            self.maybe_reload_assets();
        }
        self.handle_global_shortcuts();

        let dt = self.rl.get_frame_time();
        let screen_w = self.rl.get_screen_width();
        let screen_h = self.rl.get_screen_height();
        let fps = self.rl.get_fps();

        // Bump elapsed and mirror it into Lua before _update sees the
        // frame. Best-effort: if the Lua side has clobbered `usagi`
        // somehow, don't tear down the session over it.
        self.elapsed += dt as f64;
        if let Ok(usagi_tbl) = self.lua.globals().get::<LuaTable>("usagi") {
            let _ = usagi_tbl.set("elapsed", self.elapsed);
        }

        self.run_update(dt);
        self.run_draw(dt, fps);
        self.blit_and_overlay(screen_w, screen_h);
        true
    }

    fn maybe_reload_assets(&mut self) {
        // Script reload: re-exec on mtime change. State is preserved (no
        // _init); F5 is the explicit reset. Errors are logged and the
        // previous callbacks keep running so a half-saved file can't kill
        // the session.
        let new_mtime = freshest_lua_mtime(&self.lua, self.vfs.as_ref());
        if new_mtime.is_some() && new_mtime != self.last_modified {
            // Drop cached require results so dependencies re-execute when
            // main.lua re-runs. Built-in libs are untouched.
            if let Err(e) = clear_user_modules(&self.lua, self.vfs.as_ref()) {
                eprintln!("[usagi] clear_user_modules: {e}");
            }
            match load_script(&self.lua, self.vfs.as_ref()) {
                Ok(()) => {
                    println!("[usagi] reloaded {}", self.vfs.script_name());
                    self.update = self.lua.globals().get("_update").ok();
                    self.draw = self.lua.globals().get("_draw").ok();
                    self.last_error = None;
                }
                Err(e) => {
                    let msg = format!("reload: {}", e);
                    eprintln!("[usagi] {}", msg);
                    self.last_error = Some(msg);
                }
            }
            // Recompute after reload: the just-required modules are now
            // in package.loaded with their fresh mtimes, and we want to
            // baseline against THOSE rather than the pre-reload value.
            self.last_modified = freshest_lua_mtime(&self.lua, self.vfs.as_ref());
        }

        if self
            .sprites
            .reload_if_changed(&mut self.rl, &self.thread, self.vfs.as_ref())
        {
            println!("[usagi] reloaded sprites.png");
        }

        if let Some(a) = self.audio
            && self.sfx.reload_if_changed(a, self.vfs.as_ref())
        {
            println!("[usagi] reloaded sfx ({} sound(s))", self.sfx.len());
        }
    }

    fn handle_global_shortcuts(&mut self) {
        // Alt+Enter toggles borderless fullscreen.
        if self.rl.is_key_pressed(KeyboardKey::KEY_ENTER)
            && (self.rl.is_key_down(KeyboardKey::KEY_LEFT_ALT)
                || self.rl.is_key_down(KeyboardKey::KEY_RIGHT_ALT))
        {
            self.rl.toggle_borderless_windowed();
        }

        // ~ toggles the FPS overlay.
        if self.rl.is_key_pressed(KeyboardKey::KEY_GRAVE) {
            self.show_fps = !self.show_fps;
        }

        // F5 / Ctrl+R / Cmd+R run _init() to wipe game state. Always
        // available, in both `run` and `dev`, since it's a one-off action.
        // Caps Lock as a modifier: many users remap caps→ctrl at the OS
        // level, but raylib/GLFW often sees the raw scancode and misses the
        // remap. Accepting caps directly here makes those setups work.
        let ctrl_held = self.rl.is_key_down(KeyboardKey::KEY_LEFT_CONTROL)
            || self.rl.is_key_down(KeyboardKey::KEY_RIGHT_CONTROL)
            || self.rl.is_key_down(KeyboardKey::KEY_CAPS_LOCK);
        let super_held = self.rl.is_key_down(KeyboardKey::KEY_LEFT_SUPER)
            || self.rl.is_key_down(KeyboardKey::KEY_RIGHT_SUPER);
        let reset = self.rl.is_key_pressed(KeyboardKey::KEY_F5)
            || (self.rl.is_key_pressed(KeyboardKey::KEY_R) && (ctrl_held || super_held));
        if reset && let Ok(init) = self.lua.globals().get::<LuaFunction>("_init") {
            match init.call::<()>(()) {
                Ok(()) => {
                    println!("[usagi] reset");
                    self.last_error = None;
                }
                Err(e) => {
                    let msg = format!("_init: {}", e);
                    eprintln!("[usagi] {}", msg);
                    self.last_error = Some(msg);
                }
            }
        }
    }

    fn run_update(&mut self, dt: f32) {
        let Self {
            lua,
            rl,
            sfx,
            update,
            last_error,
            ..
        } = self;
        let Some(update_fn) = update.as_ref() else {
            return;
        };
        let rl_ref: &RaylibHandle = rl;
        let sfx_ref: &SfxLibrary<'static> = sfx;
        record_err(
            last_error,
            "_update",
            lua.scope(|scope| {
                let input_tbl: LuaTable = lua.globals().get("input")?;
                let pressed = scope
                    .create_function(|_, action: u32| Ok(input::action_pressed(rl_ref, action)))?;
                input_tbl.set("pressed", pressed)?;
                let down = scope
                    .create_function(|_, action: u32| Ok(input::action_down(rl_ref, action)))?;
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

    fn run_draw(&mut self, dt: f32, fps: u32) {
        let Self {
            lua,
            rl,
            thread,
            rt,
            sfx,
            sprites,
            font,
            draw,
            last_error,
            show_fps,
            ..
        } = self;
        let mut d_rt = rl.begin_texture_mode(thread, rt);
        if let Some(draw_fn) = draw.as_ref() {
            let d_rt_cell = std::cell::RefCell::new(&mut d_rt);
            let sprites_ref = sprites.texture();
            let font_ref: &Font = font;
            let sfx_ref: &SfxLibrary<'static> = sfx;
            record_err(
                last_error,
                "_draw",
                lua.scope(|scope| {
                    let gfx_tbl: LuaTable = lua.globals().get("gfx")?;
                    let clear = scope.create_function(|_, c: i32| {
                        d_rt_cell.borrow_mut().clear_background(palette(c));
                        Ok(())
                    })?;
                    let text =
                        scope.create_function(|_, (s, x, y, c): (String, f32, f32, i32)| {
                            d_rt_cell.borrow_mut().draw_text_ex(
                                font_ref,
                                &s,
                                Vector2::new(x.round(), y.round()),
                                crate::font::MONOGRAM_SIZE as f32,
                                0.0,
                                palette(c),
                            );
                            Ok(())
                        })?;
                    let rect = scope.create_function(
                        |_, (x, y, w, h, c): (f32, f32, f32, f32, i32)| {
                            d_rt_cell.borrow_mut().draw_rectangle_lines(
                                x.round() as i32,
                                y.round() as i32,
                                w.round() as i32,
                                h.round() as i32,
                                palette(c),
                            );
                            Ok(())
                        },
                    )?;
                    let rect_fill = scope.create_function(
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
                    let circ = scope.create_function(|_, (x, y, r, c): (f32, f32, f32, i32)| {
                        d_rt_cell.borrow_mut().draw_circle_lines(
                            x.round() as i32,
                            y.round() as i32,
                            r,
                            palette(c),
                        );
                        Ok(())
                    })?;
                    let circ_fill =
                        scope.create_function(|_, (x, y, r, c): (f32, f32, f32, i32)| {
                            d_rt_cell.borrow_mut().draw_circle(
                                x.round() as i32,
                                y.round() as i32,
                                r,
                                palette(c),
                            );
                            Ok(())
                        })?;
                    let line = scope.create_function(
                        |_, (x1, y1, x2, y2, c): (f32, f32, f32, f32, i32)| {
                            d_rt_cell.borrow_mut().draw_line(
                                x1.round() as i32,
                                y1.round() as i32,
                                x2.round() as i32,
                                y2.round() as i32,
                                palette(c),
                            );
                            Ok(())
                        },
                    )?;
                    let pixel = scope.create_function(|_, (x, y, c): (f32, f32, i32)| {
                        d_rt_cell.borrow_mut().draw_pixel(
                            x.round() as i32,
                            y.round() as i32,
                            palette(c),
                        );
                        Ok(())
                    })?;
                    // Resolves a 1-based sprite index into a (col, row,
                    // CELL) tuple on the loaded sheet, or None for
                    // out-of-range / no-sheet. Shared between `spr`
                    // and `spr_flipped` so the bookkeeping stays in
                    // one place.
                    fn cell_for(tex: &Texture2D, idx: i32) -> Option<(i32, i32, i32)> {
                        const CELL: i32 = 16;
                        if idx < 1 {
                            return None;
                        }
                        let cols = tex.width / CELL;
                        if cols <= 0 {
                            return None;
                        }
                        let idx0 = idx - 1;
                        let col = idx0 % cols;
                        let row = idx0 / cols;
                        if row * CELL >= tex.height {
                            return None;
                        }
                        Some((col, row, CELL))
                    }
                    let spr = scope.create_function(|_, (idx, x, y): (i32, f32, f32)| {
                        if let Some(tex) = sprites_ref
                            && let Some((col, row, cell)) = cell_for(tex, idx)
                        {
                            let source = Rectangle {
                                x: (col * cell) as f32,
                                y: (row * cell) as f32,
                                width: cell as f32,
                                height: cell as f32,
                            };
                            let pos = Vector2::new(x.round(), y.round());
                            d_rt_cell
                                .borrow_mut()
                                .draw_texture_rec(tex, source, pos, Color::WHITE);
                        }
                        Ok(())
                    })?;
                    let spr_ex = scope.create_function(
                        |_, (idx, x, y, flip_x, flip_y): (i32, f32, f32, bool, bool)| {
                            if let Some(tex) = sprites_ref
                                && let Some((col, row, cell)) = cell_for(tex, idx)
                            {
                                // Negative source dimensions flip the
                                // texture in `draw_texture_pro`.
                                let sw = if flip_x { -cell } else { cell } as f32;
                                let sh = if flip_y { -cell } else { cell } as f32;
                                let source = Rectangle {
                                    x: (col * cell) as f32,
                                    y: (row * cell) as f32,
                                    width: sw,
                                    height: sh,
                                };
                                let dest = Rectangle {
                                    x: x.round(),
                                    y: y.round(),
                                    width: cell as f32,
                                    height: cell as f32,
                                };
                                d_rt_cell.borrow_mut().draw_texture_pro(
                                    tex,
                                    source,
                                    dest,
                                    Vector2::zero(),
                                    0.0,
                                    Color::WHITE,
                                );
                            }
                            Ok(())
                        },
                    )?;
                    let sspr = scope.create_function(
                        |_, (sx, sy, sw, sh, dx, dy): (f32, f32, f32, f32, f32, f32)| {
                            if let Some(tex) = sprites_ref {
                                let source = Rectangle {
                                    x: sx,
                                    y: sy,
                                    width: sw,
                                    height: sh,
                                };
                                let pos = Vector2::new(dx.round(), dy.round());
                                d_rt_cell.borrow_mut().draw_texture_rec(
                                    tex,
                                    source,
                                    pos,
                                    Color::WHITE,
                                );
                            }
                            Ok(())
                        },
                    )?;
                    // Source-rect draw with full power: arbitrary
                    // source rect, arbitrary dest size, plus flips.
                    // All 10 args required — if you want a quick 1:1
                    // draw use `gfx.sspr`, and write your own thin
                    // wrapper if a particular flag combination shows
                    // up often in your code.
                    let sspr_ex = scope.create_function(
                        |_, (sx, sy, sw, sh, dx, dy, dw, dh, flip_x, flip_y): SsprExArgs| {
                            if let Some(tex) = sprites_ref {
                                let src_w = if flip_x { -sw } else { sw };
                                let src_h = if flip_y { -sh } else { sh };
                                let source = Rectangle {
                                    x: sx,
                                    y: sy,
                                    width: src_w,
                                    height: src_h,
                                };
                                let dest = Rectangle {
                                    x: dx.round(),
                                    y: dy.round(),
                                    width: dw,
                                    height: dh,
                                };
                                d_rt_cell.borrow_mut().draw_texture_pro(
                                    tex,
                                    source,
                                    dest,
                                    Vector2::zero(),
                                    0.0,
                                    Color::WHITE,
                                );
                            }
                            Ok(())
                        },
                    )?;
                    gfx_tbl.set("clear", clear)?;
                    gfx_tbl.set("text", text)?;
                    gfx_tbl.set("rect", rect)?;
                    gfx_tbl.set("rect_fill", rect_fill)?;
                    gfx_tbl.set("circ", circ)?;
                    gfx_tbl.set("circ_fill", circ_fill)?;
                    gfx_tbl.set("line", line)?;
                    gfx_tbl.set("pixel", pixel)?;
                    gfx_tbl.set("spr", spr)?;
                    gfx_tbl.set("spr_ex", spr_ex)?;
                    gfx_tbl.set("sspr", sspr)?;
                    gfx_tbl.set("sspr_ex", sspr_ex)?;

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
        if *show_fps {
            d_rt.draw_text_ex(
                font,
                &format!("FPS: {}", fps),
                Vector2::new(0.0, 0.0),
                crate::font::MONOGRAM_SIZE as f32,
                0.0,
                Color::GREEN,
            );
        }
    }

    fn blit_and_overlay(&mut self, screen_w: i32, screen_h: i32) {
        let mut d = self.rl.begin_drawing(&self.thread);
        d.clear_background(Color::BLACK);
        draw_render_target(
            &mut d,
            &mut self.rt,
            screen_w,
            screen_h,
            self.config.pixel_perfect,
        );
        if let Some(ref err) = self.last_error {
            draw_error_overlay(&mut d, self.font, err, screen_w, screen_h);
        }
    }
}

/// Runs a Usagi game session. The `vfs` supplies the script, sprites, and
/// sfx (either from disk or a fused bundle). When `dev` is true AND the
/// vfs supports reload, files are re-read on mtime change. F5 always
/// resets state via `_init()`.
pub fn run(vfs: Rc<dyn VirtualFs>, dev: bool) -> crate::Result<()> {
    let session = Session::new(vfs, dev)?;

    #[cfg(target_os = "emscripten")]
    {
        run_emscripten(Box::new(session));
        // emscripten unwinds the call stack via the JS event loop, so we
        // never get past set_main_loop_arg in practice. Satisfy the type.
        return Ok(());
    }

    #[cfg(not(target_os = "emscripten"))]
    {
        let mut session = session;
        while session.frame() {}
        Ok(())
    }
}

#[cfg(target_os = "emscripten")]
unsafe extern "C" {
    fn emscripten_set_main_loop_arg(
        func: extern "C" fn(*mut std::ffi::c_void),
        arg: *mut std::ffi::c_void,
        fps: i32,
        simulate_infinite_loop: i32,
    );
}

#[cfg(target_os = "emscripten")]
extern "C" fn frame_callback(arg: *mut std::ffi::c_void) {
    // SAFETY: `arg` was set in `run_emscripten` from `Box::into_raw(Box<Session>)`
    // and is exclusively owned by the loop. No other code touches it.
    let session: &mut Session = unsafe { &mut *(arg as *mut Session) };
    session.frame();
}

#[cfg(target_os = "emscripten")]
fn run_emscripten(session: Box<Session>) {
    // `Box::into_raw` gives us a stable pointer; the browser owns the
    // pointer for the rest of the program (the tab being closed reclaims
    // it). simulate_infinite_loop=1 tells emscripten to throw a JS
    // unwinding exception so control never returns to us.
    let session_ptr = Box::into_raw(session) as *mut std::ffi::c_void;
    unsafe {
        emscripten_set_main_loop_arg(
            frame_callback,
            session_ptr,
            0, // fps; 0 = drive at requestAnimationFrame rate (matches refresh)
            1, // simulate_infinite_loop
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_returns_title_field() {
        let lua = Lua::new();
        setup_api(&lua, false).unwrap();
        lua.load(
            r#"
            function _config()
              return { title = "Hello, Usagi!" }
            end
            "#,
        )
        .exec()
        .unwrap();
        let mut err = None;
        let config = read_config(&lua, &mut err);
        assert_eq!(config.title, "Hello, Usagi!");
        assert!(err.is_none());
    }

    #[test]
    fn config_returns_pixel_perfect_field() {
        let lua = Lua::new();
        setup_api(&lua, false).unwrap();
        lua.load("function _config() return { pixel_perfect = false } end")
            .exec()
            .unwrap();
        let mut err = None;
        let config = read_config(&lua, &mut err);
        assert!(!config.pixel_perfect);
        assert!(err.is_none());
    }

    #[test]
    fn missing_config_pixel_perfect_defaults_to_true() {
        let lua = Lua::new();
        setup_api(&lua, false).unwrap();
        let mut err = None;
        let config = read_config(&lua, &mut err);
        assert!(config.pixel_perfect, "default should be pixel-perfect on");
    }

    #[test]
    fn missing_config_returns_defaults() {
        let lua = Lua::new();
        setup_api(&lua, false).unwrap();
        let mut err = None;
        let config = read_config(&lua, &mut err);
        assert_eq!(config.title, "Usagi");
        assert!(err.is_none());
    }

    #[test]
    fn config_with_no_title_field_returns_default_title() {
        let lua = Lua::new();
        setup_api(&lua, false).unwrap();
        lua.load("function _config() return {} end").exec().unwrap();
        let mut err = None;
        let config = read_config(&lua, &mut err);
        assert_eq!(config.title, "Usagi");
        assert!(err.is_none());
    }

    #[test]
    fn config_runtime_error_is_recorded() {
        let lua = Lua::new();
        setup_api(&lua, false).unwrap();
        lua.load(r#"function _config() error("bad config") end"#)
            .exec()
            .unwrap();
        let mut err = None;
        let _ = read_config(&lua, &mut err);
        let stored = err.expect("error should have been recorded");
        assert!(stored.starts_with("_config: "), "got: {stored}");
        assert!(stored.contains("bad config"), "got: {stored}");
    }

    #[test]
    fn config_returning_non_table_is_recorded() {
        let lua = Lua::new();
        setup_api(&lua, false).unwrap();
        lua.load(r#"function _config() return 42 end"#)
            .exec()
            .unwrap();
        let mut err = None;
        let _ = read_config(&lua, &mut err);
        assert!(err.is_some());
    }
}
