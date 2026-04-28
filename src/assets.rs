//! Asset loading: Lua script, sprite sheet, and SFX. All loaders work
//! through the `VirtualFs` trait so they don't know or care whether the
//! bytes came from disk or from a compiled bundle.

use crate::preprocess::preprocess;
use crate::vfs::VirtualFs;
use mlua::prelude::*;
use sola_raylib::prelude::*;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::SystemTime;

/// Executes the VFS's script on the given Lua VM. Redefines the
/// `_init` / `_update` / `_draw` globals each call; used for both initial
/// load and live reload.
pub fn load_script(lua: &Lua, vfs: &dyn VirtualFs) -> LuaResult<()> {
    let bytes = vfs
        .read_script()
        .ok_or_else(|| LuaError::RuntimeError("script not found".to_string()))?;
    let prepared = preprocess(&bytes);
    lua.load(&prepared).set_name(vfs.script_name()).exec()
}

/// Replaces `package.searchers` with `[preload, vfs]`. Stock Lua ships
/// four searchers — preload, the Lua loader (uses `package.path`), the C
/// loader, and an all-in-one — and we want only the first. Keeping the
/// preload searcher lets users inject test doubles via `package.preload`,
/// which is a Lua idiom worth preserving for free. Dropping the path-
/// based searchers means a running game can't read arbitrary `.lua` files
/// off cwd, which would silently work in `usagi dev` but fail in a fused
/// exe — better to fail the same way in both modes.
///
/// Called once at session init. Survives across live reloads (the Lua VM
/// is preserved; only the script is re-exec'd).
pub fn install_require(lua: &Lua, vfs: Rc<dyn VirtualFs>) -> LuaResult<()> {
    let package: LuaTable = lua.globals().get("package")?;
    let stock_searchers: LuaTable = package.get("searchers")?;
    // searchers[1] is the preload searcher in every Lua 5.2+ build.
    let preload_searcher: LuaValue = stock_searchers.get(1)?;

    let vfs_for_searcher = vfs.clone();
    let searcher = lua.create_function(move |lua, name: String| -> LuaResult<LuaMultiValue> {
        match vfs_for_searcher.read_module(&name) {
            Some((bytes, chunk_name)) => {
                // Preprocess once at searcher-time so the bytes captured
                // in the loader closure are already rewritten.
                let prepared = preprocess(&bytes);
                let chunk_name_for_loader = chunk_name.clone();
                let loader = lua.create_function(
                    move |lua, (modname, _chunk): (String, LuaValue)| -> LuaResult<LuaMultiValue> {
                        lua.load(prepared.as_slice())
                            .set_name(chunk_name_for_loader.as_str())
                            .call(modname)
                    },
                )?;
                Ok(LuaMultiValue::from_vec(vec![
                    LuaValue::Function(loader),
                    LuaValue::String(lua.create_string(&chunk_name)?),
                ]))
            }
            None => {
                let msg = format!("\n\tno module '{name}' in usagi vfs");
                Ok(LuaMultiValue::from_vec(vec![LuaValue::String(
                    lua.create_string(&msg)?,
                )]))
            }
        }
    })?;
    let new_searchers = lua.create_table()?;
    new_searchers.raw_push(preload_searcher)?;
    new_searchers.raw_push(searcher)?;
    package.set("searchers", new_searchers)?;

    Ok(())
}

/// Returns the newest mtime across `main.lua` and every currently-loaded
/// require'd module that resolves through the VFS. Used as the reload
/// trigger so saving any `.lua` file in the project (not just main.lua)
/// causes a reload — that's the engine's whole iteration story.
///
/// Modules that aren't yet `require`d don't appear in `package.loaded` and
/// so won't be tracked until main.lua first pulls them in. That's fine in
/// practice: a brand-new module nobody imports yet has no observable
/// effect on the running game.
pub fn freshest_lua_mtime(lua: &Lua, vfs: &dyn VirtualFs) -> Option<SystemTime> {
    let mut newest = vfs.script_mtime();
    let Ok(package) = lua.globals().get::<LuaTable>("package") else {
        return newest;
    };
    let Ok(loaded) = package.get::<LuaTable>("loaded") else {
        return newest;
    };
    for pair in loaded.pairs::<String, LuaValue>() {
        let Ok((key, _)) = pair else { continue };
        if let Some(t) = vfs.module_mtime(&key) {
            newest = match newest {
                Some(n) => Some(n.max(t)),
                None => Some(t),
            };
        }
    }
    newest
}

/// Drops every `package.loaded` entry that resolves through the VFS. Built-
/// in libraries (`string`, `math`, `table`, etc.) are left alone because
/// the VFS doesn't claim them. Called on script reload so a saved edit to
/// any `require`d module is picked up the next time `main.lua` runs.
///
/// Uses `module_mtime` rather than `read_module` for the membership test
/// — a stat per loaded module beats a full file read per loaded module
/// when reload fires (which is potentially every saved keystroke).
pub fn clear_user_modules(lua: &Lua, vfs: &dyn VirtualFs) -> LuaResult<()> {
    let package: LuaTable = lua.globals().get("package")?;
    let loaded: LuaTable = package.get("loaded")?;
    let mut to_remove: Vec<String> = Vec::new();
    for pair in loaded.pairs::<String, LuaValue>() {
        let (key, _) = pair?;
        if vfs.module_mtime(&key).is_some() {
            to_remove.push(key);
        }
    }
    for key in to_remove {
        loaded.set(key, LuaValue::Nil)?;
    }
    Ok(())
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

/// Owns the loaded music streams and tracks which one (if any) is
/// currently playing. raylib's music streams are decoded incrementally,
/// so `update` MUST run every frame to refill the audio buffer — the
/// session loop calls it from `frame()`. Lifetime is tied to
/// `RaylibAudio`.
pub struct MusicLibrary<'a> {
    tracks: HashMap<String, Music<'a>>,
    manifest: HashMap<String, SystemTime>,
    /// Stem of the track that's currently playing, if any. Used by
    /// `update` to know which stream to refill, and by `play` / `loop_`
    /// to know what to stop before starting a new one.
    current: Option<String>,
}

impl<'a> MusicLibrary<'a> {
    pub fn empty() -> Self {
        Self {
            tracks: HashMap::new(),
            manifest: HashMap::new(),
            current: None,
        }
    }

    pub fn load(audio: &'a RaylibAudio, vfs: &dyn VirtualFs) -> Self {
        let mut tracks = HashMap::new();
        for (stem, ext) in vfs.music_entries() {
            let Some(bytes) = vfs.read_music(&stem, &ext) else {
                continue;
            };
            // raylib's `LoadMusicStreamFromMemory` stores a raw pointer
            // to the input buffer (stb_vorbis_open_memory, dr_mp3,
            // dr_flac all do this) and reads from it on every
            // `UpdateMusicStream` call. It does not copy the data
            // up front. Dropping `bytes` here would dangle that
            // pointer, the decoder would return 0 frames forever, and
            // raylib's refill loop would spin holding the audio mutex.
            // Leak to give the bytes program lifetime; the data would
            // have lived this long anyway since music plays for the
            // lifetime of the session.
            let leaked: &'static [u8] = Box::leak(bytes.into_boxed_slice());
            let type_tag = format!(".{ext}");
            match audio.new_music_from_memory(&type_tag, leaked) {
                Ok(music) => {
                    tracks.insert(stem, music);
                }
                Err(e) => eprintln!("[usagi] failed to load music '{stem}.{ext}': {e}"),
            }
        }
        Self {
            tracks,
            manifest: vfs.music_manifest(),
            current: None,
        }
    }

    /// Returns true if the library was rebuilt this call. Stops any
    /// currently-playing track on rebuild — its `Music` handle is
    /// about to be dropped.
    pub fn reload_if_changed(&mut self, audio: &'a RaylibAudio, vfs: &dyn VirtualFs) -> bool {
        let new_manifest = vfs.music_manifest();
        if new_manifest == self.manifest {
            return false;
        }
        // Drop current first so the underlying Music value unloads
        // cleanly before we replace the library.
        self.current = None;
        *self = Self::load(audio, vfs);
        true
    }

    /// Plays `name` once. If another track is playing it stops first.
    /// Unknown names silently no-op, matching `sfx.play`.
    pub fn play(&mut self, name: &str) {
        self.start(name, false);
    }

    /// Plays `name` and loops it forever. If another track is playing
    /// it stops first.
    pub fn loop_(&mut self, name: &str) {
        self.start(name, true);
    }

    fn start(&mut self, name: &str, looping: bool) {
        if !self.tracks.contains_key(name) {
            return;
        }
        // Stop whatever was playing — even if it's the same track
        // the user is asking to (re)start.
        if let Some(current) = self.current.take()
            && let Some(track) = self.tracks.get(&current)
        {
            track.stop_stream();
        }
        let Some(track) = self.tracks.get_mut(name) else {
            return;
        };
        // raylib has no `SetMusicLooping` function — the `looping`
        // field on the Music struct is read directly at end-of-stream.
        // sola-raylib's `Music` derefs to `ffi::Music` via `AsMut`.
        track.as_mut().looping = looping;
        track.play_stream();
        self.current = Some(name.to_string());
    }

    pub fn stop(&mut self) {
        if let Some(current) = self.current.take()
            && let Some(track) = self.tracks.get(&current)
        {
            track.stop_stream();
        }
    }

    /// Drives the active stream's audio buffer. raylib needs this
    /// every frame or playback drops out; cheap no-op when nothing's
    /// playing.
    pub fn update(&mut self) {
        let Some(name) = self.current.as_deref() else {
            return;
        };
        let Some(track) = self.tracks.get_mut(name) else {
            return;
        };
        track.update_stream();
    }

    pub fn len(&self) -> usize {
        self.tracks.len()
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
    fn load_script_applies_compound_op_preprocessor() {
        // End-to-end check: a script using `+=` parses+runs because the
        // preprocessor rewrites it before `lua.load`.
        let lua = Lua::new();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("ops.lua");
        fs::write(&path, "x = 0\nx += 1\nx += 2\ny = 10\ny *= 3\n").unwrap();
        let vfs = FsBacked::from_script_path(&path);
        load_script(&lua, &vfs).unwrap();
        assert_eq!(lua.globals().get::<i32>("x").unwrap(), 3);
        assert_eq!(lua.globals().get::<i32>("y").unwrap(), 30);
    }

    #[test]
    fn require_loader_applies_compound_op_preprocessor() {
        // Same as above but for `require`d modules: the preprocessor
        // must run before the searcher-side `lua.load` too, otherwise
        // compound ops would only work in main.lua.
        let lua = Lua::new();
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::write(
            root.join("main.lua"),
            "local m = require 'mod'; result = m.go()",
        )
        .unwrap();
        fs::write(
            root.join("mod.lua"),
            "local M = {}\nfunction M.go()\n  local n = 5\n  n += 7\n  return n\nend\nreturn M\n",
        )
        .unwrap();
        let vfs: Rc<dyn VirtualFs> = Rc::new(FsBacked::from_script_path(&root.join("main.lua")));
        install_require(&lua, vfs.clone()).unwrap();
        load_script(&lua, vfs.as_ref()).unwrap();
        assert_eq!(lua.globals().get::<i32>("result").unwrap(), 12);
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
        let src = fs::read(path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
        // Examples may use compound operators; the runtime applies the
        // preprocessor before `lua.load`, so the parse test must too.
        let prepared = preprocess(&src);
        lua.load(prepared.as_slice())
            .set_name(path.to_str().unwrap())
            .into_function()
            .unwrap_or_else(|e| panic!("parse {path:?}: {e}"));
    }

    #[test]
    fn require_resolves_module_from_vfs() {
        let lua = Lua::new();
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::write(
            root.join("main.lua"),
            "local m = require 'enemies'; result = m.count()",
        )
        .unwrap();
        fs::write(
            root.join("enemies.lua"),
            "local M = {}\nfunction M.count() return 7 end\nreturn M",
        )
        .unwrap();
        let vfs: Rc<dyn VirtualFs> = Rc::new(FsBacked::from_script_path(&root.join("main.lua")));
        install_require(&lua, vfs.clone()).unwrap();
        load_script(&lua, vfs.as_ref()).unwrap();
        assert_eq!(lua.globals().get::<i32>("result").unwrap(), 7);
    }

    #[test]
    fn require_caches_module_across_calls() {
        let lua = Lua::new();
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::write(root.join("main.lua"), "").unwrap();
        // Module body bumps a global on each execution; cached require
        // means it should run exactly once even if required twice.
        fs::write(
            root.join("counter.lua"),
            "load_count = (load_count or 0) + 1\nreturn { n = load_count }",
        )
        .unwrap();
        let vfs: Rc<dyn VirtualFs> = Rc::new(FsBacked::from_script_path(&root.join("main.lua")));
        install_require(&lua, vfs.clone()).unwrap();
        lua.load("local a = require 'counter'; local b = require 'counter'; same = a == b")
            .exec()
            .unwrap();
        assert!(lua.globals().get::<bool>("same").unwrap());
        assert_eq!(lua.globals().get::<i32>("load_count").unwrap(), 1);
    }

    #[test]
    fn clear_user_modules_drops_vfs_entries_only() {
        let lua = Lua::new();
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::write(root.join("main.lua"), "").unwrap();
        fs::write(root.join("data.lua"), "return { v = 1 }").unwrap();
        let vfs: Rc<dyn VirtualFs> = Rc::new(FsBacked::from_script_path(&root.join("main.lua")));
        install_require(&lua, vfs.clone()).unwrap();
        // Touch both a VFS module and a built-in lib so we can confirm
        // only the VFS one is cleared.
        lua.load("require 'data'; require 'string'").exec().unwrap();
        clear_user_modules(&lua, vfs.as_ref()).unwrap();
        let loaded: LuaTable = lua
            .globals()
            .get::<LuaTable>("package")
            .unwrap()
            .get("loaded")
            .unwrap();
        assert!(loaded.get::<LuaValue>("data").unwrap().is_nil());
        assert!(!loaded.get::<LuaValue>("string").unwrap().is_nil());
    }

    #[test]
    fn freshest_mtime_tracks_required_modules_not_just_main() {
        let lua = Lua::new();
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::write(root.join("main.lua"), "require 'helper'").unwrap();
        fs::write(root.join("helper.lua"), "return {}").unwrap();
        let vfs: Rc<dyn VirtualFs> = Rc::new(FsBacked::from_script_path(&root.join("main.lua")));
        install_require(&lua, vfs.clone()).unwrap();
        load_script(&lua, vfs.as_ref()).unwrap();
        let baseline = freshest_lua_mtime(&lua, vfs.as_ref()).expect("have an mtime baseline");

        // Bump helper.lua's mtime to a known-later instant. `set_modified`
        // requires a write-capable handle on Windows (FILE_WRITE_ATTRIBUTES
        // permission); a plain `File::open` is read-only and fails with
        // "Access is denied." Use OpenOptions with write to portably
        // get the right access bits.
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
        std::fs::OpenOptions::new()
            .write(true)
            .open(root.join("helper.lua"))
            .unwrap()
            .set_modified(later)
            .unwrap();

        let after = freshest_lua_mtime(&lua, vfs.as_ref()).expect("still have an mtime");
        assert!(
            after > baseline,
            "editing helper.lua must move the freshest mtime forward (baseline={baseline:?}, after={after:?})"
        );
    }

    #[test]
    fn install_require_preserves_package_preload_searcher() {
        // package.preload injection is the standard Lua idiom for stubbing
        // a module from outside its file (tests, mocks, dynamic content).
        // Replacing package.searchers must not blow it away.
        let lua = Lua::new();
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("main.lua"), "").unwrap();
        let vfs: Rc<dyn VirtualFs> =
            Rc::new(FsBacked::from_script_path(&dir.path().join("main.lua")));
        install_require(&lua, vfs).unwrap();
        lua.load(
            r#"
            package.preload["injected"] = function() return { tag = "preload" } end
            local m = require "injected"
            tag = m.tag
        "#,
        )
        .exec()
        .unwrap();
        assert_eq!(lua.globals().get::<String>("tag").unwrap(), "preload");
    }

    #[test]
    fn require_unknown_module_errors_with_helpful_message() {
        let lua = Lua::new();
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("main.lua"), "").unwrap();
        let vfs: Rc<dyn VirtualFs> =
            Rc::new(FsBacked::from_script_path(&dir.path().join("main.lua")));
        install_require(&lua, vfs).unwrap();
        let err = lua
            .load("require 'nope'")
            .exec()
            .expect_err("require of missing module must error");
        assert!(
            err.to_string().contains("nope"),
            "expected module name in error, got: {err}"
        );
    }
}
