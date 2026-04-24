//! Asset loading: Lua script, sprite sheet, and SFX.

use mlua::prelude::*;
use sola_raylib::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use std::time::SystemTime;

/// Reads the script file and executes it on the given Lua VM, redefining
/// the `_init` / `_update` / `_draw` globals. Used for both initial load
/// and live reload.
pub fn load_script(lua: &Lua, path: &str) -> LuaResult<()> {
    let source = std::fs::read_to_string(path).map_err(LuaError::external)?;
    lua.load(&source).set_name(path).exec()
}

/// Tries to load the sprite sheet (sprites.png next to the script). Returns
/// None on any failure. Missing file is not an error; a decode failure
/// prints to stderr.
pub fn load_sprites(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    path: &Path,
) -> Option<Texture2D> {
    if !path.exists() {
        return None;
    }
    let path_str = path.to_str()?;
    match rl.load_texture(thread, path_str) {
        Ok(tex) => Some(tex),
        Err(e) => {
            eprintln!("[usagi] failed to load sprites {}: {}", path.display(), e);
            None
        }
    }
}

/// Scans `<dir>` for .wav files and returns a manifest of stem to mtime.
/// Used to detect when sfx need reloading (file added, removed, or edited).
pub fn scan_sfx(dir: &Path) -> HashMap<String, SystemTime> {
    let mut out = HashMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wav") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(mtime) = entry.metadata().and_then(|m| m.modified()) else {
            continue;
        };
        out.insert(stem.to_string(), mtime);
    }
    out
}

/// Loads all .wav files in `<dir>` into a name-to-Sound map, keyed by file
/// stem (e.g. `sfx/jump.wav` -> "jump"). Individual decode failures log to
/// stderr; the rest still load.
pub fn load_sfx<'a>(audio: &'a RaylibAudio, dir: &Path) -> HashMap<String, Sound<'a>> {
    let mut sounds = HashMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return sounds;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wav") {
            continue;
        }
        let (Some(stem), Some(path_str)) =
            (path.file_stem().and_then(|s| s.to_str()), path.to_str())
        else {
            continue;
        };
        match audio.new_sound(path_str) {
            Ok(sound) => {
                sounds.insert(stem.to_string(), sound);
            }
            Err(e) => eprintln!("[usagi] failed to load sfx {}: {}", path.display(), e),
        }
    }
    sounds
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn scan_sfx_finds_wav_files() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("jump.wav"), b"fake").unwrap();
        fs::write(dir.path().join("coin.wav"), b"fake").unwrap();
        let manifest = scan_sfx(dir.path());
        assert!(manifest.contains_key("jump"));
        assert!(manifest.contains_key("coin"));
        assert_eq!(manifest.len(), 2);
    }

    #[test]
    fn scan_sfx_ignores_non_wav() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("jump.wav"), b"fake").unwrap();
        fs::write(dir.path().join("readme.txt"), b"hi").unwrap();
        fs::write(dir.path().join("bgm.ogg"), b"fake").unwrap();
        let manifest = scan_sfx(dir.path());
        assert_eq!(manifest.len(), 1);
        assert!(manifest.contains_key("jump"));
    }

    #[test]
    fn scan_sfx_missing_dir_returns_empty() {
        let manifest = scan_sfx(Path::new("/does/not/exist/at/all"));
        assert!(manifest.is_empty());
    }

    #[test]
    fn load_script_executes_and_sets_globals() {
        let lua = Lua::new();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("t.lua");
        fs::write(&path, "x = 42\nfunction _init() y = 99 end").unwrap();

        load_script(&lua, path.to_str().unwrap()).unwrap();
        let x: i32 = lua.globals().get("x").unwrap();
        assert_eq!(x, 42);
        let init: LuaFunction = lua.globals().get("_init").unwrap();
        init.call::<()>(()).unwrap();
        let y: i32 = lua.globals().get("y").unwrap();
        assert_eq!(y, 99);
    }

    #[test]
    fn load_script_returns_err_on_syntax_error() {
        let lua = Lua::new();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("broken.lua");
        fs::write(&path, "function _update(dt)").unwrap(); // missing end
        assert!(load_script(&lua, path.to_str().unwrap()).is_err());
    }

    #[test]
    fn load_script_returns_err_on_missing_file() {
        let lua = Lua::new();
        assert!(load_script(&lua, "/does/not/exist.lua").is_err());
    }
}
