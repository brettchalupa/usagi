//! Asset loading: Lua script, sprite sheet, and SFX. All loaders work
//! through the `VirtualFs` trait so they don't know or care whether the
//! bytes came from disk or from a compiled bundle.

use crate::vfs::VirtualFs;
use mlua::prelude::*;
use sola_raylib::prelude::*;
use std::collections::HashMap;
use std::time::SystemTime;

/// Executes the VFS's script on the given Lua VM. Redefines the
/// `_init` / `_update` / `_draw` globals each call; used for both initial
/// load and live reload.
pub fn load_script(lua: &Lua, vfs: &dyn VirtualFs) -> LuaResult<()> {
    let bytes = vfs
        .read_script()
        .ok_or_else(|| LuaError::RuntimeError("script not found".to_string()))?;
    lua.load(&bytes).set_name(vfs.script_name()).exec()
}

fn load_texture(rl: &mut RaylibHandle, thread: &RaylibThread, bytes: &[u8]) -> Option<Texture2D> {
    let image = Image::load_image_from_mem(".png", bytes)
        .map_err(|e| eprintln!("[usagi] failed to decode sprites.png: {e}"))
        .ok()?;
    rl.load_texture_from_image(thread, &image)
        .map_err(|e| eprintln!("[usagi] failed to upload sprite texture: {e}"))
        .ok()
}

/// Owns the sprite sheet texture and its mtime. `reload_if_changed` re-
/// reads from the vfs when the sprite file's mtime has moved (or always
/// no-ops on a bundle-backed vfs, whose mtimes are None).
pub struct SpriteSheet {
    pub texture: Option<Texture2D>,
    mtime: Option<SystemTime>,
}

impl SpriteSheet {
    pub fn load(rl: &mut RaylibHandle, thread: &RaylibThread, vfs: &dyn VirtualFs) -> Self {
        let texture = vfs
            .read_sprites()
            .and_then(|bytes| load_texture(rl, thread, &bytes));
        Self {
            texture,
            mtime: vfs.sprites_mtime(),
        }
    }

    /// Returns true if the sheet was reloaded this call.
    pub fn reload_if_changed(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        vfs: &dyn VirtualFs,
    ) -> bool {
        let new_mtime = vfs.sprites_mtime();
        if new_mtime == self.mtime {
            return false;
        }
        self.mtime = new_mtime;
        self.texture = vfs
            .read_sprites()
            .and_then(|bytes| load_texture(rl, thread, &bytes));
        true
    }

    pub fn texture(&self) -> Option<&Texture2D> {
        self.texture.as_ref()
    }
}

fn load_sound<'a>(audio: &'a RaylibAudio, stem: &str, bytes: &[u8]) -> Option<Sound<'a>> {
    let wave = audio
        .new_wave_from_memory(".wav", bytes)
        .map_err(|e| eprintln!("[usagi] failed to decode sfx '{stem}': {e}"))
        .ok()?;
    audio
        .new_sound_from_wave(&wave)
        .map_err(|e| eprintln!("[usagi] failed to create sfx '{stem}': {e}"))
        .ok()
}

/// Owns the loaded sounds + a manifest of their mtimes. `reload_if_changed`
/// rebuilds the whole library whenever the vfs's sfx manifest differs
/// from the one we loaded with. The lifetime is tied to `RaylibAudio`.
pub struct SfxLibrary<'a> {
    pub sounds: HashMap<String, Sound<'a>>,
    manifest: HashMap<String, SystemTime>,
}

impl<'a> SfxLibrary<'a> {
    pub fn empty() -> Self {
        Self {
            sounds: HashMap::new(),
            manifest: HashMap::new(),
        }
    }

    pub fn load(audio: &'a RaylibAudio, vfs: &dyn VirtualFs) -> Self {
        let mut sounds = HashMap::new();
        for stem in vfs.sfx_stems() {
            if let Some(bytes) = vfs.read_sfx(&stem)
                && let Some(sound) = load_sound(audio, &stem, &bytes)
            {
                sounds.insert(stem, sound);
            }
        }
        Self {
            sounds,
            manifest: vfs.sfx_manifest(),
        }
    }

    /// Returns true if the library was reloaded this call.
    pub fn reload_if_changed(&mut self, audio: &'a RaylibAudio, vfs: &dyn VirtualFs) -> bool {
        let new_manifest = vfs.sfx_manifest();
        if new_manifest == self.manifest {
            return false;
        }
        *self = Self::load(audio, vfs);
        true
    }

    pub fn play(&self, name: &str) {
        if let Some(sound) = self.sounds.get(name) {
            sound.play();
        }
    }

    pub fn len(&self) -> usize {
        self.sounds.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vfs::FsBacked;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn load_script_executes_and_sets_globals() {
        let lua = Lua::new();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("t.lua");
        fs::write(&path, "x = 42\nfunction _init() y = 99 end").unwrap();

        let vfs = FsBacked::from_script_path(&path);
        load_script(&lua, &vfs).unwrap();
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
        let vfs = FsBacked::from_script_path(&path);
        assert!(load_script(&lua, &vfs).is_err());
    }

    #[test]
    fn load_script_returns_err_on_missing_file() {
        let lua = Lua::new();
        let vfs = FsBacked::from_script_path(std::path::Path::new("/does/not/exist.lua"));
        assert!(load_script(&lua, &vfs).is_err());
    }

    /// Every `.lua` in `examples/` (including `<subdir>/main.lua`) must at
    /// least parse. Catches broken examples before `just example X` does.
    #[test]
    fn every_example_script_parses() {
        let lua = Lua::new();
        let examples_dir = std::path::Path::new("examples");
        assert!(
            examples_dir.is_dir(),
            "examples/ missing; test must run from repo root"
        );
        for entry in fs::read_dir(examples_dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                let main = path.join("main.lua");
                if main.is_file() {
                    parse_ok(&lua, &main);
                }
            } else if path.extension().and_then(|s| s.to_str()) == Some("lua") {
                parse_ok(&lua, &path);
            }
        }
    }

    fn parse_ok(lua: &Lua, path: &std::path::Path) {
        let src = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
        lua.load(&src)
            .set_name(path.to_str().unwrap())
            .into_function()
            .unwrap_or_else(|e| panic!("parse {path:?}: {e}"));
    }
}
