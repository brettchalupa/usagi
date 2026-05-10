//! The interactive game session: runs the raylib event loop, drives the
//! Lua VM, handles live reload (if `dev` is true), and renders.
//!
//! State lives on a `Session` struct so we can drive frames identically on
//! native (a `while session.frame() {}` loop) and on emscripten (handing
//! the struct to `emscripten_set_main_loop_arg`, which yields to the
//! browser between frames). Avoiding a blocking native loop on emscripten
//! is what lets us drop ASYNCIFY entirely.

use crate::api::{record_err, register_shader_api, setup_api, wrap};
use crate::assets::{
    MusicLibrary, SfxLibrary, SpriteSheet, clear_user_modules, install_require, load_script,
};
#[cfg(not(target_os = "emscripten"))]
use crate::capture::{Recorder, save_screenshot};
use crate::effect::Effects;
use crate::input;
use crate::palette::color;
use crate::pause::{PauseAction, PauseMenu};
use crate::render::{draw_error_overlay, draw_render_target, game_view_transform};
use crate::shader::ShaderManager;
use crate::vfs::VirtualFs;

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
    usagi.set(
        "measure_text",
        wrap(lua, measure, "usagi.measure_text", &["string"])?,
    )?;
    Ok(())
}

/// Shared cells that bridge the Lua `input.*` closures to raylib state.
/// All four are `Rc`s so they can be captured by individual closures
/// while the session also retains them for the per-frame
/// sample/apply step. `Cell` is enough because Lua is single-threaded
/// and the values are `Copy`. Bundled into a struct so the session
/// only holds one field instead of four.
struct InputBridge {
    /// Latest input snapshot. Refreshed at the top of every frame so
    /// `_update` and `_draw` see the same values.
    state: Rc<std::cell::Cell<input::InputState>>,
    /// Last visibility the user requested via `input.set_mouse_visible`.
    /// Read by `input.mouse_visible` so it reflects the latest
    /// request even before the session has applied it to raylib.
    cursor_visible: Rc<std::cell::Cell<bool>>,
    /// Set by `input.set_mouse_visible(v)` and consumed by the session
    /// at frame start. Deferring lets the closure stay safe (no
    /// `&mut RaylibHandle`) while still toggling actual raylib state
    /// before the first draw call sees the new visibility.
    pending_cursor: Rc<std::cell::Cell<Option<bool>>>,
}

impl InputBridge {
    fn new() -> Self {
        Self {
            state: Rc::new(std::cell::Cell::new(input::InputState::default())),
            cursor_visible: Rc::new(std::cell::Cell::new(true)),
            pending_cursor: Rc::new(std::cell::Cell::new(None)),
        }
    }
}

/// Installs the full `input.*` Lua surface (queries plus cursor
/// toggles) once at session startup. Closures read from / write to the
/// shared cells in `InputBridge`, so they're callable from `_init`,
/// `_update`, and `_draw` without per-frame `lua.scope` rewiring.
fn register_input_api(lua: &Lua, bridge: &InputBridge) -> LuaResult<()> {
    let input: LuaTable = lua.globals().get("input")?;

    let s = Rc::clone(&bridge.state);
    let pressed = lua.create_function(move |_, action: u32| Ok(s.get().action_pressed(action)))?;
    input.set("pressed", wrap(lua, pressed, "input.pressed", &["number"])?)?;

    let s = Rc::clone(&bridge.state);
    let held = lua.create_function(move |_, action: u32| Ok(s.get().action_down(action)))?;
    input.set("held", wrap(lua, held, "input.held", &["number"])?)?;

    let s = Rc::clone(&bridge.state);
    let released =
        lua.create_function(move |_, action: u32| Ok(s.get().action_released(action)))?;
    input.set(
        "released",
        wrap(lua, released, "input.released", &["number"])?,
    )?;

    let s = Rc::clone(&bridge.state);
    let mapping_for = lua.create_function(move |_, action: u32| {
        Ok(s.get().mapping_for(action).map(str::to_string))
    })?;
    input.set(
        "mapping_for",
        wrap(lua, mapping_for, "input.mapping_for", &["number"])?,
    )?;

    let s = Rc::clone(&bridge.state);
    let last_source = lua.create_function(move |_, ()| Ok(s.get().last_source().as_str()))?;
    input.set(
        "last_source",
        wrap(lua, last_source, "input.last_source", &[])?,
    )?;

    let s = Rc::clone(&bridge.state);
    let mouse = lua.create_function(move |_, ()| Ok(s.get().mouse_position()))?;
    input.set("mouse", wrap(lua, mouse, "input.mouse", &[])?)?;

    let s = Rc::clone(&bridge.state);
    let mouse_held =
        lua.create_function(move |_, button: u32| Ok(s.get().mouse_button_down(button)))?;
    input.set(
        "mouse_held",
        wrap(lua, mouse_held, "input.mouse_held", &["number"])?,
    )?;

    let s = Rc::clone(&bridge.state);
    let mouse_pressed =
        lua.create_function(move |_, button: u32| Ok(s.get().mouse_button_pressed(button)))?;
    input.set(
        "mouse_pressed",
        wrap(lua, mouse_pressed, "input.mouse_pressed", &["number"])?,
    )?;

    let s = Rc::clone(&bridge.state);
    let mouse_released =
        lua.create_function(move |_, button: u32| Ok(s.get().mouse_button_released(button)))?;
    input.set(
        "mouse_released",
        wrap(lua, mouse_released, "input.mouse_released", &["number"])?,
    )?;

    let s = Rc::clone(&bridge.state);
    let key_held = lua.create_function(move |_, key: u32| Ok(s.get().key_held(key)))?;
    input.set(
        "key_held",
        wrap(lua, key_held, "input.key_held", &["number"])?,
    )?;

    let s = Rc::clone(&bridge.state);
    let key_pressed = lua.create_function(move |_, key: u32| Ok(s.get().key_pressed(key)))?;
    input.set(
        "key_pressed",
        wrap(lua, key_pressed, "input.key_pressed", &["number"])?,
    )?;

    let s = Rc::clone(&bridge.state);
    let key_released = lua.create_function(move |_, key: u32| Ok(s.get().key_released(key)))?;
    input.set(
        "key_released",
        wrap(lua, key_released, "input.key_released", &["number"])?,
    )?;

    let cv = Rc::clone(&bridge.cursor_visible);
    let pc = Rc::clone(&bridge.pending_cursor);
    let set_visible = lua.create_function(move |_, visible: bool| {
        cv.set(visible);
        pc.set(Some(visible));
        Ok(())
    })?;
    input.set(
        "set_mouse_visible",
        wrap(lua, set_visible, "input.set_mouse_visible", &["boolean"])?,
    )?;

    let cv = Rc::clone(&bridge.cursor_visible);
    let is_visible = lua.create_function(move |_, ()| Ok(cv.get()))?;
    input.set(
        "mouse_visible",
        wrap(lua, is_visible, "input.mouse_visible", &[])?,
    )?;

    Ok(())
}

/// Installs `effect.hitstop` / `screen_shake` / `flash` / `slow_mo`
/// / `stop` once at session startup. Closures share an
/// `Rc<RefCell<Effects>>` with the session so writes from any
/// callback (`_init`, `_update`, `_draw`) land in the same per-frame
/// state.
fn register_effect_api(lua: &Lua, effects: &Rc<std::cell::RefCell<Effects>>) -> LuaResult<()> {
    let effect = lua.create_table()?;

    let e = Rc::clone(effects);
    let hitstop = lua.create_function(move |_, time: f32| {
        e.borrow_mut().hitstop(time);
        Ok(())
    })?;
    effect.set(
        "hitstop",
        wrap(lua, hitstop, "effect.hitstop", &["number"])?,
    )?;

    let e = Rc::clone(effects);
    let screen_shake = lua.create_function(move |_, (time, intensity): (f32, f32)| {
        e.borrow_mut().screen_shake(time, intensity);
        Ok(())
    })?;
    effect.set(
        "screen_shake",
        wrap(
            lua,
            screen_shake,
            "effect.screen_shake",
            &["number", "number"],
        )?,
    )?;

    let e = Rc::clone(effects);
    let flash = lua.create_function(move |_, (time, color_index): (f32, i32)| {
        e.borrow_mut().flash(time, color_index);
        Ok(())
    })?;
    effect.set(
        "flash",
        wrap(lua, flash, "effect.flash", &["number", "number"])?,
    )?;

    let e = Rc::clone(effects);
    let slow_mo = lua.create_function(move |_, (time, scale): (f32, f32)| {
        e.borrow_mut().slow_mo(time, scale);
        Ok(())
    })?;
    effect.set(
        "slow_mo",
        wrap(lua, slow_mo, "effect.slow_mo", &["number", "number"])?,
    )?;

    let e = Rc::clone(effects);
    let stop = lua.create_function(move |_, ()| {
        e.borrow_mut().reset();
        Ok(())
    })?;
    effect.set("stop", wrap(lua, stop, "effect.stop", &[])?)?;

    lua.globals().set("effect", effect)?;
    Ok(())
}

/// Installs `music.play` / `music.loop` / `music.stop` once at session
/// startup against a shared `MusicLibrary`. The closures `borrow_mut`
/// the library on each call, so they can be invoked from `_init` (e.g.
/// to start a title track before the first frame), `_update`, or any
/// other callback. Lua is single-threaded and no Lua callback recurses
/// into another, so the runtime borrow check stays satisfied.
fn register_music_api(
    lua: &Lua,
    music: &Rc<std::cell::RefCell<MusicLibrary<'static>>>,
) -> LuaResult<()> {
    let music_tbl: LuaTable = lua.globals().get("music")?;

    let m = Rc::clone(music);
    let play = lua.create_function(move |_, name: String| {
        m.borrow_mut().play(&name);
        Ok(())
    })?;
    music_tbl.set("play", wrap(lua, play, "music.play", &["string"])?)?;

    let m = Rc::clone(music);
    let loop_ = lua.create_function(move |_, name: String| {
        m.borrow_mut().loop_(&name);
        Ok(())
    })?;
    music_tbl.set("loop", wrap(lua, loop_, "music.loop", &["string"])?)?;

    let m = Rc::clone(music);
    let stop = lua.create_function(move |_, ()| {
        m.borrow_mut().stop();
        Ok(())
    })?;
    music_tbl.set("stop", wrap(lua, stop, "music.stop", &[])?)?;

    Ok(())
}

/// Installs `usagi.save(t)` and `usagi.load()` against the resolved
/// `game_id`. Resolution happens once at session creation via
/// `GameId::resolve` (preferring `_config().game_id`, falling back to
/// the project name, then a bundle-hash sentinel), so games that don't
/// set `game_id` still get stable per-game persistence instead of an
/// error. The closures each take their own `GameId` clone because
/// mlua requires `'static` captures.
fn register_save_api(lua: &Lua, game_id: crate::game_id::GameId) -> LuaResult<()> {
    let usagi: LuaTable = lua.globals().get("usagi")?;

    let id_for_save = game_id.clone();
    let save = lua.create_function(move |lua, value: mlua::Value| {
        let json = crate::save::lua_to_json(lua, value)?;
        crate::save::write_save(&id_for_save, &json)
            .map_err(|e| mlua::Error::external(format!("usagi.save: write: {e}")))?;
        Ok(())
    })?;
    usagi.set("save", wrap(lua, save, "usagi.save", &["table"])?)?;

    let id_for_load = game_id;
    let load = lua.create_function(move |lua, ()| match crate::save::read_save(&id_for_load) {
        Ok(None) => Ok(mlua::Value::Nil),
        Ok(Some(s)) => crate::save::json_to_lua(lua, &s),
        Err(e) => Err(mlua::Error::external(format!("usagi.load: read: {e}"))),
    })?;
    usagi.set("load", wrap(lua, load, "usagi.load", &[])?)?;

    Ok(())
}

use crate::config::Config;

/// Reads project config from the live session Lua VM. Errors flow
/// into `last_error` for the on-screen overlay; missing fields fall
/// back to defaults. Thin wrapper over `Config::read_from_lua`,
/// kept here so the call site reads naturally.
fn read_config(lua: &Lua, last_error: &mut Option<String>) -> Config {
    Config::read_from_lua(lua, Some(last_error))
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
    /// Owns the active post-process `Shader` (a GPU resource) so its
    /// `Drop` (UnloadShader) runs while the GL context is still alive.
    /// Must come before `rl` for the same reason `rt` does.
    shader: Rc<std::cell::RefCell<ShaderManager>>,
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
    /// `MusicLibrary` mutates on play/loop/stop/pause/update, so it
    /// lives behind an `Rc<RefCell>`. The session and the Lua-side
    /// `music.*` closures both hold an `Rc`, which lets the closures
    /// be registered once at startup (callable from `_init`) instead
    /// of being scoped per-frame like draw closures.
    music: Rc<std::cell::RefCell<MusicLibrary<'static>>>,

    last_error: Option<String>,
    last_modified: Option<SystemTime>,
    show_fps: bool,
    config: Config,

    /// Wall-clock seconds since the session started. Mirrored into the
    /// `usagi.elapsed` Lua field at the start of each frame, before
    /// `_update`. f64 to avoid f32 precision drift over hour-long runs.
    elapsed: f64,

    /// Engine-level juice state (hitstop, screen shake, flash, slow_mo)
    /// driven by the `effect.*` Lua API. Decayed once per frame in
    /// `frame()` (gated on the pause overlay being closed), then read
    /// from the update gate, dt scaler, blit, and post-draw overlay.
    effects: Rc<std::cell::RefCell<Effects>>,

    /// Most recent palette index passed to `gfx.clear`. Read by the
    /// blit when shake is active so the strips exposed at the edges
    /// of the shifted RT are filled with the game's bg color rather
    /// than letterbox black. Defaults to 0 (black) so a game that
    /// never calls `gfx.clear` gets the prior behavior.
    last_clear: std::cell::Cell<i32>,

    /// Engine-level pause overlay. While `pause.open` is true, `_update`
    /// and `_draw` are skipped and the RT renders a black "PAUSED"
    /// screen instead. Music keeps streaming so background tracks don't
    /// stutter when the player taps in and out.
    pause: PauseMenu,
    /// Shared cells backing the Lua `input.*` API: the latest input
    /// snapshot, current cursor visibility, and any pending visibility
    /// toggle that the frame loop needs to apply via `&mut rl`.
    input_bridge: InputBridge,
    /// Logs gamepad connect / disconnect / hot-swap events so the
    /// player can see what the engine sees. Useful when face buttons
    /// feel wrong: the printed name is the only knob
    /// `GamepadFamily::detect` reads, so a name that doesn't contain
    /// any of the Nintendo / PlayStation substrings gets the Xbox
    /// fallback layout.
    gamepad_probe: input::GamepadProbe,
    /// Per-frame snapshot of analog-stick axis values, consumed by
    /// `action_pressed` / `action_released` so menus can be navigated
    /// with the stick (edge-detected, just like the d-pad).
    axis_edges: input::AxisEdgeTracker,
    /// Mask of inputs the player was holding while the pause menu was
    /// open or as it closed. Suppressed from the Lua-facing
    /// `InputState` until each release, so a BTN1/BTN2 press that
    /// exits the menu doesn't fire in `_update` the same frame.
    input_swallow: input::InputSwallow,
    /// In-game GIF recorder, toggled with F9 / Cmd+G. Native-only:
    /// emscripten has no real filesystem to write to.
    #[cfg(not(target_os = "emscripten"))]
    recorder: Recorder,
    /// Where the recorder and screenshot helper write their `*.gif`
    /// and `*.png` files. One bucket so games have a single dir to
    /// gitignore. Captured at session creation so we don't depend on
    /// CWD changes mid-session.
    #[cfg(not(target_os = "emscripten"))]
    captures_dir: std::path::PathBuf,
    /// Filename prefix for capture files. Derived from the resolved
    /// `game_id` so artifacts read as `<game>-YYYYMMDD-HHMMSS.gif`
    /// (e.g. `snake-...gif`). Stored on the session so the prefix
    /// can't drift across captures within one run.
    #[cfg(not(target_os = "emscripten"))]
    capture_prefix: String,
    /// Per-game settings loaded from disk (or localStorage on web)
    /// at boot. Held on the session so the global mute hotkey can
    /// flip the volume in-place and persist the change.
    settings: crate::settings::Settings,
    /// Per-game keyboard overrides. Read by `input::action_*`; the
    /// pause menu's Configure Keys flow writes through here.
    keymap: crate::keymap::Keymap,
    /// Resolved game id, kept on the session so settings writes
    /// (mute toggles) can address the same per-game storage as save
    /// data. Cloned out of the resolver since `register_save_api`
    /// consumes the original.
    game_id: crate::game_id::GameId,
    /// Set by Shift+Esc in dev to request a clean exit out of the
    /// frame loop. `frame()` checks it before doing any per-frame work.
    should_quit: bool,
    /// Mirror of the `dev` argument to `Session::new`. Stored so
    /// `frame()` can gate the Shift+Esc quit shortcut without taking
    /// `dev` as a parameter on every method.
    dev: bool,

    vfs: Rc<dyn VirtualFs>,
    reload: bool,

    // Raylib handle last: drops after every GPU resource above, so
    // `CloseWindow` runs only once textures/render targets are unloaded.
    thread: RaylibThread,
    rl: RaylibHandle,
}

impl Session {
    fn new(vfs: Rc<dyn VirtualFs>, dev: bool) -> crate::Result<Self> {
        // Log the engine version once at boot so user-submitted bug
        // reports (console / terminal) carry the version stamp without
        // the user having to remember `usagi --version`. Same string
        // shows on web (browser console) and native (terminal).
        crate::msg::info!("usagi v{}", env!("CARGO_PKG_VERSION"));

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

        // Resolve game_id and load settings before the window opens
        // so the fullscreen state can be applied immediately after
        // `builder.build()`. Doing the toggle later (after audio /
        // font / Lua setup) leaves a visible windowed frame on
        // macOS while raylib animates the transition.
        let project_name_hint = vfs.project_name_hint();
        let resolved_game_id = crate::game_id::GameId::resolve(
            config.game_id.as_deref(),
            project_name_hint.as_deref(),
            vfs.as_bundle(),
        );
        let project_name = crate::project_name::ProjectName::resolve(
            config.name.as_deref(),
            project_name_hint.as_deref(),
        );
        #[allow(unused_mut)]
        let mut settings = crate::settings::load(&resolved_game_id);
        // Browsers can't auto-restore fullscreen at startup (requires
        // a user gesture) and raylib's emscripten
        // `ToggleBorderlessWindowed` is broken anyway: it calls
        // `Module.requestFullscreen` via EM_ASM, but that runtime
        // method isn't in our `EXPORTED_RUNTIME_METHODS` list, so the
        // EM_ASM throws and aborts the wasm during `callMain` (black
        // screen, audio still plays from the already-initialized
        // thread). Force `false` on load so the apply-persisted-
        // fullscreen branch below never runs on web, and so any stale
        // `true` left in localStorage by a prior crashing session
        // gets overwritten on the next settings write.
        #[cfg(target_os = "emscripten")]
        {
            settings.fullscreen = false;
        }
        let keymap = crate::keymap::load(&resolved_game_id);
        #[cfg(not(target_os = "emscripten"))]
        let capture_prefix = resolved_game_id.short_name().to_string();

        // Resolved at this point from `_config().game_width / game_height`,
        // defaulting to 320x180 when the user doesn't override.
        let res = config.resolution;

        // Games up to 640x360 open at 2x for readability; bigger
        // games open at 1x so the initial window doesn't blow past
        // common laptop displays. The 640 threshold means the
        // default 320x180 game opens at 640x360 and a 640x360 game
        // opens at 1280x720 (a nice minimal-fullscreen baseline);
        // anything past that lands native-sized. The window is
        // resizable, so users can drag it bigger.
        let win_scale_threshold = crate::config::Resolution::DEFAULT.w * 2.0;
        let win_scale = if res.w.max(res.h) > win_scale_threshold {
            1.0
        } else {
            2.0
        };

        // `.highdpi()` and `.resizable()` are desktop-only: on emscripten
        // they fight the JS shell's CSS scaling. `.highdpi()` doubles the
        // canvas backing-store via devicePixelRatio. `.resizable()` makes
        // raylib's emscripten resize callback set the canvas backing-store
        // to `window.innerWidth × window.innerHeight` on every resize event
        // (and one fires at page load), stretching the framebuffer to
        // viewport dims and breaking aspect ratio. On web we keep the
        // backing-store at res.w*win_scale × res.h*win_scale and let the
        // shell's CSS upscale via `image-rendering: pixelated`.
        let mut builder = sola_raylib::init();
        builder
            .size((res.w * win_scale) as i32, (res.h * win_scale) as i32)
            .title(project_name.display());
        // raylib defaults to LOG_INFO, which prints a screenful of
        // GLFW/GL/audio init details every boot. Drop to LOG_WARNING
        // so real signal (asset load failures, gamepad anomalies, GL
        // fallbacks) still surfaces but the routine chatter doesn't.
        // `USAGI_VERBOSE=1` brings the full log back for debugging.
        let log_level = if std::env::var_os("USAGI_VERBOSE").is_some() {
            TraceLogLevel::LOG_INFO
        } else {
            TraceLogLevel::LOG_WARNING
        };
        builder.log_level(log_level);
        #[cfg(not(target_os = "emscripten"))]
        {
            builder.highdpi().resizable();
        }

        let (mut rl, thread) = builder.build();

        // Apply window icon: configured tile from sprites.png if set,
        // otherwise the embedded usagi default. macOS title bars
        // ignore this (Cocoa limitation); the bundle path in
        // `usagi export` is what makes the Dock icon stick there.
        match (config.icon, vfs.read_sprites()) {
            (Some(n), Some(bytes)) => {
                crate::icon::apply_from_sprites(&mut rl, &bytes, n, config.sprite_size)
            }
            _ => crate::icon::apply(&mut rl),
        }

        // Apply persisted fullscreen as soon as the window exists.
        // Has a visible windowed-frame flash on macOS (raylib's
        // builder doesn't expose `FLAG_BORDERLESS_WINDOWED_MODE`
        // yet); revisit once sola-raylib ships
        // `builder.borderless_windowed()`.
        if settings.fullscreen {
            rl.toggle_borderless_windowed();
        }

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
        rl.set_window_min_size(res.w as i32, res.h as i32);
        rl.set_exit_key(None);
        let rt: RenderTexture2D = rl
            .load_render_texture(&thread, res.w as u32, res.h as u32)
            .unwrap();

        // Mirror the resolved dims into the Lua side immediately so
        // `_init` reads the correct `usagi.GAME_W` / `GAME_H`. The api
        // setup seeded defaults; this writes the active values.
        if let Ok(usagi_tbl) = lua.globals().get::<LuaTable>("usagi") {
            let _ = usagi_tbl.set("GAME_W", res.w);
            let _ = usagi_tbl.set("GAME_H", res.h);
            let _ = usagi_tbl.set("SPRITE_SIZE", config.sprite_size);
        }

        // Load the font before `_init` runs so we can register
        // `usagi.measure_text` against a leaked `&'static Font`. That
        // makes the function callable from any callback (including
        // `_init`), not just from inside per-frame scopes.
        let sprites = SpriteSheet::load(&mut rl, &thread, vfs.as_ref());
        let font: &'static Font = &*Box::leak(Box::new(crate::font::load(&mut rl, &thread)));

        register_usagi_measure_text(&lua, font)
            .map_err(|e| crate::Error::Cli(format!("registering usagi.measure_text: {e}")))?;

        let input_bridge = InputBridge::new();
        let mut axis_edges = input::AxisEdgeTracker::new();
        // Seed the snapshot once so `_init` reads real values (mouse
        // position over the live window, etc.) instead of zeroed defaults.
        input_bridge.state.set(input::InputState::sample(
            &rl,
            res,
            config.pixel_perfect,
            &keymap,
            &axis_edges,
            input::InputSource::default(),
            None,
        ));
        // Roll forward axis state so frame 1's `action_pressed` compares
        // against frame 0's stick position rather than zeros (otherwise a
        // stick already past the deadzone at boot would fire a spurious
        // press on frame 1).
        axis_edges.snapshot(&rl);
        register_input_api(&lua, &input_bridge)
            .map_err(|e| crate::Error::Cli(format!("registering input.* API: {e}")))?;

        // Clone for session retain (mute toggle and fullscreen
        // toggle write settings back under this id) before handing
        // ownership to `register_save_api`. Capture filename prefix
        // is derived from the same id so saves and captures share a
        // name (e.g. `snake-...gif`).
        let game_id = resolved_game_id.clone();
        register_save_api(&lua, resolved_game_id)
            .map_err(|e| crate::Error::Cli(format!("registering usagi.save / usagi.load: {e}")))?;

        // Audio and the music library load before `_init` so games can
        // call `music.play` / `music.loop` from `_init` (e.g. start a
        // title track immediately). The `music.*` Lua closures are
        // registered against an `Rc<RefCell<MusicLibrary>>` that the
        // session also holds, so user calls flow through the same
        // library that the engine drives every frame.
        let audio: Option<&'static RaylibAudio> = RaylibAudio::init_audio_device()
            .map_err(|e| crate::msg::err!("audio init failed: {}", e))
            .ok()
            .map(|a| &*Box::leak(Box::new(a)));

        // Per-channel volumes are applied to the libraries below.
        // Master volume stays at raylib's default (1.0) so the channel
        // settings are the source of truth.
        let (mut sfx, mut music) = match audio {
            Some(a) => (
                SfxLibrary::load(a, vfs.as_ref()),
                MusicLibrary::load(a, vfs.as_ref()),
            ),
            None => (SfxLibrary::empty(), MusicLibrary::empty()),
        };
        sfx.set_volume(settings.sfx_volume);
        music.set_volume(settings.music_volume);
        let music = Rc::new(std::cell::RefCell::new(music));
        register_music_api(&lua, &music)
            .map_err(|e| crate::Error::Cli(format!("registering music.* API: {e}")))?;

        let shader = Rc::new(std::cell::RefCell::new(ShaderManager::new()));
        register_shader_api(&lua, &shader)
            .map_err(|e| crate::Error::Cli(format!("registering gfx.shader_* API: {e}")))?;

        let effects = Rc::new(std::cell::RefCell::new(Effects::new()));
        register_effect_api(&lua, &effects)
            .map_err(|e| crate::Error::Cli(format!("registering effect.* API: {e}")))?;

        if let Ok(init) = lua.globals().get::<LuaFunction>("_init") {
            record_err(&mut last_error, "_init", init.call::<()>(()));
        }
        let update: Option<LuaFunction> = lua.globals().get("_update").ok();
        let draw: Option<LuaFunction> = lua.globals().get("_draw").ok();
        // Baseline against every project .lua file so the first frame
        // doesn't spuriously reload just because a sibling module's mtime
        // is newer than main.lua's.
        let last_modified = vfs.freshest_lua_mtime();

        Ok(Self {
            rt,
            sprites,
            font,
            lua,
            update,
            draw,
            audio,
            sfx,
            music,
            last_error,
            last_modified,
            show_fps: false,
            config,
            elapsed: 0.0,
            effects,
            last_clear: std::cell::Cell::new(0),
            pause: PauseMenu::new(),
            input_bridge,
            gamepad_probe: input::GamepadProbe::new(),
            axis_edges,
            input_swallow: input::InputSwallow::new(),
            #[cfg(not(target_os = "emscripten"))]
            recorder: Recorder::new(),
            // Captures (gifs + screenshots) land in `<cwd>/captures/`.
            // CWD is the project dir for `usagi dev` / `usagi run`,
            // and is the exec dir for fused / exported binaries.
            // Print the absolute path on save.
            #[cfg(not(target_os = "emscripten"))]
            captures_dir: std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join("captures"),
            #[cfg(not(target_os = "emscripten"))]
            capture_prefix,
            settings,
            keymap,
            game_id,
            should_quit: false,
            dev,
            shader,
            vfs,
            reload,
            thread,
            rl,
        })
    }

    /// Runs a single frame. Returns false when the user has closed the
    /// window (only meaningful on native — browsers handle close themselves).
    fn frame(&mut self) -> bool {
        if self.rl.window_should_close() || self.should_quit {
            return false;
        }

        // Logs gamepad connect / disconnect once each, so a misdetected
        // controller (e.g. Switch Pro showing as "Wireless Gamepad" and
        // falling back to the Xbox face layout) is debuggable from the
        // CLI output alone.
        self.gamepad_probe.poll(&self.rl);

        if self.reload {
            self.maybe_reload_assets();
        }
        self.handle_global_shortcuts();

        let dt = self.rl.get_frame_time();
        let pause_action = self.pause.update(
            &mut self.rl,
            &self.settings,
            &self.keymap,
            &self.axis_edges,
            dt,
        );
        if let Some(action) = pause_action {
            self.apply_pause_action(action);
        }
        if self.should_quit {
            return false;
        }

        let screen_w = self.rl.get_screen_width();
        let screen_h = self.rl.get_screen_height();
        let fps = self.rl.get_fps();

        // Refresh the input snapshot once per frame so the Lua-side
        // `input.*` closures see consistent values throughout `_update`
        // and `_draw`. raylib polls input once per frame anyway, so
        // sampling here matches what live calls would return.
        let prior_state = self.input_bridge.state.get();
        let mut sampled = input::InputState::sample(
            &self.rl,
            self.config.resolution,
            self.config.pixel_perfect,
            &self.keymap,
            &self.axis_edges,
            prior_state.last_source(),
            prior_state.last_pad(),
        );
        // Refresh the swallow mask while the menu is up or just
        // closed; otherwise drain it as the player releases each
        // suppressed input. Apply before storing so user Lua never
        // sees a BTN1/BTN2 press that was actually meant for the
        // pause menu.
        let capture_swallow = self.pause.open || self.pause.just_closed();
        self.input_swallow.update(&sampled, capture_swallow);
        self.input_swallow.apply(&mut sampled);
        self.input_bridge.state.set(sampled);
        // Snapshot after this frame's `action_pressed` reads, so next
        // frame compares the live stick against this frame's value.
        self.axis_edges.snapshot(&self.rl);

        // Apply any cursor-visibility toggle that user Lua requested
        // last frame (or during `_init`). Done here while `&mut rl` is
        // free, before `begin_texture_mode` borrows it.
        if let Some(visible) = self.input_bridge.pending_cursor.take() {
            input::set_mouse_visible(&mut self.rl, visible);
        }

        // Bump elapsed and mirror it into Lua before _update sees the
        // frame. Best-effort: if the Lua side has clobbered `usagi`
        // somehow, don't tear down the session over it.
        self.elapsed += dt as f64;
        if let Ok(usagi_tbl) = self.lua.globals().get::<LuaTable>("usagi") {
            let _ = usagi_tbl.set("elapsed", self.elapsed);
        }

        if self.pause.just_opened() {
            self.music.borrow_mut().pause()
        }
        if self.pause.just_closed() {
            self.music.borrow_mut().resume()
        }
        self.music.borrow_mut().update();

        if self.pause.open {
            self.run_draw(dt, fps);
            self.draw_paused();
        } else {
            // Decay juice timers with real wall-clock dt before
            // anything reads them, so hitstop/shake/flash/slow_mo
            // expire on schedule even when slow_mo is active.
            self.effects.borrow_mut().tick(dt);
            let frozen = self.effects.borrow().frozen();
            let scaled_dt = dt * self.effects.borrow().time_scale();
            if !frozen {
                self.run_update(scaled_dt);
            }
            self.run_draw(dt, fps);
        }

        self.shader
            .borrow_mut()
            .apply_pending(&mut self.rl, &self.thread, self.vfs.as_ref());

        #[cfg(not(target_os = "emscripten"))]
        if self.recorder.is_recording() {
            self.recorder.capture(&self.rt);
        }

        self.blit_and_overlay(screen_w, screen_h);
        true
    }

    /// Renders the pause overlay onto the RT in place of `_draw`. Split
    /// out so the borrow-splitting destructure stays local.
    fn draw_paused(&mut self) {
        let family = self.input_bridge.state.get().gamepad_family();
        let res = self.config.resolution;
        let Self {
            rl,
            thread,
            rt,
            pause,
            font,
            settings,
            keymap,
            ..
        } = self;
        let mut d_rt = rl.begin_texture_mode(thread, rt);
        pause.draw(&mut d_rt, font, settings, keymap, family, res);
    }

    fn maybe_reload_assets(&mut self) {
        // Script reload: re-exec on mtime change. State is preserved (no
        // _init); F5 is the explicit reset. Errors are logged and the
        // previous callbacks keep running so a half-saved file can't kill
        // the session.
        let new_mtime = self.vfs.freshest_lua_mtime();
        if new_mtime.is_some() && new_mtime != self.last_modified {
            // Drop cached require results so dependencies re-execute when
            // main.lua re-runs. Built-in libs are untouched.
            if let Err(e) = clear_user_modules(&self.lua, self.vfs.as_ref()) {
                crate::msg::err!("clear_user_modules: {e}");
            }
            match load_script(&self.lua, self.vfs.as_ref()) {
                Ok(()) => {
                    crate::msg::info!("reloaded {} & required dependents", self.vfs.script_name());
                    self.update = self.lua.globals().get("_update").ok();
                    self.draw = self.lua.globals().get("_draw").ok();
                    self.last_error = None;
                }
                Err(e) => {
                    let msg = format!("reload: {}", e);
                    crate::msg::err!("{}", msg);
                    self.last_error = Some(msg);
                }
            }
            // Cache the pre-reload value rather than re-stat'ing after
            // the reload: any save that landed during load_script will
            // bump the next freshest_lua_mtime past this captured value
            // and re-trigger reload. The old re-stat approach silently
            // swallowed mid-reload saves.
            self.last_modified = new_mtime;
        }

        if self
            .sprites
            .reload_if_changed(&mut self.rl, &self.thread, self.vfs.as_ref())
        {
            crate::msg::info!("reloaded sprites.png");
        }

        if let Some(a) = self.audio
            && self.sfx.reload_if_changed(a, self.vfs.as_ref())
        {
            crate::msg::info!("reloaded sfx ({} sound(s))", self.sfx.len());
        }

        if let Some(a) = self.audio
            && self
                .music
                .borrow_mut()
                .reload_if_changed(a, self.vfs.as_ref())
        {
            crate::msg::info!("reloaded music ({} track(s))", self.music.borrow().len());
        }

        if self
            .shader
            .borrow_mut()
            .reload_if_changed(&mut self.rl, &self.thread, self.vfs.as_ref())
        {
            crate::msg::info!("reloaded shader");
        }
    }

    /// Wipes engine-level juice state and re-runs `_init()`. Used by
    /// the F5 / Ctrl+R / Cmd+R hotkey and the pause-menu Reset Game
    /// item. Effects are cleared *before* `_init()` so a fresh game
    /// can call `effect.flash(...)` etc. during init and have those
    /// stick. That way a long `effect.hitstop(100)` from the previous
    /// run doesn't freeze the new one.
    fn reset_game(&mut self) {
        self.effects.borrow_mut().reset();
        let Ok(init) = self.lua.globals().get::<LuaFunction>("_init") else {
            return;
        };
        match init.call::<()>(()) {
            Ok(()) => {
                crate::msg::info!("reset");
                self.last_error = None;
            }
            Err(e) => {
                let msg = format!("_init: {}", e);
                crate::msg::err!("{}", msg);
                self.last_error = Some(msg);
            }
        }
    }

    /// Flips fullscreen state and persists. Native uses raylib's
    /// borderless toggle; web routes through the browser's Fullscreen
    /// API since raylib's desktop fullscreen calls no-op under
    /// emscripten. The browser side requires a user-gesture call
    /// stack — both call sites (Alt+Enter, pause-menu BTN1) qualify.
    fn toggle_fullscreen(&mut self) {
        #[cfg(target_os = "emscripten")]
        unsafe {
            usagi_fullscreen_toggle();
        }
        #[cfg(not(target_os = "emscripten"))]
        self.rl.toggle_borderless_windowed();
        self.settings.fullscreen = !self.settings.fullscreen;
        if let Err(e) = crate::settings::write(&self.game_id, &self.settings) {
            crate::msg::err!("settings write failed: {e}");
        }
    }

    fn handle_global_shortcuts(&mut self) {
        // Alt+Enter toggles borderless fullscreen and persists.
        if self.rl.is_key_pressed(KeyboardKey::KEY_ENTER)
            && (self.rl.is_key_down(KeyboardKey::KEY_LEFT_ALT)
                || self.rl.is_key_down(KeyboardKey::KEY_RIGHT_ALT))
        {
            self.toggle_fullscreen();
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
        if reset {
            self.reset_game();
        }

        // Shift + Esc quits in dev builds
        let shift = self.rl.is_key_down(KeyboardKey::KEY_LEFT_SHIFT)
            || self.rl.is_key_down(KeyboardKey::KEY_RIGHT_SHIFT);
        if self.dev && shift && self.rl.is_key_pressed(KeyboardKey::KEY_ESCAPE) {
            self.should_quit = true;
        }

        // Shift+M mutes both channels (music + sfx); a second press
        // restores both to their respective defaults. Shift gates the
        // hotkey so games can bind plain `M` to gameplay.
        if self.rl.is_key_pressed(KeyboardKey::KEY_M) && shift && self.audio.is_some() {
            let any_audible = self.settings.music_volume > 0.0 || self.settings.sfx_volume > 0.0;
            let (m, s) = if any_audible {
                (0.0, 0.0)
            } else {
                (
                    crate::settings::DEFAULT_MUSIC_VOLUME,
                    crate::settings::DEFAULT_SFX_VOLUME,
                )
            };
            self.settings.music_volume = m;
            self.settings.sfx_volume = s;
            self.music.borrow_mut().set_volume(m);
            self.sfx.set_volume(s);
            if let Err(e) = crate::settings::write(&self.game_id, &self.settings) {
                crate::msg::err!("settings write failed: {e}");
            }
            crate::msg::err!("music: {m:.2}, sfx: {s:.2}");
        }

        // F9 / Cmd+G / Ctrl+G toggles GIF recording.
        #[cfg(not(target_os = "emscripten"))]
        {
            let mod_just_pressed = self.rl.is_key_pressed(KeyboardKey::KEY_LEFT_SUPER)
                || self.rl.is_key_pressed(KeyboardKey::KEY_RIGHT_SUPER)
                || self.rl.is_key_pressed(KeyboardKey::KEY_LEFT_CONTROL)
                || self.rl.is_key_pressed(KeyboardKey::KEY_RIGHT_CONTROL);
            let mod_held = ctrl_held || super_held;

            // F9 / Cmd+G / Ctrl+G toggles GIF recording.
            let g_pressed = self.rl.is_key_pressed(KeyboardKey::KEY_G);
            let g_down = self.rl.is_key_down(KeyboardKey::KEY_G);
            let cmd_g = (g_pressed && mod_held) || (mod_just_pressed && g_down);
            let toggle_rec = self.rl.is_key_pressed(KeyboardKey::KEY_F9) || cmd_g;
            if toggle_rec
                && let Err(e) = self.recorder.toggle(
                    &self.captures_dir,
                    &self.capture_prefix,
                    self.config.resolution,
                )
            {
                crate::msg::err!("recorder toggle failed: {e}");
            }

            // F8 / Cmd+F / Ctrl+F saves a one-shot PNG screenshot.
            let f_pressed = self.rl.is_key_pressed(KeyboardKey::KEY_F);
            let f_down = self.rl.is_key_down(KeyboardKey::KEY_F);
            let cmd_f = (f_pressed && mod_held) || (mod_just_pressed && f_down);
            let take_shot = self.rl.is_key_pressed(KeyboardKey::KEY_F8) || cmd_f;
            if take_shot
                && let Err(e) = save_screenshot(
                    &self.rt,
                    &self.captures_dir,
                    &self.capture_prefix,
                    self.config.resolution,
                )
            {
                crate::msg::err!("screenshot failed: {e}");
            }
        }
    }

    /// Applies a pause-menu transition. The menu only mutates its own
    /// state; everything that touches settings, audio, the window, or
    /// disk lands here so the side effects sit alongside the matching
    /// hotkey handlers.
    fn apply_pause_action(&mut self, action: PauseAction) {
        match action {
            PauseAction::Resume => {}
            PauseAction::SetMusicVolume(v) => {
                let v = v.clamp(0.0, 1.0);
                self.settings.music_volume = v;
                self.music.borrow_mut().set_volume(v);
                if let Err(e) = crate::settings::write(&self.game_id, &self.settings) {
                    crate::msg::err!("settings write failed: {e}");
                }
            }
            PauseAction::SetSfxVolume(v) => {
                let v = v.clamp(0.0, 1.0);
                self.settings.sfx_volume = v;
                self.sfx.set_volume(v);
                if let Err(e) = crate::settings::write(&self.game_id, &self.settings) {
                    crate::msg::err!("settings write failed: {e}");
                }
            }
            PauseAction::ToggleFullscreen => {
                self.toggle_fullscreen();
            }
            PauseAction::ResetGame => {
                self.reset_game();
            }
            PauseAction::ClearSave => {
                #[cfg(not(target_os = "emscripten"))]
                match crate::save::clear_save(&self.game_id) {
                    Ok(()) => crate::msg::info!("save data cleared"),
                    Err(e) => crate::msg::err!("save clear failed: {e}"),
                }
                #[cfg(target_os = "emscripten")]
                crate::msg::err!("clear save data is not supported on web yet");
            }
            PauseAction::SetKeymap(km) => {
                self.keymap = km;
                if let Err(e) = crate::keymap::write(&self.game_id, &self.keymap) {
                    crate::msg::err!("keymap write failed: {e}");
                }
            }
            PauseAction::Quit => {
                self.should_quit = true;
            }
        }
    }

    fn run_update(&mut self, dt: f32) {
        let Self {
            lua,
            sfx,
            update,
            last_error,
            ..
        } = self;
        let Some(update_fn) = update.as_ref() else {
            return;
        };
        let sfx_ref: &SfxLibrary<'static> = sfx;
        record_err(
            last_error,
            "_update",
            lua.scope(|scope| {
                let sfx_tbl: LuaTable = lua.globals().get("sfx")?;
                let play = scope.create_function(|_, name: String| {
                    sfx_ref.play(&name);
                    Ok(())
                })?;
                sfx_tbl.set("play", wrap(lua, play, "sfx.play", &["string"])?)?;

                update_fn.call::<()>(dt)?;
                Ok(())
            }),
        );
    }

    fn run_draw(&mut self, dt: f32, fps: u32) {
        let flash_overlay = self.effects.borrow().flash_overlay();
        let res = self.config.resolution;
        let sprite_size = self.config.sprite_size;
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
            last_clear,
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
                        last_clear.set(c);
                        d_rt_cell.borrow_mut().clear_background(color(c));
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
                                color(c),
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
                                color(c),
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
                                color(c),
                            );
                            Ok(())
                        },
                    )?;
                    let circ = scope.create_function(|_, (x, y, r, c): (f32, f32, f32, i32)| {
                        d_rt_cell.borrow_mut().draw_circle_lines(
                            x.round() as i32,
                            y.round() as i32,
                            r,
                            color(c),
                        );
                        Ok(())
                    })?;
                    let circ_fill =
                        scope.create_function(|_, (x, y, r, c): (f32, f32, f32, i32)| {
                            d_rt_cell.borrow_mut().draw_circle(
                                x.round() as i32,
                                y.round() as i32,
                                r,
                                color(c),
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
                                color(c),
                            );
                            Ok(())
                        },
                    )?;
                    let pixel = scope.create_function(|_, (x, y, c): (f32, f32, i32)| {
                        d_rt_cell.borrow_mut().draw_pixel(
                            x.round() as i32,
                            y.round() as i32,
                            color(c),
                        );
                        Ok(())
                    })?;
                    // Resolves a 1-based sprite index into a (col, row,
                    // cell) tuple on the loaded sheet, or None for
                    // out-of-range / no-sheet. Shared between `spr`
                    // and `spr_ex` so the bookkeeping stays in one
                    // place. `cell` is the configured `sprite_size`
                    // captured from the session before the scope.
                    fn cell_for(tex: &Texture2D, idx: i32, cell: i32) -> Option<(i32, i32, i32)> {
                        if idx < 1 || cell < 1 {
                            return None;
                        }
                        let cols = tex.width / cell;
                        if cols <= 0 {
                            return None;
                        }
                        let idx0 = idx - 1;
                        let col = idx0 % cols;
                        let row = idx0 / cols;
                        if row * cell >= tex.height {
                            return None;
                        }
                        Some((col, row, cell))
                    }
                    let spr = scope.create_function(|_, (idx, x, y): (i32, f32, f32)| {
                        if let Some(tex) = sprites_ref
                            && let Some((col, row, cell)) = cell_for(tex, idx, sprite_size)
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
                                && let Some((col, row, cell)) = cell_for(tex, idx, sprite_size)
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
                    gfx_tbl.set("clear", wrap(lua, clear, "gfx.clear", &["number"])?)?;
                    gfx_tbl.set(
                        "text",
                        wrap(
                            lua,
                            text,
                            "gfx.text",
                            &["string", "number", "number", "number"],
                        )?,
                    )?;
                    gfx_tbl.set(
                        "rect",
                        wrap(
                            lua,
                            rect,
                            "gfx.rect",
                            &["number", "number", "number", "number", "number"],
                        )?,
                    )?;
                    gfx_tbl.set(
                        "rect_fill",
                        wrap(
                            lua,
                            rect_fill,
                            "gfx.rect_fill",
                            &["number", "number", "number", "number", "number"],
                        )?,
                    )?;
                    gfx_tbl.set(
                        "circ",
                        wrap(
                            lua,
                            circ,
                            "gfx.circ",
                            &["number", "number", "number", "number"],
                        )?,
                    )?;
                    gfx_tbl.set(
                        "circ_fill",
                        wrap(
                            lua,
                            circ_fill,
                            "gfx.circ_fill",
                            &["number", "number", "number", "number"],
                        )?,
                    )?;
                    gfx_tbl.set(
                        "line",
                        wrap(
                            lua,
                            line,
                            "gfx.line",
                            &["number", "number", "number", "number", "number"],
                        )?,
                    )?;
                    gfx_tbl.set(
                        "pixel",
                        wrap(lua, pixel, "gfx.pixel", &["number", "number", "number"])?,
                    )?;
                    gfx_tbl.set(
                        "spr",
                        wrap(lua, spr, "gfx.spr", &["number", "number", "number"])?,
                    )?;
                    gfx_tbl.set(
                        "spr_ex",
                        wrap(
                            lua,
                            spr_ex,
                            "gfx.spr_ex",
                            &["number", "number", "number", "boolean", "boolean"],
                        )?,
                    )?;
                    gfx_tbl.set(
                        "sspr",
                        wrap(
                            lua,
                            sspr,
                            "gfx.sspr",
                            &["number", "number", "number", "number", "number", "number"],
                        )?,
                    )?;
                    gfx_tbl.set(
                        "sspr_ex",
                        wrap(
                            lua,
                            sspr_ex,
                            "gfx.sspr_ex",
                            &[
                                "number", "number", "number", "number", "number", "number",
                                "number", "number", "boolean", "boolean",
                            ],
                        )?,
                    )?;

                    let sfx_tbl: LuaTable = lua.globals().get("sfx")?;
                    let play = scope.create_function(|_, name: String| {
                        sfx_ref.play(&name);
                        Ok(())
                    })?;
                    sfx_tbl.set("play", wrap(lua, play, "sfx.play", &["string"])?)?;

                    draw_fn.call::<()>(dt)?;
                    Ok(())
                }),
            );
        }
        if let Some((idx, alpha)) = flash_overlay {
            let mut c = color(idx);
            c.a = alpha;
            d_rt.draw_rectangle(0, 0, res.w as i32, res.h as i32, c);
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

    /// draw the renter target to the screen, on top of a true black bg
    fn blit_and_overlay(&mut self, screen_w: i32, screen_h: i32) {
        #[cfg(not(target_os = "emscripten"))]
        let recording = self.recorder.is_recording();
        // Shake offset is sampled here (post-update, post-draw) so the
        // RT itself stays unshaken; only the blit's dest rect moves.
        // That keeps overlays drawn outside this function (error,
        // REC) stable while the world dances. Suppressed under the
        // pause overlay so the world doesn't keep shaking under
        // "PAUSED".
        let shake = if self.pause.open {
            (0.0, 0.0)
        } else {
            self.effects.borrow_mut().shake_offset()
        };
        let mut d = self.rl.begin_drawing(&self.thread);
        d.clear_background(Color::BLACK);
        // Fill the unshaken game viewport with the most recent
        // `gfx.clear` color so the strips exposed at the shifted
        // RT's edges read as the game's bg, not letterbox black.
        // Letterbox bars stay black (window clear above). Skipped
        // when no shake is active, since the RT blit covers the
        // full viewport in that case.
        let res = self.config.resolution;
        if shake != (0.0, 0.0) {
            let (scale, top_left_x, top_left_y) =
                game_view_transform(screen_w, screen_h, res, self.config.pixel_perfect);
            d.draw_rectangle(
                top_left_x as i32,
                top_left_y as i32,
                (res.w * scale) as i32,
                (res.h * scale) as i32,
                color(self.last_clear.get()),
            );
        }
        // Wrap the RT-to-window blit in `begin_shader_mode` when a
        // shader is active so the post-process runs at window
        // resolution (smoother than game-res). The error overlay and
        // REC indicator draw outside this scope so they're not warped
        // by the effect.
        {
            let mut sm = self.shader.borrow_mut();
            if let Some(shader) = sm.active_shader_mut() {
                let mut s = d.begin_shader_mode(shader);
                draw_render_target(
                    &mut s,
                    &mut self.rt,
                    screen_w,
                    screen_h,
                    res,
                    self.config.pixel_perfect,
                    shake,
                );
            } else {
                draw_render_target(
                    &mut d,
                    &mut self.rt,
                    screen_w,
                    screen_h,
                    res,
                    self.config.pixel_perfect,
                    shake,
                );
            }
        }
        if let Some(ref err) = self.last_error {
            draw_error_overlay(&mut d, self.font, err, screen_w, screen_h);
        }
        // REC indicator drawn after the RT blit so the captured gif
        // Lives on the screen overlay only so the player sees it, the recording
        // doesn't.
        #[cfg(not(target_os = "emscripten"))]
        if recording {
            const PADDING: i32 = 12;
            let dot_r = 8.0;
            let dot_x = (screen_w - PADDING - dot_r as i32 * 2) as f32 + dot_r;
            let dot_y = (PADDING + dot_r as i32) as f32;
            let pulse = 0.6 + 0.4 * (self.elapsed * 4.0).sin() as f32;
            let alpha = (pulse * 255.0) as u8;
            let red = Color::new(220, 60, 60, alpha);
            d.draw_circle(dot_x as i32, dot_y as i32, dot_r, red);
            d.draw_text_ex(
                self.font,
                "REC",
                Vector2::new(dot_x - dot_r - 40.0, dot_y - 11.0),
                (crate::font::MONOGRAM_SIZE * 2) as f32,
                0.0,
                red,
            );
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
    /// Defined by `web/usagi_fullscreen.js` and linked via
    /// `--js-library`. Routes the toggle through the browser's
    /// Fullscreen API since raylib's desktop fullscreen calls don't
    /// work on emscripten.
    fn usagi_fullscreen_toggle();
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
              return { name = "Hello, Usagi!" }
            end
            "#,
        )
        .exec()
        .unwrap();
        let mut err = None;
        let config = read_config(&lua, &mut err);
        assert_eq!(config.name.as_deref(), Some("Hello, Usagi!"));
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
    fn missing_config_pixel_perfect_defaults_to_false() {
        let lua = Lua::new();
        setup_api(&lua, false).unwrap();
        let mut err = None;
        let config = read_config(&lua, &mut err);
        assert!(
            !config.pixel_perfect,
            "default should be pixel-perfect off (fill the window)"
        );
    }

    /// Regression: `_config()` returning a table without `pixel_perfect`
    /// must keep the default value. mlua coerces missing/nil to
    /// `Ok(false)` for `bool` fields, so the read path has to use
    /// `Option<bool>` to preserve "field absent → keep default".
    #[test]
    fn config_without_pixel_perfect_field_keeps_default() {
        let lua = Lua::new();
        setup_api(&lua, false).unwrap();
        lua.load(r#"function _config() return { name = "Game" } end"#)
            .exec()
            .unwrap();
        let mut err = None;
        let config = read_config(&lua, &mut err);
        assert_eq!(
            config.pixel_perfect,
            Config::default().pixel_perfect,
            "missing pixel_perfect field must not override the default"
        );
        assert_eq!(config.name.as_deref(), Some("Game"));
        assert!(err.is_none());
    }

    #[test]
    fn missing_config_returns_defaults() {
        let lua = Lua::new();
        setup_api(&lua, false).unwrap();
        let mut err = None;
        let config = read_config(&lua, &mut err);
        assert!(config.name.is_none());
        assert!(err.is_none());
    }

    #[test]
    fn config_with_no_name_field_leaves_name_unset() {
        let lua = Lua::new();
        setup_api(&lua, false).unwrap();
        lua.load("function _config() return {} end").exec().unwrap();
        let mut err = None;
        let config = read_config(&lua, &mut err);
        assert!(config.name.is_none());
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
