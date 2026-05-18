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

use crate::game_id::GameId;
use mlua::{Lua, LuaSerdeExt, Value};

#[cfg(not(target_os = "emscripten"))]
use std::path::PathBuf;

const SAVE_FILE: &str = "save.json";
const SAVE_FILE_TMP: &str = "save.json.tmp";

/// Serializes a Lua value (typically a table) to a pretty-printed JSON
/// string. Validates the table shape up front so the user gets a clear
/// "JSON can't hold this" message instead of a cryptic serde error or,
/// worse, silent data loss when mlua treats a sparse near-array as its
/// dense prefix.
pub fn lua_to_json(lua: &Lua, value: Value) -> mlua::Result<String> {
    if let Value::Table(ref t) = value {
        validate_save_table(t)?;
    }
    let json: serde_json::Value = lua.from_value(value)?;
    serde_json::to_string_pretty(&json)
        .map_err(|e| mlua::Error::external(format!("save: serialize: {e}")))
}

/// Walks a Lua table (and its nested tables) and rejects shapes that
/// don't round-trip cleanly through JSON: integer keys that aren't a
/// dense `1..n` array, mixed string+integer keys, and non-string /
/// non-integer keys. The error message points at the workaround
/// (`tostring(k)` for maps, fill `1..n` for arrays) instead of the raw
/// serde "expected a string key" wording.
fn validate_save_table(table: &mlua::Table) -> mlua::Result<()> {
    let mut int_keys: Vec<i64> = Vec::new();
    let mut string_count: usize = 0;
    let mut nested: Vec<mlua::Table> = Vec::new();
    for pair in table.pairs::<Value, Value>() {
        let (k, v) = pair?;
        match k {
            Value::String(_) => string_count += 1,
            Value::Integer(n) => int_keys.push(n),
            Value::Number(n)
                if n.is_finite() && n.fract() == 0.0 && n.abs() < (i64::MAX as f64) =>
            {
                int_keys.push(n as i64);
            }
            other => {
                return Err(mlua::Error::external(format!(
                    "usagi.save: table key must be a string or 1..n integer; got {}. \
                     JSON only supports string keys (and 1..n arrays).",
                    other.type_name()
                )));
            }
        }
        if let Value::Table(t) = v {
            nested.push(t);
        }
    }
    if string_count > 0 && !int_keys.is_empty() {
        return Err(mlua::Error::external(
            "usagi.save: table mixes string and integer keys. JSON tables hold either a \
             map (all string keys) or a dense 1..n array, not both. \
             Convert integer keys with tostring(k) to save as a map.",
        ));
    }
    if !int_keys.is_empty() {
        int_keys.sort_unstable();
        let n = int_keys.len() as i64;
        let dense_1_to_n = int_keys[0] == 1 && int_keys[(n - 1) as usize] == n;
        if !dense_1_to_n {
            return Err(mlua::Error::external(format!(
                "usagi.save: integer-keyed table must be a dense 1..n array (no gaps, starting at 1); \
                 got keys {int_keys:?}. \
                 Convert the keys with tostring(k) to save as a map, or fill 1..n for an array.",
            )));
        }
    }
    for t in nested {
        validate_save_table(&t)?;
    }
    Ok(())
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
pub fn save_dir(game_id: &GameId) -> std::io::Result<PathBuf> {
    use directories::ProjectDirs;
    // ProjectDirs::from(qualifier, organization, application). Empty
    // qualifier and organization keep the path short on macOS. We get
    // `~/Library/Application Support/<game_id>` instead of
    // `.../<org>.<game_id>`.
    ProjectDirs::from("", "", game_id.as_str())
        .map(|p| p.data_dir().to_path_buf())
        .ok_or_else(|| std::io::Error::other("could not resolve data dir for this OS"))
}

/// Absolute path to the save file for `game_id`. Returns the path even
/// if the file or its parent directory don't exist yet, so callers can
/// display "would be saved at: ..." messaging.
#[cfg(not(target_os = "emscripten"))]
pub fn save_path(game_id: &GameId) -> std::io::Result<PathBuf> {
    Ok(save_dir(game_id)?.join(SAVE_FILE))
}

/// Removes the save file. No-op if it doesn't exist (a "clear" of an
/// empty save shouldn't error). The parent directory is left in
/// place; an empty `<game_id>/` dir is harmless and gets reused on
/// next write.
#[cfg(not(target_os = "emscripten"))]
pub fn clear_save(game_id: &GameId) -> std::io::Result<()> {
    let path = save_path(game_id)?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(not(target_os = "emscripten"))]
pub fn write_save(game_id: &GameId, contents: &str) -> std::io::Result<()> {
    let dir = save_dir(game_id)?;
    std::fs::create_dir_all(&dir)?;
    let final_path = dir.join(SAVE_FILE);
    let tmp_path = dir.join(SAVE_FILE_TMP);
    std::fs::write(&tmp_path, contents)?;
    std::fs::rename(&tmp_path, &final_path)?;
    Ok(())
}

#[cfg(not(target_os = "emscripten"))]
pub fn read_save(game_id: &GameId) -> std::io::Result<Option<String>> {
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

    /// Generic key/value write into the JS-side storage shim. Despite
    /// the FFI symbols being named `usagi_save_*`, the JS side just
    /// proxies localStorage with no opinion on the key. Exposed
    /// `pub(crate)` so siblings (settings, future per-game state)
    /// can share one localStorage-shaped backend without duplicating
    /// the FFI declarations.
    pub(crate) fn kv_write(key: &str, val: &str) -> std::io::Result<()> {
        let key = CString::new(key).map_err(|_| std::io::Error::other("key contained NUL byte"))?;
        let val =
            CString::new(val).map_err(|_| std::io::Error::other("value contained NUL byte"))?;
        unsafe {
            usagi_save_write(key.as_ptr(), val.as_ptr());
        }
        Ok(())
    }

    /// Generic key/value read from the JS-side storage shim. See
    /// `kv_write` for the rationale on the shared shim.
    pub(crate) fn kv_read(key: &str) -> std::io::Result<Option<String>> {
        let key = CString::new(key).map_err(|_| std::io::Error::other("key contained NUL byte"))?;
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

    pub fn write_save(game_id: &super::GameId, contents: &str) -> std::io::Result<()> {
        kv_write(&format!("usagi.save.{}", game_id.as_str()), contents)
    }

    pub fn read_save(game_id: &super::GameId) -> std::io::Result<Option<String>> {
        kv_read(&format!("usagi.save.{}", game_id.as_str()))
    }
}

#[cfg(target_os = "emscripten")]
pub use web::{read_save, write_save};

// Shared key/value primitives backed by the same JS storage shim as
// save data. Used by `settings.rs` to land on web localStorage with
// the same persistence semantics as `usagi.save` / `usagi.load`.
#[cfg(target_os = "emscripten")]
pub(crate) use web::{kv_read, kv_write};

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
    fn rejects_sparse_integer_keys() {
        // {[6]=1, [7]=2} used to surface as "invalid type: integer `6`,
        // expected a string key"; now we catch it up front with a
        // pointer at the workaround.
        let lua = Lua::new();
        let t: mlua::Table = lua.load(r#"return {[6]=1, [7]=2}"#).eval().unwrap();
        let err = lua_to_json(&lua, Value::Table(t)).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("1..n") && msg.contains("tostring"),
            "expected workaround hint, got: {msg}"
        );
    }

    #[test]
    fn rejects_array_with_gap_instead_of_silently_truncating() {
        // {[1]=x, [3]=z} used to serialize as `[x]` (mlua stops at the
        // Lua "border" — silent data loss). Now it errors so the dev
        // sees the problem before shipping.
        let lua = Lua::new();
        let t: mlua::Table = lua.load(r#"return {[1]="x", [3]="z"}"#).eval().unwrap();
        let err = lua_to_json(&lua, Value::Table(t)).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("1..n"),
            "expected dense-array hint, got: {msg}"
        );
    }

    #[test]
    fn rejects_mixed_string_and_integer_keys() {
        let lua = Lua::new();
        let t: mlua::Table = lua.load(r#"return {a=1, [1]="x"}"#).eval().unwrap();
        let err = lua_to_json(&lua, Value::Table(t)).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("mixes string and integer"),
            "expected mixed-keys hint, got: {msg}"
        );
    }

    #[test]
    fn rejects_sparse_keys_in_nested_table() {
        let lua = Lua::new();
        let t: mlua::Table = lua
            .load(r#"return { settings = { [6]=1, [7]=2 } }"#)
            .eval()
            .unwrap();
        let err = lua_to_json(&lua, Value::Table(t)).unwrap_err();
        assert!(err.to_string().contains("1..n"));
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
