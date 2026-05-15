//! Static Lua API: installs the `gfx`, `input`, `sfx`, and `usagi` tables
//! with constants. The per-frame closures (gfx.clear, input.pressed, etc.)
//! live in the game loop because they need to borrow frame-local state.

use crate::config::{DEFAULT_SPRITE_SIZE, Resolution};
use crate::input::{
    ACTION_BTN1, ACTION_BTN2, ACTION_BTN3, ACTION_DOWN, ACTION_LEFT, ACTION_RIGHT, ACTION_UP,
    KEY_TABLE, MOUSE_LEFT, MOUSE_MIDDLE, MOUSE_RIGHT,
};
use crate::shader::{ShaderManager, ShaderValue};
use mlua::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Lua-side argument validator. Wraps a raw Rust callback so bad input is
/// rejected with a clean `error(...)` from Lua-land instead of propagating
/// out of the Rust closure as `Err`. The latter triggers a longjmp through
/// Rust frames, which crashes on Windows MSVC. Keeping the failure path inside
/// Lua means the longjmp only ever traverses Lua/C frames back to the nearest
/// pcall — fine on every platform.
///
/// The helper is exposed as the global `_usagi_wrap`; registration sites
/// reach it via `wrap(lua, raw, name, types)` below.
const WRAP_HELPER_LUA: &str = r##"
return function(raw, name, ...)
  local n = select("#", ...)
  local types = {...}
  return function(...)
    for i = 1, n do
      local expected = types[i]
      if expected ~= "any" then
        local got = type(select(i, ...))
        if got ~= expected then
          error(string.format(
            "bad argument #%d to '%s' (%s expected, got %s)",
            i, name, expected, got
          ), 2)
        end
      end
    end
    return raw(...)
  end
end
"##;

/// Installs `_usagi_wrap` (see [`WRAP_HELPER_LUA`]). Call once before any API
/// registration so [`wrap`] can find it.
pub fn install_wrap_helper(lua: &Lua) -> LuaResult<()> {
    let wrap_fn: LuaFunction = lua
        .load(WRAP_HELPER_LUA)
        .set_name("=usagi/wrap.lua")
        .eval()?;
    lua.globals().set("_usagi_wrap", wrap_fn)?;
    Ok(())
}

/// Wraps `raw` with Lua-side argument validation. `types` lists the expected
/// `type()` of each positional arg (`"number"`, `"string"`, `"boolean"`,
/// `"function"`, `"table"`, `"nil"`, or `"any"` to skip the check). The
/// returned function lives in Lua, so when validation fails it raises a
/// Lua error that unwinds through Lua/C frames only — see
/// [`WRAP_HELPER_LUA`] for the rationale.
pub fn wrap(lua: &Lua, raw: LuaFunction, name: &str, types: &[&str]) -> LuaResult<LuaFunction> {
    let wrap_fn: LuaFunction = lua.globals().get("_usagi_wrap")?;
    let mut args: Vec<LuaValue> = Vec::with_capacity(types.len() + 2);
    args.push(LuaValue::Function(raw));
    args.push(LuaValue::String(lua.create_string(name)?));
    for t in types {
        args.push(LuaValue::String(lua.create_string(*t)?));
    }
    wrap_fn.call(LuaMultiValue::from_vec(args))
}

/// Installs the Lua-facing globals: `gfx`, `input`, `sfx`, `usagi`. Each is a
/// table with any constants it owns. Per-frame function members (e.g.
/// gfx.clear, sfx.play) are registered inside `lua.scope` blocks in the main
/// loop so their closures can borrow the current frame's draw handle, audio
/// device, etc.
/// Build target the running binary was compiled for, exposed as
/// `usagi.PLATFORM`. Returned values mirror the export targets in
/// `usagi export`: web (emscripten), macos, linux, windows. Anything
/// else (BSDs, exotic Unixes) reports as "unknown" rather than
/// silently masquerading as Linux.
pub fn current_platform() -> &'static str {
    if cfg!(target_os = "emscripten") {
        "web"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "unknown"
    }
}

pub fn setup_api(lua: &Lua, dev: bool) -> LuaResult<()> {
    install_wrap_helper(lua)?;

    let gfx = lua.create_table()?;
    // Color slots are **1-based** to match `gfx.spr` and Lua array
    // conventions: slot 1 is the first color, slot 16 is the last.
    // Slot `0` is `COLOR_TRUE_WHITE`: pure (255,255,255) regardless of
    // the active palette. It's the identity tint for
    // `gfx.spr_ex` / `gfx.sspr_ex` because Pico-8's `COLOR_WHITE`
    // (255,241,232) shifts sprite colors slightly.
    gfx.set("COLOR_TRUE_WHITE", 0)?;
    gfx.set("COLOR_BLACK", 1)?;
    gfx.set("COLOR_DARK_BLUE", 2)?;
    gfx.set("COLOR_DARK_PURPLE", 3)?;
    gfx.set("COLOR_DARK_GREEN", 4)?;
    gfx.set("COLOR_BROWN", 5)?;
    gfx.set("COLOR_DARK_GRAY", 6)?;
    gfx.set("COLOR_LIGHT_GRAY", 7)?;
    gfx.set("COLOR_WHITE", 8)?;
    gfx.set("COLOR_RED", 9)?;
    gfx.set("COLOR_ORANGE", 10)?;
    gfx.set("COLOR_YELLOW", 11)?;
    gfx.set("COLOR_GREEN", 12)?;
    gfx.set("COLOR_BLUE", 13)?;
    gfx.set("COLOR_INDIGO", 14)?;
    gfx.set("COLOR_PINK", 15)?;
    gfx.set("COLOR_PEACH", 16)?;
    lua.globals().set("gfx", gfx)?;

    let input = lua.create_table()?;
    input.set("LEFT", ACTION_LEFT)?;
    input.set("RIGHT", ACTION_RIGHT)?;
    input.set("UP", ACTION_UP)?;
    input.set("DOWN", ACTION_DOWN)?;
    input.set("BTN1", ACTION_BTN1)?;
    input.set("BTN2", ACTION_BTN2)?;
    input.set("BTN3", ACTION_BTN3)?;
    input.set("MOUSE_LEFT", MOUSE_LEFT)?;
    input.set("MOUSE_RIGHT", MOUSE_RIGHT)?;
    input.set("MOUSE_MIDDLE", MOUSE_MIDDLE)?;
    // Direct keyboard constants (escape hatch — bypasses keymap and
    // gamepad). See `KEY_TABLE` in `crate::input` for the full list and
    // the rationale for only exposing common keys.
    for (name, key) in KEY_TABLE {
        input.set(*name, *key as i32 as u32)?;
    }
    input.set(
        "SOURCE_KEYBOARD",
        crate::input::InputSource::Keyboard.as_str(),
    )?;
    input.set(
        "SOURCE_GAMEPAD",
        crate::input::InputSource::Gamepad.as_str(),
    )?;
    lua.globals().set("input", input)?;

    let sfx = lua.create_table()?;
    lua.globals().set("sfx", sfx)?;

    let music = lua.create_table()?;
    lua.globals().set("music", music)?;

    // `gfx` / `input` are top-level globals (see above). The `usagi` table is
    // reserved for engine-level info: runtime constants, current frame stats,
    // etc. Not a namespace for the per-domain APIs.
    let usagi = lua.create_table()?;
    // GAME_W / GAME_H are seeded with the engine defaults at API
    // setup so tests / tools that don't drive a session see sane
    // values. The session re-writes them with the resolved
    // `_config().game_width / game_height` once read, before
    // `_init` runs.
    usagi.set("GAME_W", Resolution::DEFAULT.w)?;
    usagi.set("GAME_H", Resolution::DEFAULT.h)?;
    usagi.set("SPRITE_SIZE", DEFAULT_SPRITE_SIZE)?;
    // Build target the binary was compiled for: "web", "macos",
    // "linux", "windows", or "unknown". Lets games gate code paths
    // by platform without parsing UA strings or shelling out (e.g.
    // skip mouse-only UI on web, hide CLI hints on desktop).
    usagi.set("PLATFORM", current_platform())?;
    // True when running under `usagi dev`. False for `usagi run` and
    // fused/compiled binaries. Lets games gate debug overlays, dev menus,
    // verbose logging, etc.
    usagi.set("IS_DEV", dev)?;
    // Wall-clock seconds since the session started. The session updates
    // this once per frame before _update; tests and tools that don't
    // drive a frame loop see the seed value below. Doesn't reset on F5.
    usagi.set("elapsed", 0.0_f64)?;
    // `usagi.measure_text` is registered later, once the bundled font
    // is loaded, so the closure can capture it. Stubbed here so tests
    // and tools that don't drive a session can still reference the
    // field without erroring.
    usagi.set(
        "measure_text",
        lua.create_function(|_, _s: String| Ok((0i32, 0i32)))?,
    )?;
    // `usagi.dump` lives in runtime/usagi.lua so the pretty-printer is
    // pure Lua and easy to fork or override at the script level. The
    // file returns the dump function so we can attach it without
    // round-tripping through a global.
    let dump_fn: LuaFunction = lua
        .load(include_str!("../runtime/usagi.lua"))
        .set_name("usagi/usagi.lua")
        .eval()?;
    usagi.set("dump", dump_fn)?;
    lua.globals().set("usagi", usagi)?;

    // Pure-Lua stdlib (`util.clamp`, `util.rect_overlap`, etc.). Loaded
    // here so it's available to user `_init` / `_update` / `_draw` and
    // also to tests and tools that only call `setup_api`. Source lives
    // in `runtime/util.lua` for forkability — anyone modifying Usagi
    // can edit it without touching Rust.
    let util_src = include_str!("../runtime/util.lua");
    let util_table: LuaTable = lua.load(util_src).set_name("usagi/util.lua").eval()?;
    lua.globals().set("util", util_table)?;

    Ok(())
}

/// Installs the `gfx.shader_set` / `gfx.shader_uniform` Lua bindings
/// against a shared `ShaderManager`. Calls only enqueue requests; the
/// session drains them once per frame where `&mut RaylibHandle` is in
/// scope. Registered once at session startup so the bindings work
/// from `_init`, `_update`, and `_draw`.
pub fn register_shader_api(lua: &Lua, mgr: &Rc<RefCell<ShaderManager>>) -> LuaResult<()> {
    let gfx: LuaTable = lua.globals().get("gfx")?;

    let m = Rc::clone(mgr);
    let shader_set = lua.create_function(move |_, name: Option<String>| {
        m.borrow_mut().request_set(name);
        Ok(())
    })?;
    // `name` is optional (nil clears the active shader), so the wrapper
    // accepts "any" and lets mlua's Option<String> conversion sort it.
    gfx.set(
        "shader_set",
        wrap(lua, shader_set, "gfx.shader_set", &["any"])?,
    )?;

    let m = Rc::clone(mgr);
    let shader_uniform = lua.create_function(move |_, (name, value): (String, LuaValue)| {
        let v = parse_uniform(&value).map_err(mlua::Error::external)?;
        m.borrow_mut().queue_uniform(name, v);
        Ok(())
    })?;
    gfx.set(
        "shader_uniform",
        wrap(
            lua,
            shader_uniform,
            "gfx.shader_uniform",
            &["string", "any"],
        )?,
    )?;

    Ok(())
}

fn parse_uniform(value: &LuaValue) -> Result<ShaderValue, String> {
    if let Some(n) = value.as_f64() {
        return Ok(ShaderValue::Float(n as f32));
    }
    if let Some(n) = value.as_integer() {
        return Ok(ShaderValue::Float(n as f32));
    }
    if let LuaValue::Table(t) = value {
        let len = t.raw_len();
        return match len {
            2 => Ok(ShaderValue::Vec2([read_idx(t, 1)?, read_idx(t, 2)?])),
            3 => Ok(ShaderValue::Vec3([
                read_idx(t, 1)?,
                read_idx(t, 2)?,
                read_idx(t, 3)?,
            ])),
            4 => Ok(ShaderValue::Vec4([
                read_idx(t, 1)?,
                read_idx(t, 2)?,
                read_idx(t, 3)?,
                read_idx(t, 4)?,
            ])),
            n => Err(format!(
                "shader_uniform: table must have 2, 3, or 4 numbers, got {n}"
            )),
        };
    }
    Err("shader_uniform: value must be a number or 2/3/4-length table".to_string())
}

fn read_idx(t: &LuaTable, idx: usize) -> Result<f32, String> {
    let v: f64 = t
        .raw_get(idx)
        .map_err(|e| format!("shader_uniform: reading index {idx}: {e}"))?;
    Ok(v as f32)
}

/// Records a Lua error: stores the message so it can be displayed on-screen,
/// and prints to stderr only when the message changed from what `state` was
/// already holding. Wraps every call into user Lua so a typo / nil-call /
/// runtime error doesn't tear down the process. Per-frame callbacks
/// (`_update`, `_draw`) hit this every frame while a bug stands; the
/// dedupe keeps the terminal usable until the user fixes the script.
pub fn record_err(state: &mut Option<String>, label: &str, result: LuaResult<()>) {
    if let Err(e) = result {
        let msg = format!("{}: {}", label, e);
        if state.as_deref() != Some(msg.as_str()) {
            crate::msg::err!("{}", msg);
            *state = Some(msg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::is_valid_action;
    use crate::palette::color;

    #[test]
    fn setup_installs_expected_globals() {
        let lua = Lua::new();
        setup_api(&lua, false).unwrap();

        let gfx: LuaTable = lua.globals().get("gfx").unwrap();
        let input: LuaTable = lua.globals().get("input").unwrap();
        let sfx: LuaTable = lua.globals().get("sfx").unwrap();
        let music: LuaTable = lua.globals().get("music").unwrap();
        let usagi: LuaTable = lua.globals().get("usagi").unwrap();

        assert_eq!(gfx.get::<i32>("COLOR_TRUE_WHITE").unwrap(), 0);
        assert_eq!(gfx.get::<i32>("COLOR_BLACK").unwrap(), 1);
        assert_eq!(gfx.get::<i32>("COLOR_WHITE").unwrap(), 8);
        assert_eq!(gfx.get::<i32>("COLOR_RED").unwrap(), 9);
        assert_eq!(gfx.get::<i32>("COLOR_PEACH").unwrap(), 16);

        // Input constants just need to be present; values are action IDs.
        assert!(input.get::<u32>("LEFT").is_ok());
        assert!(input.get::<u32>("BTN1").is_ok());
        assert!(input.get::<u32>("BTN2").is_ok());
        assert!(input.get::<u32>("BTN3").is_ok());
        assert!(input.get::<u32>("MOUSE_LEFT").is_ok());
        assert!(input.get::<u32>("MOUSE_RIGHT").is_ok());
        assert!(input.get::<u32>("MOUSE_MIDDLE").is_ok());

        // sfx and music are registered but empty of fields at
        // static-setup time — their per-frame closures live in the
        // session loop.
        assert!(sfx.get::<LuaValue>("play").unwrap().is_nil());
        assert!(music.get::<LuaValue>("play").unwrap().is_nil());
        assert!(music.get::<LuaValue>("loop").unwrap().is_nil());
        assert!(music.get::<LuaValue>("stop").unwrap().is_nil());

        assert_eq!(usagi.get::<f32>("GAME_W").unwrap(), Resolution::DEFAULT.w);
        assert_eq!(usagi.get::<f32>("GAME_H").unwrap(), Resolution::DEFAULT.h);
        assert_eq!(
            usagi.get::<i32>("SPRITE_SIZE").unwrap(),
            DEFAULT_SPRITE_SIZE
        );
        assert_eq!(usagi.get::<f64>("elapsed").unwrap(), 0.0);
    }

    #[test]
    fn wrap_rejects_bad_arg_with_lua_error_not_panic() {
        // GH Issue #103: callbacks must reject bad input via Lua-side
        // `error(...)` so the longjmp stays inside Lua/C frames. Returning
        // `Err` from a typed Rust callback would re-raise via `lua_error`
        // through Rust frames, which trips Windows MSVC's GS stack-cookie
        // check and aborts the process.
        //
        // This test exercises the wrap machinery on every platform; on
        // Mac/Linux longjmp-through-Rust happens to work, so this would
        // have passed pre-fix too — it locks the wrap contract in place.
        let lua = Lua::new();
        install_wrap_helper(&lua).unwrap();

        let raw = lua
            .create_function(|_, c: i32| Ok(c * 2))
            .expect("raw callback");
        let wrapped = wrap(&lua, raw, "test.double", &["number"]).expect("wrap");
        lua.globals().set("doublez", wrapped).unwrap();

        // Happy path: a number passes the validator and reaches the raw fn.
        let v: i32 = lua.load("return doublez(7)").eval().unwrap();
        assert_eq!(v, 14);

        // Sad path: nil is caught by the Lua wrapper before mlua's i32
        // conversion even runs. The error must propagate cleanly as Err,
        // not crash the process.
        let err = lua
            .load("doublez(nil)")
            .exec()
            .expect_err("nil arg must be rejected");
        let s = err.to_string();
        assert!(
            s.contains("test.double") && s.contains("number expected"),
            "expected friendly arg error, got: {s}"
        );

        // Sad path 2: wrong type (string) is also caught.
        let err = lua
            .load("doublez('hi')")
            .exec()
            .expect_err("string arg must be rejected");
        assert!(err.to_string().contains("test.double"));
    }

    #[test]
    fn wrap_with_any_skips_validation() {
        // The "any" placeholder lets a callback opt out of Lua-side type
        // checking for an arg whose conversion is best handled by mlua
        // (e.g. Option<String>, LuaValue serialization).
        let lua = Lua::new();
        install_wrap_helper(&lua).unwrap();

        let raw = lua
            .create_function(|_, _v: LuaValue| Ok(true))
            .expect("raw callback");
        let wrapped = wrap(&lua, raw, "test.any", &["any"]).expect("wrap");
        lua.globals().set("any_ok", wrapped).unwrap();

        let result: bool = lua.load("return any_ok(nil)").eval().unwrap();
        assert!(result);
        let result: bool = lua.load("return any_ok({1, 2, 3})").eval().unwrap();
        assert!(result);
    }

    #[test]
    fn is_dev_reflects_setup_arg() {
        let lua = Lua::new();
        setup_api(&lua, true).unwrap();
        let usagi: LuaTable = lua.globals().get("usagi").unwrap();
        assert!(usagi.get::<bool>("IS_DEV").unwrap());

        let lua = Lua::new();
        setup_api(&lua, false).unwrap();
        let usagi: LuaTable = lua.globals().get("usagi").unwrap();
        assert!(!usagi.get::<bool>("IS_DEV").unwrap());
    }

    #[test]
    fn record_err_stores_and_prefixes_label() {
        let lua = Lua::new();
        let result: LuaResult<()> = lua.load("error('boom')").exec();
        let mut state = None;
        record_err(&mut state, "_update", result);
        let stored = state.expect("should have recorded");
        assert!(stored.starts_with("_update: "), "got: {stored}");
        assert!(stored.contains("boom"), "got: {stored}");
    }

    #[test]
    fn record_err_leaves_state_alone_on_ok() {
        let mut state = Some("previous".to_string());
        record_err(&mut state, "_update", Ok(()));
        assert_eq!(state.as_deref(), Some("previous"));
    }

    /// Repeating the same error (the typical per-frame `_update` /
    /// `_draw` failure) overwrites `state` with the same text but
    /// must not change anything observable. The visible promise this
    /// test stands behind is the `record_err` body's `as_deref()
    /// != Some(...)` guard around the stderr write.
    #[test]
    fn record_err_is_idempotent_for_repeated_messages() {
        let mut state = None;
        record_err(&mut state, "_update", Err(mlua::Error::external("same")));
        let first = state.clone().expect("first should record");
        record_err(&mut state, "_update", Err(mlua::Error::external("same")));
        assert_eq!(state.as_deref(), Some(first.as_str()));
    }

    /// A new error message after an old one still records (and
    /// would be logged): the dedupe is per-content, not "log only
    /// once ever".
    #[test]
    fn record_err_records_new_message_after_old() {
        let mut state = None;
        record_err(&mut state, "_update", Err(mlua::Error::external("alpha")));
        record_err(&mut state, "_update", Err(mlua::Error::external("beta")));
        let stored = state.expect("second error should record");
        assert!(stored.contains("beta"), "got: {stored}");
        assert!(
            !stored.contains("alpha"),
            "old text should be replaced: {stored}"
        );
    }

    /// Every `gfx.COLOR_*` constant must map to a real palette entry.
    /// Guards against adding a new color constant without teaching
    /// `palette::color`, which would silently render as magenta.
    #[test]
    fn every_gfx_color_maps_to_a_distinct_palette_entry() {
        let lua = Lua::new();
        setup_api(&lua, false).unwrap();
        let gfx: LuaTable = lua.globals().get("gfx").unwrap();

        let magenta = color(i32::MAX); // known sentinel color
        let mut indices: Vec<i32> = Vec::new();

        for pair in gfx.pairs::<String, i32>() {
            let (name, idx) = pair.unwrap();
            if !name.starts_with("COLOR_") {
                continue;
            }
            let c = color(idx);
            assert!(
                (c.r, c.g, c.b) != (magenta.r, magenta.g, magenta.b),
                "{name}={idx} falls through to the magenta sentinel in palette::color",
            );
            indices.push(idx);
        }

        assert!(
            indices.len() >= 16,
            "expected at least 16 COLOR_* constants, got {}",
            indices.len()
        );

        let mut sorted = indices.clone();
        sorted.sort();
        let unique = sorted.len();
        sorted.dedup();
        assert_eq!(
            unique,
            sorted.len(),
            "duplicate COLOR_* indices in setup_api"
        );
    }

    /// Every `input.*` constant must map to a valid action in
    /// `crate::input`. Guards against adding a new input action to
    /// `setup_api` without extending `BINDINGS`, which would make
    /// `input.held(input.X)` always return false. `MOUSE_*`, `SOURCE_*`,
    /// and `KEY_*` constants are skipped here because they're not
    /// action IDs (KEY_* are raw raylib keycodes, MOUSE_* are mouse
    /// button enum values, SOURCE_* are strings).
    #[test]
    fn every_input_constant_is_a_valid_action() {
        let lua = Lua::new();
        setup_api(&lua, false).unwrap();
        let input: LuaTable = lua.globals().get("input").unwrap();
        let mut checked = 0;
        for pair in input.pairs::<String, mlua::Value>() {
            let (name, value) = pair.unwrap();
            if name.starts_with("MOUSE_") || name.starts_with("SOURCE_") || name.starts_with("KEY_")
            {
                continue;
            }
            let code: u32 = mlua::FromLua::from_lua(value, &lua).unwrap_or_else(|e| {
                panic!("input.{name} should be a u32 action id but was not: {e}")
            });
            assert!(
                is_valid_action(code),
                "input.{name} = {code} is not a valid action",
            );
            checked += 1;
        }
        assert!(
            checked >= 7,
            "expected at least 7 input.* actions, got {checked}"
        );
    }

    /// A minimal Lua script exercises the registered API surface without
    /// erroring. Covers the per-frame scope closures by registering stub
    /// implementations of the runtime functions.
    #[test]
    fn script_can_call_full_api_under_scope() {
        let lua = Lua::new();
        setup_api(&lua, false).unwrap();

        lua.scope(|scope| {
            let gfx: LuaTable = lua.globals().get("gfx")?;
            gfx.set("clear", scope.create_function(|_, _c: i32| Ok(()))?)?;
            gfx.set(
                "rect",
                scope.create_function(|_, _a: (f32, f32, f32, f32, i32)| Ok(()))?,
            )?;
            gfx.set(
                "rect_fill",
                scope.create_function(|_, _a: (f32, f32, f32, f32, i32)| Ok(()))?,
            )?;
            gfx.set(
                "circ",
                scope.create_function(|_, _a: (f32, f32, f32, i32)| Ok(()))?,
            )?;
            gfx.set(
                "circ_fill",
                scope.create_function(|_, _a: (f32, f32, f32, i32)| Ok(()))?,
            )?;
            gfx.set(
                "line",
                scope.create_function(|_, _a: (f32, f32, f32, f32, i32)| Ok(()))?,
            )?;
            gfx.set(
                "rect_ex",
                scope.create_function(|_, _a: (f32, f32, f32, f32, f32, i32)| Ok(()))?,
            )?;
            gfx.set(
                "circ_ex",
                scope.create_function(|_, _a: (f32, f32, f32, f32, i32)| Ok(()))?,
            )?;
            gfx.set(
                "line_ex",
                scope.create_function(|_, _a: (f32, f32, f32, f32, f32, i32)| Ok(()))?,
            )?;
            gfx.set(
                "text",
                scope.create_function(|_, _a: (String, f32, f32, i32)| Ok(()))?,
            )?;
            gfx.set(
                "text_ex",
                scope.create_function(|_, _a: (String, f32, f32, f32, f32, i32)| Ok(()))?,
            )?;
            gfx.set(
                "spr",
                scope.create_function(|_, _a: (i32, f32, f32)| Ok(()))?,
            )?;
            gfx.set(
                "spr_ex",
                scope
                    .create_function(|_, _a: (i32, f32, f32, bool, bool, f32, i32, f32)| Ok(()))?,
            )?;
            gfx.set(
                "sspr",
                scope.create_function(|_, _a: (f32, f32, f32, f32, f32, f32)| Ok(()))?,
            )?;
            type SsprExArgs = (
                f32,
                f32,
                f32,
                f32,
                f32,
                f32,
                f32,
                f32,
                bool,
                bool,
                f32,
                i32,
                f32,
            );
            gfx.set(
                "sspr_ex",
                scope.create_function(|_, _a: SsprExArgs| Ok(()))?,
            )?;
            gfx.set(
                "pixel",
                scope.create_function(|_, _a: (f32, f32, i32)| Ok(()))?,
            )?;

            let input: LuaTable = lua.globals().get("input")?;
            input.set("pressed", scope.create_function(|_, _k: u32| Ok(false))?)?;
            input.set("held", scope.create_function(|_, _k: u32| Ok(false))?)?;
            input.set("released", scope.create_function(|_, _k: u32| Ok(false))?)?;
            input.set("mouse", scope.create_function(|_, ()| Ok((0i32, 0i32)))?)?;
            input.set("mouse_held", scope.create_function(|_, _b: u32| Ok(false))?)?;
            input.set(
                "mouse_pressed",
                scope.create_function(|_, _b: u32| Ok(false))?,
            )?;
            input.set(
                "mouse_released",
                scope.create_function(|_, _b: u32| Ok(false))?,
            )?;
            input.set("mouse_scroll", scope.create_function(|_, ()| Ok(0.0f32))?)?;
            input.set("key_held", scope.create_function(|_, _k: u32| Ok(false))?)?;
            input.set(
                "key_pressed",
                scope.create_function(|_, _k: u32| Ok(false))?,
            )?;
            input.set(
                "key_released",
                scope.create_function(|_, _k: u32| Ok(false))?,
            )?;
            input.set(
                "set_mouse_visible",
                scope.create_function(|_, _v: bool| Ok(()))?,
            )?;
            input.set("mouse_visible", scope.create_function(|_, ()| Ok(true))?)?;
            input.set(
                "mapping_for",
                scope.create_function(|_, _k: u32| Ok(None::<String>))?,
            )?;
            input.set(
                "last_source",
                scope.create_function(|_, ()| Ok("keyboard"))?,
            )?;

            let sfx: LuaTable = lua.globals().get("sfx")?;
            sfx.set("play", scope.create_function(|_, _n: String| Ok(()))?)?;
            sfx.set(
                "play_ex",
                scope.create_function(|_, _a: (String, f32, f32, f32)| Ok(()))?,
            )?;

            let music: LuaTable = lua.globals().get("music")?;
            music.set("play", scope.create_function(|_, _n: String| Ok(()))?)?;
            music.set("loop", scope.create_function(|_, _n: String| Ok(()))?)?;
            music.set("stop", scope.create_function(|_, ()| Ok(()))?)?;
            music.set(
                "play_ex",
                scope.create_function(|_, _a: (String, f32, f32, f32, bool)| Ok(()))?,
            )?;
            music.set(
                "mutate",
                scope.create_function(|_, _a: (f32, f32, f32)| Ok(()))?,
            )?;

            lua.load(
                r#"
                gfx.clear(gfx.COLOR_BLACK)
                gfx.rect(10, 20, 30, 40, gfx.COLOR_RED)
                gfx.rect_fill(10, 20, 30, 40, gfx.COLOR_BLUE)
                gfx.circ(50, 50, 8, gfx.COLOR_GREEN)
                gfx.circ_fill(60, 60, 4, gfx.COLOR_YELLOW)
                gfx.line(0, 0, 100, 100, gfx.COLOR_WHITE)
                gfx.rect_ex(10, 20, 30, 40, 2, gfx.COLOR_RED)
                gfx.circ_ex(50, 50, 8, 2, gfx.COLOR_GREEN)
                gfx.line_ex(0, 0, 100, 100, 3, gfx.COLOR_WHITE)
                gfx.text("hi", 0, 0, gfx.COLOR_WHITE)
                gfx.text_ex("hi", 0, 0, 2, math.pi / 4, gfx.COLOR_WHITE)
                gfx.spr(1, usagi.GAME_W / 2, usagi.GAME_H / 2)
                gfx.spr_ex(1, 0, 0, true, true, math.pi / 2, gfx.COLOR_WHITE, 1.0)
                gfx.sspr(0, 0, 16, 16, 10, 10)
                gfx.sspr_ex(0, 0, 16, 16, 10, 10, 32, 32, true, false, 0, gfx.COLOR_RED, 0.5)
                gfx.pixel(5, 5, gfx.COLOR_WHITE)
                local mw, mh = usagi.measure_text("hello")
                assert(type(mw) == "number" and type(mh) == "number")
                assert(type(usagi.elapsed) == "number")
                assert(type(input.pressed(input.LEFT)) == "boolean")
                assert(type(input.held(input.BTN1)) == "boolean")
                assert(type(input.released(input.BTN1)) == "boolean")
                assert(type(input.pressed(input.BTN2)) == "boolean")
                assert(type(input.pressed(input.BTN3)) == "boolean")
                local mx, my = input.mouse()
                assert(type(mx) == "number" and type(my) == "number")
                assert(type(input.mouse_held(input.MOUSE_LEFT)) == "boolean")
                assert(type(input.mouse_pressed(input.MOUSE_RIGHT)) == "boolean")
                assert(type(input.mouse_released(input.MOUSE_LEFT)) == "boolean")
                assert(type(input.mouse_scroll()) == "number")
                assert(type(input.key_held(input.KEY_F1)) == "boolean")
                assert(type(input.key_pressed(input.KEY_BACKTICK)) == "boolean")
                assert(type(input.key_released(input.KEY_SPACE)) == "boolean")
                input.set_mouse_visible(false)
                input.set_mouse_visible(true)
                assert(type(input.mouse_visible()) == "boolean")
                sfx.play("missing")
                sfx.play_ex("missing", 0.8, 1.2, -0.5)
                music.play("missing")
                music.loop("missing")
                music.stop()
                music.play_ex("missing", 0.7, 1.0, 0.0, true)
                music.mutate(0.5, 1.2, 0.0)
                "#,
            )
            .exec()?;
            Ok(())
        })
        .expect("api smoke script failed");
    }

    /// Lua 5.4 keeps integers and floats as distinct number subtypes.
    /// `gfx.shader_uniform("u_pulse", 0)` must not be rejected just
    /// because `0` is an integer literal — both subtypes need to land
    /// as a float uniform.
    #[test]
    fn parse_uniform_accepts_integer_and_float() {
        let lua = Lua::new();
        let int_val: LuaValue = lua.load("return 0").eval().unwrap();
        match parse_uniform(&int_val).unwrap() {
            ShaderValue::Float(n) => assert_eq!(n, 0.0),
            other => panic!("expected Float, got {other:?}"),
        }

        let float_val: LuaValue = lua.load("return 0.5").eval().unwrap();
        match parse_uniform(&float_val).unwrap() {
            ShaderValue::Float(n) => assert!((n - 0.5).abs() < 1e-6),
            other => panic!("expected Float, got {other:?}"),
        }
    }

    #[test]
    fn parse_uniform_accepts_2_3_4_length_tables() {
        let lua = Lua::new();
        let v2: LuaValue = lua.load("return {1, 2}").eval().unwrap();
        assert!(matches!(parse_uniform(&v2).unwrap(), ShaderValue::Vec2(_)));

        let v3: LuaValue = lua.load("return {1, 2, 3}").eval().unwrap();
        assert!(matches!(parse_uniform(&v3).unwrap(), ShaderValue::Vec3(_)));

        let v4: LuaValue = lua.load("return {1.5, 2, 3, 4.25}").eval().unwrap();
        match parse_uniform(&v4).unwrap() {
            ShaderValue::Vec4(v) => assert_eq!(v, [1.5, 2.0, 3.0, 4.25]),
            other => panic!("expected Vec4, got {other:?}"),
        }
    }

    #[test]
    fn parse_uniform_rejects_unsupported_types() {
        let lua = Lua::new();

        let nil_val: LuaValue = LuaValue::Nil;
        let err = parse_uniform(&nil_val).unwrap_err();
        assert!(err.contains("number"), "got: {err}");

        let str_val: LuaValue = lua.load("return 'hi'").eval().unwrap();
        let err = parse_uniform(&str_val).unwrap_err();
        assert!(err.contains("number"), "got: {err}");

        let bad_table: LuaValue = lua.load("return {1, 2, 3, 4, 5}").eval().unwrap();
        let err = parse_uniform(&bad_table).unwrap_err();
        assert!(err.contains("got 5"), "got: {err}");
    }

    /// Exercises every `util.*` function with valid inputs and expected
    /// outputs. Pure-Lua, so we run a single chunk that returns true on
    /// success or raises an assertion failure with a message.
    #[test]
    fn util_functions_compute_expected_values() {
        let lua = Lua::new();
        setup_api(&lua, false).unwrap();
        lua.load(
            r#"
            local eq = function(a, b, label)
              assert(a == b, label .. ": expected " .. tostring(b) .. ", got " .. tostring(a))
            end
            local feq = function(a, b, label)
              assert(math.abs(a - b) < 1e-9, label .. ": expected ~" .. b .. ", got " .. a)
            end

            eq(util.clamp(5, 0, 10), 5, "clamp mid")
            eq(util.clamp(-1, 0, 10), 0, "clamp low")
            eq(util.clamp(11, 0, 10), 10, "clamp high")

            feq(util.lerp(0, 10, 0), 0, "lerp 0")
            feq(util.lerp(0, 10, 1), 10, "lerp 1")
            feq(util.lerp(0, 10, 0.5), 5, "lerp half")
            feq(util.lerp(0, 10, 2), 20, "lerp extrapolate")

            feq(util.wrap(0, 0, 4), 0, "wrap zero")
            feq(util.wrap(4, 0, 4), 0, "wrap upper")
            feq(util.wrap(-1, 0, 4), 3, "wrap negative")
            feq(util.wrap(5, 0, 4), 1, "wrap above")
            feq(util.wrap(0, 1, 5), 4, "wrap into shifted span")

            local n = util.vec_normalize({ x = 10, y = 0 })
            feq(n.x, 1, "normalize x"); feq(n.y, 0, "normalize y")
            local z = util.vec_normalize({ x = 0, y = 0 })
            feq(z.x, 0, "zero vec x"); feq(z.y, 0, "zero vec y")
            local d = util.vec_normalize({ x = 3, y = 4 })
            feq(d.x, 0.6, "diag x"); feq(d.y, 0.8, "diag y")

            assert(util.rect_overlap({x=0,y=0,w=10,h=10}, {x=5,y=5,w=10,h=10}), "rect overlap")
            assert(not util.rect_overlap({x=0,y=0,w=10,h=10}, {x=10,y=0,w=10,h=10}), "edge-adjacent rects do not overlap")
            assert(not util.rect_overlap({x=0,y=0,w=10,h=10}, {x=20,y=20,w=10,h=10}), "far rects")

            assert(util.circ_overlap({x=0,y=0,r=5}, {x=3,y=0,r=5}), "circ overlap")
            assert(not util.circ_overlap({x=0,y=0,r=5}, {x=10,y=0,r=5}), "tangent circs do not overlap")
            assert(not util.circ_overlap({x=0,y=0,r=5}, {x=20,y=0,r=5}), "far circs")

            assert(util.circ_rect_overlap({x=5,y=5,r=3}, {x=0,y=0,w=10,h=10}), "circ inside rect")
            assert(util.circ_rect_overlap({x=12,y=5,r=3}, {x=0,y=0,w=10,h=10}), "circ overlapping rect edge")
            assert(not util.circ_rect_overlap({x=20,y=20,r=3}, {x=0,y=0,w=10,h=10}), "circ far from rect")

            -- point_in_rect: half-open [x, x+w) on each axis.
            assert(util.point_in_rect({x=5,y=5}, {x=0,y=0,w=10,h=10}), "point inside rect")
            assert(util.point_in_rect({x=0,y=0}, {x=0,y=0,w=10,h=10}), "point at top-left edge is inside")
            assert(not util.point_in_rect({x=10,y=5}, {x=0,y=0,w=10,h=10}), "point at right edge is outside")
            assert(not util.point_in_rect({x=5,y=10}, {x=0,y=0,w=10,h=10}), "point at bottom edge is outside")
            assert(not util.point_in_rect({x=-1,y=5}, {x=0,y=0,w=10,h=10}), "point left of rect")

            -- point_in_circ: strict (boundary outside, matching circ_overlap).
            assert(util.point_in_circ({x=0,y=0}, {x=0,y=0,r=5}), "point at center")
            assert(util.point_in_circ({x=3,y=0}, {x=0,y=0,r=5}), "point inside circle")
            assert(not util.point_in_circ({x=5,y=0}, {x=0,y=0,r=5}), "point on boundary is outside")
            assert(not util.point_in_circ({x=10,y=0}, {x=0,y=0,r=5}), "point far from circle")

            eq(util.sign(5), 1, "sign positive")
            eq(util.sign(-3), -1, "sign negative")
            eq(util.sign(0), 0, "sign zero")

            feq(util.round(0.4), 0, "round down")
            feq(util.round(0.5), 1, "round half up")
            feq(util.round(0.51), 1, "round up")
            feq(util.round(-0.6), -1, "round neg down")

            -- approach caps so it never overshoots its target
            feq(util.approach(0, 10, 5), 5, "approach mid")
            feq(util.approach(8, 10, 5), 10, "approach caps at target")
            feq(util.approach(10, 0, 3), 7, "approach down")
            feq(util.approach(5, 5, 100), 5, "approach already there")

            feq(util.vec_dist({x=0,y=0}, {x=3,y=4}), 5, "vec_dist 3-4-5")
            feq(util.vec_dist({x=10,y=10}, {x=10,y=10}), 0, "vec_dist same point")
            feq(util.vec_dist_sq({x=0,y=0}, {x=3,y=4}), 25, "vec_dist_sq 3-4-25")

            local v = util.vec_from_angle(0, 10)
            feq(v.x, 10, "vec_from_angle 0 x"); feq(v.y, 0, "vec_from_angle 0 y")
            local u = util.vec_from_angle(math.pi / 2, 4)
            feq(u.y, 4, "vec_from_angle pi/2 y")
            assert(math.abs(u.x) < 1e-9, "vec_from_angle pi/2 x near zero")
            local unit = util.vec_from_angle(0)
            feq(unit.x, 1, "vec_from_angle default len = 1")

            -- flash: at 4 hz the on/off interval is 0.25s
            assert(util.flash(0, 4) == true, "flash true at t=0")
            assert(util.flash(0.125, 4) == true, "flash still on within first interval")
            assert(util.flash(0.25, 4) == false, "flash off after first interval")
            assert(util.flash(0.5, 4) == true, "flash on again")
            "#,
        )
        .exec()
        .expect("util correctness");
    }

    /// Each shape-checked util raises a helpful error when given a
    /// table missing a required field, so users see "missing field 'h'"
    /// instead of "attempt to perform arithmetic on a nil value."
    #[test]
    fn util_shape_assertions_fire_with_helpful_messages() {
        let lua = Lua::new();
        setup_api(&lua, false).unwrap();

        let cases: &[(&str, &str, &str)] = &[
            (
                "util.rect_overlap({x=0,y=0,w=10,h=10}, {x=0,y=0,w=10})",
                "rect_overlap",
                "'h'",
            ),
            (
                "util.rect_overlap({x=0,y=0,w=10,h=10}, 'nope')",
                "rect_overlap",
                "must be a table",
            ),
            (
                "util.circ_overlap({x=0,y=0}, {x=0,y=0,r=5})",
                "circ_overlap",
                "'r'",
            ),
            (
                "util.circ_rect_overlap({x=0,y=0,r=5}, {x=0,y=0,w=10})",
                "circ_rect_overlap",
                "'h'",
            ),
            ("util.vec_normalize({x=0})", "vec_normalize", "'y'"),
            ("util.vec_dist({x=0,y=0}, {x=0})", "vec_dist", "'y'"),
            (
                "util.vec_dist_sq({x=0,y=0}, 'oops')",
                "vec_dist_sq",
                "must be a table",
            ),
            (
                "util.point_in_rect({x=0}, {x=0,y=0,w=10,h=10})",
                "point_in_rect",
                "'y'",
            ),
            (
                "util.point_in_circ({x=0,y=0}, {x=0,y=0})",
                "point_in_circ",
                "'r'",
            ),
        ];

        for (snippet, fn_name, expected) in cases {
            let err = lua
                .load(*snippet)
                .exec()
                .expect_err(&format!("expected {snippet} to error"));
            let msg = err.to_string();
            assert!(
                msg.contains(fn_name),
                "{snippet}: error should mention '{fn_name}', got: {msg}"
            );
            assert!(
                msg.contains(expected),
                "{snippet}: error should mention '{expected}', got: {msg}"
            );
        }
    }

    #[test]
    fn usagi_dump_renders_nested_table_with_sorted_keys() {
        let lua = Lua::new();
        setup_api(&lua, false).unwrap();
        let usagi: LuaTable = lua.globals().get("usagi").unwrap();
        let dump: LuaFunction = usagi.get("dump").unwrap();
        let t: LuaTable = lua
            .load(
                r#"
                return {
                    name = "snake",
                    level = 3,
                    tags = { "fast", "small" },
                }
                "#,
            )
            .eval()
            .unwrap();
        let s: String = dump.call(t).unwrap();
        assert!(s.contains("name = \"snake\""), "got: {s}");
        assert!(s.contains("level = 3"), "got: {s}");
        // Array entries render in order, no `[1] =` prefix.
        assert!(s.contains("\"fast\""), "got: {s}");
        assert!(s.contains("\"small\""), "got: {s}");
        // Keys are sorted: level comes before name comes before tags.
        let lvl = s.find("level").unwrap();
        let nm = s.find("name").unwrap();
        let tg = s.find("tags").unwrap();
        assert!(lvl < nm && nm < tg, "keys not sorted: {s}");
    }

    #[test]
    fn usagi_dump_handles_primitives_and_cycles() {
        let lua = Lua::new();
        setup_api(&lua, false).unwrap();
        let usagi: LuaTable = lua.globals().get("usagi").unwrap();
        let dump: LuaFunction = usagi.get("dump").unwrap();

        assert_eq!(dump.call::<String>(42i32).unwrap(), "42");
        assert_eq!(dump.call::<String>(true).unwrap(), "true");
        assert_eq!(dump.call::<String>(LuaValue::Nil).unwrap(), "nil");
        // %q quotes the string.
        assert_eq!(dump.call::<String>("hi").unwrap(), "\"hi\"");
        // Empty table is the literal "{}".
        let empty: LuaTable = lua.create_table().unwrap();
        assert_eq!(dump.call::<String>(empty).unwrap(), "{}");

        // A self-referencing table renders <cycle> instead of recursing forever.
        let t: LuaTable = lua
            .load(
                r#"
                local a = {}
                a.self = a
                return a
                "#,
            )
            .eval()
            .unwrap();
        let s: String = dump.call(t).unwrap();
        assert!(s.contains("<cycle>"), "got: {s}");
    }

    #[test]
    fn platform_is_one_of_the_known_values() {
        // The set is a stable contract for games doing
        // `usagi.PLATFORM == "web"` checks. Unknown is allowed for
        // builds on uncovered targets (BSDs etc.) but should never
        // come up on the four shipped export targets.
        let p = current_platform();
        assert!(
            matches!(p, "web" | "macos" | "linux" | "windows" | "unknown"),
            "unexpected platform: {p}",
        );
    }

    #[test]
    fn setup_exposes_platform_on_usagi_table() {
        let lua = Lua::new();
        setup_api(&lua, false).unwrap();
        let usagi: LuaTable = lua.globals().get("usagi").unwrap();
        let p: String = usagi.get("PLATFORM").unwrap();
        assert_eq!(p, current_platform());
    }
}
