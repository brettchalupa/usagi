//! Static Lua API: installs the `gfx`, `input`, `sfx`, and `usagi` tables
//! with constants. The per-frame closures (gfx.clear, input.pressed, etc.)
//! live in the game loop because they need to borrow frame-local state.

use crate::input::{
    ACTION_BTN1, ACTION_BTN2, ACTION_BTN3, ACTION_DOWN, ACTION_LEFT, ACTION_RIGHT, ACTION_UP,
};
use crate::{GAME_HEIGHT, GAME_WIDTH};
use mlua::prelude::*;

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
    use crate::palette::palette;

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

    /// Every `gfx.COLOR_*` constant must map to a real palette() entry.
    /// Guards against adding a new color constant without teaching
    /// `palette()`, which would silently render as magenta.
    #[test]
    fn every_gfx_color_maps_to_a_distinct_palette_entry() {
        let lua = Lua::new();
        setup_api(&lua, false).unwrap();
        let gfx: LuaTable = lua.globals().get("gfx").unwrap();

        let magenta = palette(i32::MAX); // known sentinel color
        let mut indices: Vec<i32> = Vec::new();

        for pair in gfx.pairs::<String, i32>() {
            let (name, idx) = pair.unwrap();
            if !name.starts_with("COLOR_") {
                continue;
            }
            let c = palette(idx);
            assert!(
                (c.r, c.g, c.b) != (magenta.r, magenta.g, magenta.b),
                "{name}={idx} falls through to the magenta sentinel in palette()",
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
    /// `input.down(input.X)` always return false.
    #[test]
    fn every_input_constant_is_a_valid_action() {
        let lua = Lua::new();
        setup_api(&lua, false).unwrap();
        let input: LuaTable = lua.globals().get("input").unwrap();
        let mut checked = 0;
        for pair in input.pairs::<String, u32>() {
            let (name, code) = pair.unwrap();
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
            input.set("down", scope.create_function(|_, _k: u32| Ok(false))?)?;

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
                assert(type(input.down(input.BTN1)) == "boolean")
                assert(type(input.pressed(input.BTN2)) == "boolean")
                assert(type(input.pressed(input.BTN3)) == "boolean")
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
}
