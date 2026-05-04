//! Static Lua API: installs the `gfx`, `input`, `sfx`, and `usagi` tables
//! with constants. The per-frame closures (gfx.clear, input.pressed, etc.)
//! live in the game loop because they need to borrow frame-local state.

use crate::input::{
    ACTION_BTN1, ACTION_BTN2, ACTION_BTN3, ACTION_DOWN, ACTION_LEFT, ACTION_RIGHT, ACTION_UP,
    KEY_TABLE, MOUSE_LEFT, MOUSE_RIGHT,
};
use crate::shader::{ShaderManager, ShaderValue};
use crate::{GAME_HEIGHT, GAME_WIDTH};
use mlua::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Installs the Lua-facing globals: `gfx`, `input`, `sfx`, `usagi`. Each is a
/// table with any constants it owns. Per-frame function members (e.g.
/// gfx.clear, sfx.play) are registered inside `lua.scope` blocks in the main
/// loop so their closures can borrow the current frame's draw handle, audio
/// device, etc.
pub fn setup_api(lua: &Lua, dev: bool) -> LuaResult<()> {
    let gfx = lua.create_table()?;
    gfx.set("COLOR_BLACK", 0)?;
    gfx.set("COLOR_DARK_BLUE", 1)?;
    gfx.set("COLOR_DARK_PURPLE", 2)?;
    gfx.set("COLOR_DARK_GREEN", 3)?;
    gfx.set("COLOR_BROWN", 4)?;
    gfx.set("COLOR_DARK_GRAY", 5)?;
    gfx.set("COLOR_LIGHT_GRAY", 6)?;
    gfx.set("COLOR_WHITE", 7)?;
    gfx.set("COLOR_RED", 8)?;
    gfx.set("COLOR_ORANGE", 9)?;
    gfx.set("COLOR_YELLOW", 10)?;
    gfx.set("COLOR_GREEN", 11)?;
    gfx.set("COLOR_BLUE", 12)?;
    gfx.set("COLOR_INDIGO", 13)?;
    gfx.set("COLOR_PINK", 14)?;
    gfx.set("COLOR_PEACH", 15)?;
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
    usagi.set("GAME_W", GAME_WIDTH)?;
    usagi.set("GAME_H", GAME_HEIGHT)?;
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
    lua.globals().set("usagi", usagi)?;

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
    gfx.set("shader_set", shader_set)?;

    let m = Rc::clone(mgr);
    let shader_uniform = lua.create_function(move |_, (name, value): (String, LuaValue)| {
        let v = parse_uniform(&value).map_err(mlua::Error::external)?;
        m.borrow_mut().queue_uniform(name, v);
        Ok(())
    })?;
    gfx.set("shader_uniform", shader_uniform)?;

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

/// Records a Lua error: prints to stderr and stores the message so it can be
/// displayed on-screen. Wraps every call into user Lua so a typo / nil-call /
/// runtime error doesn't tear down the process.
pub fn record_err(state: &mut Option<String>, label: &str, result: LuaResult<()>) {
    if let Err(e) = result {
        let msg = format!("{}: {}", label, e);
        eprintln!("[usagi] {}", msg);
        *state = Some(msg);
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

        assert_eq!(gfx.get::<i32>("COLOR_BLACK").unwrap(), 0);
        assert_eq!(gfx.get::<i32>("COLOR_WHITE").unwrap(), 7);
        assert_eq!(gfx.get::<i32>("COLOR_RED").unwrap(), 8);
        assert_eq!(gfx.get::<i32>("COLOR_PEACH").unwrap(), 15);

        // Input constants just need to be present; values are action IDs.
        assert!(input.get::<u32>("LEFT").is_ok());
        assert!(input.get::<u32>("BTN1").is_ok());
        assert!(input.get::<u32>("BTN2").is_ok());
        assert!(input.get::<u32>("BTN3").is_ok());
        assert!(input.get::<u32>("MOUSE_LEFT").is_ok());
        assert!(input.get::<u32>("MOUSE_RIGHT").is_ok());

        // sfx and music are registered but empty of fields at
        // static-setup time — their per-frame closures live in the
        // session loop.
        assert!(sfx.get::<LuaValue>("play").unwrap().is_nil());
        assert!(music.get::<LuaValue>("play").unwrap().is_nil());
        assert!(music.get::<LuaValue>("loop").unwrap().is_nil());
        assert!(music.get::<LuaValue>("stop").unwrap().is_nil());

        assert_eq!(usagi.get::<f32>("GAME_W").unwrap(), GAME_WIDTH);
        assert_eq!(usagi.get::<f32>("GAME_H").unwrap(), GAME_HEIGHT);
        assert_eq!(usagi.get::<f64>("elapsed").unwrap(), 0.0);
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
                "text",
                scope.create_function(|_, _a: (String, f32, f32, i32)| Ok(()))?,
            )?;
            gfx.set(
                "spr",
                scope.create_function(|_, _a: (i32, f32, f32)| Ok(()))?,
            )?;
            gfx.set(
                "spr_ex",
                scope.create_function(|_, _a: (i32, f32, f32, bool, bool)| Ok(()))?,
            )?;
            gfx.set(
                "sspr",
                scope.create_function(|_, _a: (f32, f32, f32, f32, f32, f32)| Ok(()))?,
            )?;
            type SsprExArgs = (f32, f32, f32, f32, f32, f32, f32, f32, bool, bool);
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

            let music: LuaTable = lua.globals().get("music")?;
            music.set("play", scope.create_function(|_, _n: String| Ok(()))?)?;
            music.set("loop", scope.create_function(|_, _n: String| Ok(()))?)?;
            music.set("stop", scope.create_function(|_, ()| Ok(()))?)?;

            lua.load(
                r#"
                gfx.clear(gfx.COLOR_BLACK)
                gfx.rect(10, 20, 30, 40, gfx.COLOR_RED)
                gfx.rect_fill(10, 20, 30, 40, gfx.COLOR_BLUE)
                gfx.circ(50, 50, 8, gfx.COLOR_GREEN)
                gfx.circ_fill(60, 60, 4, gfx.COLOR_YELLOW)
                gfx.line(0, 0, 100, 100, gfx.COLOR_WHITE)
                gfx.text("hi", 0, 0, gfx.COLOR_WHITE)
                gfx.spr(1, usagi.GAME_W / 2, usagi.GAME_H / 2)
                gfx.spr_ex(1, 0, 0, true, true)
                gfx.sspr(0, 0, 16, 16, 10, 10)
                gfx.sspr_ex(0, 0, 16, 16, 10, 10, 32, 32, true, false)
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
                assert(type(input.key_held(input.KEY_F1)) == "boolean")
                assert(type(input.key_pressed(input.KEY_BACKTICK)) == "boolean")
                assert(type(input.key_released(input.KEY_SPACE)) == "boolean")
                input.set_mouse_visible(false)
                input.set_mouse_visible(true)
                assert(type(input.mouse_visible()) == "boolean")
                sfx.play("missing")
                music.play("missing")
                music.loop("missing")
                music.stop()
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
}
