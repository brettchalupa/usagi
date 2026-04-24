//! Static Lua API: installs the `gfx`, `input`, `sfx`, and `usagi` tables
//! with constants. The per-frame closures (gfx.clear, input.pressed, etc.)
//! live in the game loop because they need to borrow frame-local state.

use crate::{GAME_HEIGHT, GAME_WIDTH};
use mlua::prelude::*;
use sola_raylib::prelude::*;

/// Installs the Lua-facing globals: `gfx`, `input`, `sfx`, `usagi`. Each is a
/// table with any constants it owns. Per-frame function members (e.g.
/// gfx.clear, sfx.play) are registered inside `lua.scope` blocks in the main
/// loop so their closures can borrow the current frame's draw handle, audio
/// device, etc.
pub fn setup_api(lua: &Lua) -> LuaResult<()> {
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
    input.set("LEFT", KeyboardKey::KEY_LEFT as u32)?;
    input.set("RIGHT", KeyboardKey::KEY_RIGHT as u32)?;
    input.set("UP", KeyboardKey::KEY_UP as u32)?;
    input.set("DOWN", KeyboardKey::KEY_DOWN as u32)?;
    input.set("A", KeyboardKey::KEY_Z as u32)?;
    input.set("B", KeyboardKey::KEY_X as u32)?;
    lua.globals().set("input", input)?;

    let sfx = lua.create_table()?;
    lua.globals().set("sfx", sfx)?;

    // `gfx` / `input` are top-level globals (see above). The `usagi` table is
    // reserved for engine-level info: runtime constants, current frame stats,
    // etc. Not a namespace for the per-domain APIs.
    let usagi = lua.create_table()?;
    usagi.set("GAME_W", GAME_WIDTH)?;
    usagi.set("GAME_H", GAME_HEIGHT)?;
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

    #[test]
    fn setup_installs_expected_globals() {
        let lua = Lua::new();
        setup_api(&lua).unwrap();

        let gfx: LuaTable = lua.globals().get("gfx").unwrap();
        let input: LuaTable = lua.globals().get("input").unwrap();
        let sfx: LuaTable = lua.globals().get("sfx").unwrap();
        let usagi: LuaTable = lua.globals().get("usagi").unwrap();

        assert_eq!(gfx.get::<i32>("COLOR_BLACK").unwrap(), 0);
        assert_eq!(gfx.get::<i32>("COLOR_WHITE").unwrap(), 7);
        assert_eq!(gfx.get::<i32>("COLOR_RED").unwrap(), 8);
        assert_eq!(gfx.get::<i32>("COLOR_PEACH").unwrap(), 15);

        // Input constants just need to be present; values are raylib keycodes.
        assert!(input.get::<u32>("LEFT").is_ok());
        assert!(input.get::<u32>("A").is_ok());

        // sfx is registered but empty of fields at static-setup time.
        assert!(sfx.get::<LuaValue>("play").unwrap().is_nil());

        assert_eq!(usagi.get::<f32>("GAME_W").unwrap(), GAME_WIDTH);
        assert_eq!(usagi.get::<f32>("GAME_H").unwrap(), GAME_HEIGHT);
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
}
