//! Persistent save data: a single Lua table round-tripped through JSON.
//!
//! API surface (in Lua):
//!
//! ```lua
//! usagi.save({ score = 200, settings = { volume = 0.7 } })
//! local data = usagi.load()  -- table on hit, nil on first run
//! ```
//!
//! ## Format choice: JSON
//! Plain text, externally editable, human-debuggable. Matches "the player
//! sends you their save file" workflows. Lua-source saves (a `return {...}`
//! file roundtripped through `loadstring`) would also work but exposes a
//! code-execution surface we'd rather not have on data the player can
//! tamper with.
//!
//! ## Where saves live
//! - Native: `<data_dir>/<game_id>/save.json`. `<data_dir>` is whatever
//!   the `directories` crate considers right for the OS (linux:
//!   `~/.local/share`, macOS: `~/Library/Application Support`, Windows:
//!   `%APPDATA%`).
//! - Web: `localStorage` under key `usagi.save.<game_id>`. localStorage
//!   was picked over IDBFS so the save layer Just Works regardless of
//!   what custom shells do — there's no `FS.syncfs()` dance, no
//!   shell-side cooperation needed.
//!
//! ## game_id
//! Required for save/load. Validated at the point of the first
//! `save`/`load` call rather than at startup so games that don't
//! persist data don't have to declare one. The convention is
//! reverse-DNS: `com.brettmakesgames.snake`. Matches Playdate
//! bundle IDs and lines up with what macOS app bundles, iOS
//! bundles, and Windows packaged apps all want, so the same
//! string is reusable when packaging targets are added later.
//!
//! ## Atomic writes (native)
//! Write to `save.json.tmp`, then `rename` over `save.json`. A
//! crash mid-write leaves the previous save intact and a stale
//! `.tmp` that we ignore on read. POSIX `rename` is atomic on the
//! same filesystem; Windows `MoveFileEx` with `MOVEFILE_REPLACE_EXISTING`
//! has the same semantics, which is what `std::fs::rename` uses.

use mlua::{Lua, LuaSerdeExt, Value};

#[cfg(not(target_os = "emscripten"))]
use std::path::PathBuf;

const SAVE_FILE: &str = "save.json";
const SAVE_FILE_TMP: &str = "save.json.tmp";

/// Serializes a Lua value (typically a table) to a pretty-printed JSON
/// string. Errors surface to Lua as runtime errors with whatever
/// context serde_json gives us (e.g. "key must be a string" for
/// integer-keyed maps that aren't 1..n arrays).
pub fn lua_to_json(lua: &Lua, value: Value) -> mlua::Result<String> {
    let json: serde_json::Value = lua.from_value(value)?;
    serde_json::to_string_pretty(&json)
        .map_err(|e| mlua::Error::external(format!("save: serialize: {e}")))
}

/// Parses a JSON string into a Lua value. JSON arrays become 1-indexed
/// Lua arrays, JSON objects become Lua tables with string keys. Returns
/// a Lua error (not a panic) on malformed input.
pub fn json_to_lua(lua: &Lua, s: &str) -> mlua::Result<Value> {
    let json: serde_json::Value =
        serde_json::from_str(s).map_err(|e| mlua::Error::external(format!("load: parse: {e}")))?;
    lua.to_value(&json)
}

/// Lightweight check that the dev-supplied id is sane enough to hand
/// to the filesystem. We're not trying to be a security boundary, just
/// catching the obvious footguns (empty string, parent-dir traversal,
/// path separators) that would land saves in surprising places.
pub fn validate_game_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("game_id cannot be empty".into());
    }
    if id.contains('/') || id.contains('\\') || id.contains("..") {
        return Err(format!(
            "game_id '{id}' contains illegal characters (no '/', '\\', or '..')"
        ));
    }
    Ok(())
}

#[cfg(not(target_os = "emscripten"))]
fn save_dir(game_id: &str) -> std::io::Result<PathBuf> {
    use directories::ProjectDirs;
    // ProjectDirs::from(qualifier, organization, application). Empty
    // qualifier and organization keep the path short on macOS. We get
    // `~/Library/Application Support/<game_id>` instead of
    // `.../<org>.<game_id>`.
    ProjectDirs::from("", "", game_id)
        .map(|p| p.data_dir().to_path_buf())
        .ok_or_else(|| std::io::Error::other("could not resolve data dir for this OS"))
}

#[cfg(not(target_os = "emscripten"))]
pub fn write_save(game_id: &str, contents: &str) -> std::io::Result<()> {
    let dir = save_dir(game_id)?;
    std::fs::create_dir_all(&dir)?;
    let final_path = dir.join(SAVE_FILE);
    let tmp_path = dir.join(SAVE_FILE_TMP);
    std::fs::write(&tmp_path, contents)?;
    std::fs::rename(&tmp_path, &final_path)?;
    Ok(())
}

#[cfg(not(target_os = "emscripten"))]
pub fn read_save(game_id: &str) -> std::io::Result<Option<String>> {
    let path = save_dir(game_id)?.join(SAVE_FILE);
    match std::fs::read_to_string(&path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(target_os = "emscripten")]
mod web {
    use std::ffi::{CStr, CString};
    use std::os::raw::c_char;

    // Defined by `web/usagi_save.js` and linked via `--js-library`.
    // `usagi_save_read` returns a malloc'd C string the caller must
    // free with `usagi_save_free`, or a null pointer if the key is
    // absent. We could free with `libc::free` directly but routing it
    // through the JS side keeps the allocation lifecycle symmetric.
    unsafe extern "C" {
        fn usagi_save_write(key: *const c_char, val: *const c_char);
        fn usagi_save_read(key: *const c_char) -> *mut c_char;
        fn usagi_save_free(val: *mut c_char);
    }

    pub fn write_save(game_id: &str, contents: &str) -> std::io::Result<()> {
        let key = CString::new(format!("usagi.save.{game_id}"))
            .map_err(|_| std::io::Error::other("game_id contained NUL byte"))?;
        let val = CString::new(contents)
            .map_err(|_| std::io::Error::other("save data contained NUL byte"))?;
        unsafe {
            usagi_save_write(key.as_ptr(), val.as_ptr());
        }
        Ok(())
    }

    pub fn read_save(game_id: &str) -> std::io::Result<Option<String>> {
        let key = CString::new(format!("usagi.save.{game_id}"))
            .map_err(|_| std::io::Error::other("game_id contained NUL byte"))?;
        unsafe {
            let p = usagi_save_read(key.as_ptr());
            if p.is_null() {
                return Ok(None);
            }
            let s = CStr::from_ptr(p).to_string_lossy().into_owned();
            usagi_save_free(p);
            Ok(Some(s))
        }
    }
}

#[cfg(target_os = "emscripten")]
pub use web::{read_save, write_save};

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::Lua;

    #[test]
    fn roundtrips_simple_table() {
        let lua = Lua::new();
        let t: mlua::Table = lua
            .load(r#"return { score = 200, name = "brett", alive = true }"#)
            .eval()
            .unwrap();
        let json = lua_to_json(&lua, Value::Table(t)).unwrap();
        let v = json_to_lua(&lua, &json).unwrap();
        let back = match v {
            Value::Table(t) => t,
            other => panic!("expected table, got {other:?}"),
        };
        assert_eq!(back.get::<i64>("score").unwrap(), 200);
        assert_eq!(back.get::<String>("name").unwrap(), "brett");
        assert!(back.get::<bool>("alive").unwrap());
    }

    #[test]
    fn roundtrips_nested_table() {
        let lua = Lua::new();
        let t: mlua::Table = lua
            .load(
                r#"return {
                    settings = { volume = 0.7, fullscreen = false },
                    run = { score = 12, level = 3 },
                }"#,
            )
            .eval()
            .unwrap();
        let json = lua_to_json(&lua, Value::Table(t)).unwrap();
        let v = json_to_lua(&lua, &json).unwrap();
        let Value::Table(back) = v else { panic!() };
        let settings: mlua::Table = back.get("settings").unwrap();
        assert!((settings.get::<f64>("volume").unwrap() - 0.7).abs() < 1e-9);
        assert!(!settings.get::<bool>("fullscreen").unwrap());
    }

    #[test]
    fn roundtrips_array_table() {
        let lua = Lua::new();
        let t: mlua::Table = lua.load(r#"return {10, 20, 30}"#).eval().unwrap();
        let json = lua_to_json(&lua, Value::Table(t)).unwrap();
        // 1..n integer keys should serialize as a JSON array, not an object.
        assert!(json.contains('['), "expected array, got: {json}");
        let Value::Table(back) = json_to_lua(&lua, &json).unwrap() else {
            panic!()
        };
        assert_eq!(back.get::<i64>(1).unwrap(), 10);
        assert_eq!(back.get::<i64>(3).unwrap(), 30);
    }

    #[test]
    fn rejects_function_values() {
        let lua = Lua::new();
        let t: mlua::Table = lua
            .load(r#"return { fn = function() return 1 end }"#)
            .eval()
            .unwrap();
        let err = lua_to_json(&lua, Value::Table(t)).unwrap_err();
        // Don't pin the exact text; just confirm we got a serialization
        // error rather than panicking or silently dropping the key.
        let msg = err.to_string();
        assert!(
            msg.to_lowercase().contains("function") || msg.to_lowercase().contains("serialize"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn parse_error_surfaces_as_lua_error() {
        let lua = Lua::new();
        let err = json_to_lua(&lua, "{not valid json").unwrap_err();
        assert!(err.to_string().to_lowercase().contains("load"));
    }

    #[test]
    fn validate_game_id_rejects_bad_inputs() {
        assert!(validate_game_id("").is_err());
        assert!(validate_game_id("foo/bar").is_err());
        assert!(validate_game_id("..").is_err());
        assert!(validate_game_id("foo\\bar").is_err());
        assert!(validate_game_id("com..foo").is_err()); // consecutive dots
        // The reverse-DNS convention should pass cleanly. Single dots
        // are fine, only the parent-dir traversal pattern is rejected.
        assert!(validate_game_id("com.brettmakesgames.snake").is_ok());
        assert!(validate_game_id("brett_snake").is_ok());
        assert!(validate_game_id("Snake-2026").is_ok());
    }
}
