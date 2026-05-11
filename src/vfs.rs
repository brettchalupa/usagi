//! Virtual filesystem abstraction so session/tools can read assets from
//! either the real filesystem (dev/run modes) or an in-memory bundle
//! (a fused, compiled game). The trait surface is intentionally narrow —
//! just the three asset types Usagi knows about.

use crate::bundle::Bundle;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub trait VirtualFs {
    /// A name for the script used in Lua stack traces and error messages.
    fn script_name(&self) -> String;
    fn read_script(&self) -> Option<Vec<u8>>;

    fn read_sprites(&self) -> Option<Vec<u8>>;
    fn sprites_mtime(&self) -> Option<SystemTime>;

    /// Optional `palette.png` at project root (or bundled). When
    /// present, overrides the default Pico-8 palette. Format: exactly
    /// 1px tall, one color per pixel (`crate::palette::Palette::from_image_bytes`
    /// enforces this). Missing file is fine — engine falls back to
    /// the Pico-8 default.
    fn read_palette(&self) -> Option<Vec<u8>>;
    fn palette_mtime(&self) -> Option<SystemTime>;

    fn sfx_stems(&self) -> Vec<String>;
    fn read_sfx(&self, stem: &str) -> Option<Vec<u8>>;
    fn sfx_manifest(&self) -> HashMap<String, SystemTime>;

    /// Returns `(stem, ext)` for every recognized music file in the
    /// project's `music/` dir. ext is the lower-cased extension
    /// without the dot — the format raylib's `LoadMusicStreamFromMemory`
    /// expects (e.g. `("invincible", "ogg")`).
    fn music_entries(&self) -> Vec<(String, String)>;
    fn read_music(&self, stem: &str, ext: &str) -> Option<Vec<u8>>;
    fn music_manifest(&self) -> HashMap<String, SystemTime>;

    /// Resolves a Lua module name (e.g. `"enemies"` or `"world.tiles"`) to
    /// `(bytes, chunk_name)`. Tries `name.lua` first, then `name/init.lua`,
    /// matching stock Lua's `?.lua;?/init.lua` convention. The chunk name is
    /// passed to `lua.load().set_name()` so stack traces point at a useful
    /// path. Returns None when the module name is invalid (path traversal,
    /// empty segments) or no matching file exists.
    fn read_module(&self, mod_name: &str) -> Option<(Vec<u8>, String)>;

    /// Returns the mtime of whichever file `read_module` would have read,
    /// or None if the module isn't resolvable or the backend has no
    /// notion of mtimes (bundled games). Used by the live-reload watcher
    /// so any saved `.lua` file in the project triggers a reload, not
    /// just `main.lua`.
    fn module_mtime(&self, _mod_name: &str) -> Option<SystemTime> {
        None
    }

    /// Reads an arbitrary file from the project root by relative path
    /// (`/`-separated). Used by the shader loader, which wants generic
    /// access to `shaders/<name>.fs` etc. without growing a dedicated
    /// per-asset method. Path traversal segments are rejected.
    fn read_file(&self, rel: &str) -> Option<Vec<u8>>;
    /// Mtime for the same path. None when the backend has no mtimes
    /// (bundle) or the file is missing.
    fn file_mtime(&self, _rel: &str) -> Option<SystemTime> {
        None
    }

    /// Newest mtime across every `.lua` file under the project root.
    /// Used by the live-reload watcher as the change-detection signal.
    /// Walking the project tree (rather than only `package.loaded`) means
    /// a module that fell out of `package.loaded` because a previous
    /// reload errored mid-`require` is still tracked, so saving the fix
    /// re-triggers reload. `BundleBacked` returns None (bundles don't
    /// reload).
    fn freshest_lua_mtime(&self) -> Option<SystemTime> {
        None
    }

    /// Whether filesystem reload checks are meaningful on this vfs.
    /// `FsBacked` returns true; `BundleBacked` always returns false.
    fn supports_reload(&self) -> bool;

    /// Best-effort project-name hint, used by `game_id::resolve` as a
    /// fallback when `_config().game_id` is unset. `FsBacked` returns the
    /// project's directory or script-stem name; `BundleBacked` returns
    /// None because in-memory bundles carry no original name. None at
    /// runtime is fine: the resolver drops to a bundle-hash fallback.
    fn project_name_hint(&self) -> Option<String> {
        None
    }

    /// Borrow the bundle that backs this vfs, if any. `BundleBacked`
    /// returns Some so `game_id::resolve` can hash the bundle as a
    /// last-resort fallback; `FsBacked` returns None because the project
    /// lives as loose files (and the directory name covers the fallback
    /// case there).
    fn as_bundle(&self) -> Option<&Bundle> {
        None
    }
}

/// True when a `.lua` file is annotated as type-stubs-only via the
/// lua-language-server `---@meta` marker. Such files declare globals
/// like `gfx = {}` purely so the LSP can autocomplete them; executing
/// one at runtime would clobber the engine's real tables. Used to
/// exclude meta files from both the bundle walk and the `require`
/// searcher, so projects bootstrapped by `usagi init` (which ships
/// `meta/usagi.lua`) don't accidentally ship or import their stubs.
pub(crate) fn is_meta_chunk(bytes: &[u8]) -> bool {
    // The marker, when present, is on the first non-blank line. Scan a
    // small prefix so we don't pay a UTF-8 conversion on whole files.
    let head = &bytes[..bytes.len().min(256)];
    let s = std::str::from_utf8(head).unwrap_or("");
    for line in s.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        return trimmed.starts_with("---@meta");
    }
    false
}

/// File extensions accepted in the `music/` directory. raylib's
/// `LoadMusicStreamFromMemory` reads the type tag (".ogg", ".mp3", etc.)
/// to pick a decoder. OGG is the safest format for cross-platform
/// shipping since the emscripten build explicitly enables `USE_OGG=1`
/// and `USE_VORBIS=1`; the others rely on raylib's bundled header-only
/// parsers (dr_wav, dr_mp3, dr_flac) which do work on web but get less
/// testing in the emscripten path.
pub(crate) const MUSIC_EXTS: &[&str] = &["ogg", "mp3", "wav", "flac"];

/// True for paths that are safe to read off the project root: no
/// absolute prefix, no `.`/`..` segments, no backslashes, no empty
/// segments. Shared between `FsBacked::read_file` and
/// `BundleBacked::read_file` so the two backends accept the same
/// vocabulary.
fn safe_rel_path(rel: &str) -> bool {
    if rel.is_empty() || rel.contains('\\') {
        return false;
    }
    if rel.starts_with('/') {
        return false;
    }
    rel.split('/')
        .all(|seg| !seg.is_empty() && seg != "." && seg != "..")
}

/// Translates a dotted Lua module name into the relative paths that should
/// be checked, in order. Returns None for names that contain path
/// separators, leading/trailing dots, empty segments, or `..`/`.` segments
/// — anything that would let a bad require escape the project root or
/// land somewhere unexpected.
fn module_candidates(name: &str) -> Option<Vec<String>> {
    if name.is_empty() {
        return None;
    }
    if name.contains(['/', '\\']) {
        return None;
    }
    let rel: String = name.replace('.', "/");
    if rel
        .split('/')
        .any(|seg| seg.is_empty() || seg == "." || seg == "..")
    {
        return None;
    }
    Some(vec![format!("{rel}.lua"), format!("{rel}/init.lua")])
}

/// Recursively visits every `*.lua` file under `root`. Skips
/// dotfiles / dot-directories. Read errors are swallowed (best-effort
/// walk) since the call sites (live-reload watcher, bundle exporter)
/// don't want to abort on a transient permission blip.
pub(crate) fn for_each_lua_file<F: FnMut(&std::fs::DirEntry)>(root: &Path, mut visit: F) {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if name.starts_with('.') {
                continue;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                stack.push(p);
                continue;
            }
            if p.extension().and_then(|e| e.to_str()) != Some("lua") {
                continue;
            }
            visit(&entry);
        }
    }
}

/// Newest mtime across reload-relevant `.lua` files under `root`. The
/// `meta/usagi.lua` LSP stub `usagi init` ships is excluded so editing
/// it doesn't trigger a no-op reload (it's never executable per
/// `is_meta_chunk`). Skip is name-specific, not directory-wide.
fn freshest_lua_mtime_under(root: &Path) -> Option<SystemTime> {
    let usagi_meta_stub = Path::new("meta").join("usagi.lua");
    let mut newest: Option<SystemTime> = None;
    for_each_lua_file(root, |entry| {
        let p = entry.path();
        if p.strip_prefix(root)
            .map(|rel| rel == usagi_meta_stub)
            .unwrap_or(false)
        {
            return;
        }
        let Ok(t) = entry.metadata().and_then(|m| m.modified()) else {
            return;
        };
        newest = Some(match newest {
            Some(prev) => prev.max(t),
            None => t,
        });
    });
    newest
}

/// Disk-backed vfs. `root` is the directory that holds `sprites.png` and
/// `sfx/`. `script_filename` is the main Lua file inside `root` (None when
/// the vfs is used purely for asset browsing, e.g. the tools window).
#[derive(Clone)]
pub struct FsBacked {
    root: PathBuf,
    script_filename: Option<String>,
}

impl FsBacked {
    pub fn from_script_path(script_path: &Path) -> Self {
        // `Path::parent` of a bare filename like `main.lua` returns
        // `Some("")`, not `None` — and an empty path can't be
        // `read_dir`'d, so live-reload's `freshest_lua_mtime` walk
        // silently failed. Treat empty (and absent) parents as ".".
        let root = match script_path.parent() {
            Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
            _ => PathBuf::from("."),
        };
        let script_filename = script_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(String::from);
        Self {
            root,
            script_filename,
        }
    }

    pub fn from_project_dir(root: PathBuf) -> Self {
        Self {
            root,
            script_filename: None,
        }
    }

    fn script_path(&self) -> Option<PathBuf> {
        self.script_filename.as_deref().map(|n| self.root.join(n))
    }

    fn sprites_path(&self) -> PathBuf {
        self.root.join("sprites.png")
    }

    fn palette_path(&self) -> PathBuf {
        self.root.join("palette.png")
    }

    fn sfx_dir(&self) -> PathBuf {
        self.root.join("sfx")
    }

    fn music_dir(&self) -> PathBuf {
        self.root.join("music")
    }
}

impl VirtualFs for FsBacked {
    fn script_name(&self) -> String {
        self.script_path()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "<no script>".to_string())
    }

    fn read_script(&self) -> Option<Vec<u8>> {
        std::fs::read(self.script_path()?).ok()
    }

    fn read_sprites(&self) -> Option<Vec<u8>> {
        std::fs::read(self.sprites_path()).ok()
    }

    fn sprites_mtime(&self) -> Option<SystemTime> {
        std::fs::metadata(self.sprites_path())
            .and_then(|m| m.modified())
            .ok()
    }

    fn read_palette(&self) -> Option<Vec<u8>> {
        std::fs::read(self.palette_path()).ok()
    }

    fn palette_mtime(&self) -> Option<SystemTime> {
        std::fs::metadata(self.palette_path())
            .and_then(|m| m.modified())
            .ok()
    }

    fn sfx_stems(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(self.sfx_dir()) else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if p.extension().and_then(|s| s.to_str()) != Some("wav") {
                    return None;
                }
                p.file_stem().and_then(|s| s.to_str()).map(String::from)
            })
            .collect()
    }

    fn read_sfx(&self, stem: &str) -> Option<Vec<u8>> {
        std::fs::read(self.sfx_dir().join(format!("{stem}.wav"))).ok()
    }

    fn sfx_manifest(&self) -> HashMap<String, SystemTime> {
        let Ok(entries) = std::fs::read_dir(self.sfx_dir()) else {
            return HashMap::new();
        };
        let mut out = HashMap::new();
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|s| s.to_str()) != Some("wav") {
                continue;
            }
            let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Ok(mtime) = entry.metadata().and_then(|m| m.modified()) else {
                continue;
            };
            out.insert(stem.to_string(), mtime);
        }
        out
    }

    fn music_entries(&self) -> Vec<(String, String)> {
        let Ok(entries) = std::fs::read_dir(self.music_dir()) else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                let ext = p
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(str::to_ascii_lowercase)?;
                if !MUSIC_EXTS.contains(&ext.as_str()) {
                    return None;
                }
                let stem = p.file_stem().and_then(|s| s.to_str())?.to_string();
                Some((stem, ext))
            })
            .collect()
    }

    fn read_music(&self, stem: &str, ext: &str) -> Option<Vec<u8>> {
        std::fs::read(self.music_dir().join(format!("{stem}.{ext}"))).ok()
    }

    fn music_manifest(&self) -> HashMap<String, SystemTime> {
        let Ok(entries) = std::fs::read_dir(self.music_dir()) else {
            return HashMap::new();
        };
        let mut out = HashMap::new();
        for entry in entries.flatten() {
            let p = entry.path();
            let Some(ext) = p
                .extension()
                .and_then(|s| s.to_str())
                .map(str::to_ascii_lowercase)
            else {
                continue;
            };
            if !MUSIC_EXTS.contains(&ext.as_str()) {
                continue;
            }
            let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Ok(mtime) = entry.metadata().and_then(|m| m.modified()) else {
                continue;
            };
            out.insert(stem.to_string(), mtime);
        }
        out
    }

    fn read_module(&self, mod_name: &str) -> Option<(Vec<u8>, String)> {
        let candidates = module_candidates(mod_name)?;
        for rel in candidates {
            let full = self.root.join(&rel);
            if let Ok(bytes) = std::fs::read(&full) {
                if is_meta_chunk(&bytes) {
                    // LSP type-stub file: visible to the language server
                    // but never executable. Refuse the require so the
                    // searcher chain (and its error message) is honest.
                    return None;
                }
                return Some((bytes, full.to_string_lossy().into_owned()));
            }
        }
        None
    }

    fn module_mtime(&self, mod_name: &str) -> Option<SystemTime> {
        let candidates = module_candidates(mod_name)?;
        for rel in candidates {
            let full = self.root.join(&rel);
            if let Ok(t) = std::fs::metadata(&full).and_then(|m| m.modified()) {
                return Some(t);
            }
        }
        None
    }

    fn read_file(&self, rel: &str) -> Option<Vec<u8>> {
        if !safe_rel_path(rel) {
            return None;
        }
        std::fs::read(self.root.join(rel)).ok()
    }

    fn file_mtime(&self, rel: &str) -> Option<SystemTime> {
        if !safe_rel_path(rel) {
            return None;
        }
        std::fs::metadata(self.root.join(rel))
            .and_then(|m| m.modified())
            .ok()
    }

    fn supports_reload(&self) -> bool {
        true
    }

    fn freshest_lua_mtime(&self) -> Option<SystemTime> {
        freshest_lua_mtime_under(&self.root)
    }

    fn project_name_hint(&self) -> Option<String> {
        // Mirror `export::project_name`: when the script is `main.lua` the
        // parent directory's name is the project; otherwise the file stem.
        // Keeping the two in sync means a runtime save and an export's
        // CFBundleIdentifier line up on the same name fallback.
        if let Some(name) = self.script_filename.as_deref() {
            let stem = std::path::Path::new(name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("game");
            if stem == "main" {
                self.root
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(String::from)
            } else {
                Some(stem.to_string())
            }
        } else {
            self.root
                .file_name()
                .and_then(|s| s.to_str())
                .map(String::from)
        }
    }
}

/// Bundle-backed vfs. All reads go against the in-memory bundle. Mtimes
/// are always None, so reload-if-changed checks no-op.
pub struct BundleBacked {
    bundle: Bundle,
}

impl BundleBacked {
    pub fn new(bundle: Bundle) -> Self {
        Self { bundle }
    }
}

impl VirtualFs for BundleBacked {
    fn script_name(&self) -> String {
        "main.lua".to_string()
    }

    fn read_script(&self) -> Option<Vec<u8>> {
        self.bundle.get("main.lua").map(<[u8]>::to_vec)
    }

    fn read_sprites(&self) -> Option<Vec<u8>> {
        self.bundle.get("sprites.png").map(<[u8]>::to_vec)
    }

    fn sprites_mtime(&self) -> Option<SystemTime> {
        None
    }

    fn read_palette(&self) -> Option<Vec<u8>> {
        self.bundle.get("palette.png").map(<[u8]>::to_vec)
    }

    fn palette_mtime(&self) -> Option<SystemTime> {
        None
    }

    fn sfx_stems(&self) -> Vec<String> {
        self.bundle
            .names()
            .filter_map(|name| {
                name.strip_prefix("sfx/")
                    .and_then(|f| f.strip_suffix(".wav"))
                    .map(String::from)
            })
            .collect()
    }

    fn read_sfx(&self, stem: &str) -> Option<Vec<u8>> {
        self.bundle
            .get(&format!("sfx/{stem}.wav"))
            .map(<[u8]>::to_vec)
    }

    fn sfx_manifest(&self) -> HashMap<String, SystemTime> {
        HashMap::new()
    }

    fn music_entries(&self) -> Vec<(String, String)> {
        self.bundle
            .names()
            .filter_map(|name| {
                let rel = name.strip_prefix("music/")?;
                let dot = rel.rfind('.')?;
                let stem = &rel[..dot];
                let ext = rel[dot + 1..].to_ascii_lowercase();
                if !MUSIC_EXTS.contains(&ext.as_str()) {
                    return None;
                }
                Some((stem.to_string(), ext))
            })
            .collect()
    }

    fn read_music(&self, stem: &str, ext: &str) -> Option<Vec<u8>> {
        self.bundle
            .get(&format!("music/{stem}.{ext}"))
            .map(<[u8]>::to_vec)
    }

    fn music_manifest(&self) -> HashMap<String, SystemTime> {
        HashMap::new()
    }

    fn read_module(&self, mod_name: &str) -> Option<(Vec<u8>, String)> {
        let candidates = module_candidates(mod_name)?;
        for rel in candidates {
            if let Some(bytes) = self.bundle.get(&rel) {
                return Some((bytes.to_vec(), rel));
            }
        }
        None
    }

    fn read_file(&self, rel: &str) -> Option<Vec<u8>> {
        if !safe_rel_path(rel) {
            return None;
        }
        self.bundle.get(rel).map(<[u8]>::to_vec)
    }

    fn supports_reload(&self) -> bool {
        false
    }

    fn as_bundle(&self) -> Option<&Bundle> {
        Some(&self.bundle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn fs_backed_reads_script_and_supports_reload() {
        let dir = TempDir::new().unwrap();
        let script = dir.path().join("game.lua");
        fs::write(&script, b"-- hello").unwrap();
        let vfs = FsBacked::from_script_path(&script);
        assert_eq!(vfs.read_script().as_deref(), Some(b"-- hello".as_slice()));
        assert!(vfs.supports_reload());
    }

    /// `usagi dev main.lua` (bare filename, no directory component) used
    /// to silently break live reload because `Path::parent` returns
    /// `Some("")` for bare names — not `None` — so the empty-path root
    /// failed every `read_dir` and `freshest_lua_mtime` returned `None`.
    /// Treat empty parents as "." so the watcher walks the CWD.
    #[test]
    fn from_script_path_bare_filename_uses_dot_as_root() {
        let vfs = FsBacked::from_script_path(Path::new("main.lua"));
        assert_eq!(vfs.root, PathBuf::from("."));
    }

    #[test]
    fn fs_backed_missing_sprites_returns_none() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("g.lua"), b"").unwrap();
        let vfs = FsBacked::from_script_path(&dir.path().join("g.lua"));
        assert!(vfs.read_sprites().is_none());
        assert!(vfs.sprites_mtime().is_none());
    }

    #[test]
    fn fs_backed_lists_sfx_stems() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::write(root.join("main.lua"), b"").unwrap();
        fs::create_dir(root.join("sfx")).unwrap();
        fs::write(root.join("sfx/jump.wav"), b"wav").unwrap();
        fs::write(root.join("sfx/coin.wav"), b"wav").unwrap();
        fs::write(root.join("sfx/readme.txt"), b"nope").unwrap();
        let vfs = FsBacked::from_script_path(&root.join("main.lua"));
        let mut stems = vfs.sfx_stems();
        stems.sort();
        assert_eq!(stems, vec!["coin".to_string(), "jump".to_string()]);
        assert_eq!(vfs.read_sfx("jump").as_deref(), Some(b"wav".as_slice()));
        assert!(vfs.read_sfx("missing").is_none());
    }

    #[test]
    fn module_candidates_normalizes_dots() {
        assert_eq!(
            module_candidates("foo"),
            Some(vec!["foo.lua".into(), "foo/init.lua".into()])
        );
        assert_eq!(
            module_candidates("a.b.c"),
            Some(vec!["a/b/c.lua".into(), "a/b/c/init.lua".into()])
        );
    }

    #[test]
    fn module_candidates_rejects_unsafe_names() {
        assert_eq!(module_candidates(""), None);
        assert_eq!(module_candidates("../escape"), None);
        assert_eq!(module_candidates("foo/bar"), None);
        assert_eq!(module_candidates("foo\\bar"), None);
        assert_eq!(module_candidates(".foo"), None);
        assert_eq!(module_candidates("foo."), None);
        assert_eq!(module_candidates("foo..bar"), None);
    }

    #[test]
    fn is_meta_chunk_recognizes_lsp_marker() {
        assert!(is_meta_chunk(b"---@meta\ngfx = {}\n"));
        // Marker on the second line after a blank line still counts —
        // the first non-empty line is what matters.
        assert!(is_meta_chunk(b"\n---@meta\nstuff\n"));
        // Indented marker is fine: trim_start handles it.
        assert!(is_meta_chunk(b"  ---@meta\n"));
        // Plain comment is not a meta marker.
        assert!(!is_meta_chunk(b"-- normal comment\nlocal x = 1\n"));
        // Real code without a marker is not meta.
        assert!(!is_meta_chunk(b"local M = {}\nreturn M\n"));
        assert!(!is_meta_chunk(b""));
    }

    #[test]
    fn fs_backed_read_module_skips_meta_files() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::write(root.join("main.lua"), b"").unwrap();
        fs::create_dir_all(root.join("meta")).unwrap();
        fs::write(root.join("meta/usagi.lua"), b"---@meta\ngfx = {}\n").unwrap();
        let vfs = FsBacked::from_script_path(&root.join("main.lua"));
        assert!(
            vfs.read_module("meta.usagi").is_none(),
            "meta-marked files must not be loadable via require"
        );
    }

    #[test]
    fn fs_backed_read_module_resolves_dots_and_init() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::write(root.join("main.lua"), b"").unwrap();
        fs::write(root.join("enemies.lua"), b"-- enemies").unwrap();
        fs::create_dir_all(root.join("world")).unwrap();
        fs::write(root.join("world/tiles.lua"), b"-- tiles").unwrap();
        fs::create_dir_all(root.join("ui")).unwrap();
        fs::write(root.join("ui/init.lua"), b"-- ui init").unwrap();

        let vfs = FsBacked::from_script_path(&root.join("main.lua"));
        assert_eq!(
            vfs.read_module("enemies").map(|(b, _)| b),
            Some(b"-- enemies".to_vec())
        );
        assert_eq!(
            vfs.read_module("world.tiles").map(|(b, _)| b),
            Some(b"-- tiles".to_vec())
        );
        assert_eq!(
            vfs.read_module("ui").map(|(b, _)| b),
            Some(b"-- ui init".to_vec())
        );
        assert!(vfs.read_module("missing").is_none());
        // Unsafe names are rejected even if a matching file would exist.
        assert!(vfs.read_module("..").is_none());
    }

    #[test]
    fn bundle_backed_read_module_resolves_dots_and_init() {
        let mut b = Bundle::new();
        b.insert("main.lua", b"".to_vec());
        b.insert("enemies.lua", b"-- enemies".to_vec());
        b.insert("world/tiles.lua", b"-- tiles".to_vec());
        b.insert("ui/init.lua", b"-- ui init".to_vec());
        let vfs = BundleBacked::new(b);
        assert_eq!(
            vfs.read_module("enemies").map(|(b, _)| b),
            Some(b"-- enemies".to_vec())
        );
        assert_eq!(
            vfs.read_module("world.tiles").map(|(b, _)| b),
            Some(b"-- tiles".to_vec())
        );
        assert_eq!(
            vfs.read_module("ui").map(|(b, _)| b),
            Some(b"-- ui init".to_vec())
        );
        assert!(vfs.read_module("missing").is_none());
    }

    #[test]
    fn bundle_backed_reads_mapped_paths() {
        let mut b = Bundle::new();
        b.insert("main.lua", b"-- bundled".to_vec());
        b.insert("sprites.png", vec![1, 2, 3]);
        b.insert("sfx/jump.wav", vec![4, 5, 6]);
        let vfs = BundleBacked::new(b);
        assert_eq!(vfs.read_script().as_deref(), Some(b"-- bundled".as_slice()));
        assert_eq!(vfs.read_sprites().as_deref(), Some([1, 2, 3].as_slice()));
        assert_eq!(vfs.read_sfx("jump").as_deref(), Some([4, 5, 6].as_slice()));
        assert_eq!(vfs.sfx_stems(), vec!["jump".to_string()]);
        assert!(!vfs.supports_reload());
    }

    #[test]
    fn freshest_lua_mtime_tracks_files_outside_package_loaded() {
        // Regression for the live-reload "stuck after a partial-reload
        // failure" bug: the watcher must see a save to any .lua file in
        // the project, even one that's not currently in `package.loaded`.
        // Walking `package.loaded` alone (the old approach) hid orphans
        // and made saves silently no-op until restart.
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::write(root.join("main.lua"), b"").unwrap();
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::write(root.join("nested/orphan.lua"), b"return {}").unwrap();
        let vfs = FsBacked::from_script_path(&root.join("main.lua"));

        let baseline = vfs
            .freshest_lua_mtime()
            .expect("baseline mtime present with files on disk");

        // Bump the orphan's mtime. `set_modified` needs a write-capable
        // handle on Windows; OpenOptions::write gets the right access bits
        // portably.
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
        std::fs::OpenOptions::new()
            .write(true)
            .open(root.join("nested/orphan.lua"))
            .unwrap()
            .set_modified(later)
            .unwrap();

        let after = vfs
            .freshest_lua_mtime()
            .expect("still have an mtime after bump");
        assert!(
            after > baseline,
            "saving any project .lua file must move freshest forward (baseline={baseline:?}, after={after:?})"
        );
    }

    #[test]
    fn freshest_lua_mtime_is_none_when_no_lua_files() {
        let dir = TempDir::new().unwrap();
        let vfs = FsBacked::from_project_dir(dir.path().to_path_buf());
        assert!(vfs.freshest_lua_mtime().is_none());
    }

    #[test]
    fn freshest_lua_mtime_ignores_meta_usagi_stub() {
        // `usagi init` ships `meta/usagi.lua` as an LSP type stub. It's
        // never executed at runtime, so saving it shouldn't trigger a
        // (wasted) live reload. Other files in `meta/` still drive
        // reload — the skip is name-specific, not directory-wide.
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("meta")).unwrap();
        fs::write(root.join("meta/usagi.lua"), b"---@meta\n").unwrap();
        let vfs = FsBacked::from_project_dir(root.to_path_buf());
        assert!(
            vfs.freshest_lua_mtime().is_none(),
            "meta/usagi.lua alone must not register on the watcher"
        );

        fs::write(root.join("meta/custom.lua"), b"---@meta\n").unwrap();
        assert!(
            vfs.freshest_lua_mtime().is_some(),
            "user-authored files in meta/ are still tracked"
        );
    }

    #[test]
    fn freshest_lua_mtime_skips_dot_directories() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join(".git/hook.lua"), b"-- ignored").unwrap();
        let vfs = FsBacked::from_project_dir(root.to_path_buf());
        assert!(vfs.freshest_lua_mtime().is_none());
    }

    #[test]
    fn bundle_backed_freshest_lua_mtime_is_none() {
        let mut b = Bundle::new();
        b.insert("main.lua", b"-- bundled".to_vec());
        let vfs = BundleBacked::new(b);
        assert!(vfs.freshest_lua_mtime().is_none());
    }

    #[test]
    fn fs_backed_name_hint_uses_parent_for_main_lua() {
        let dir = TempDir::new().unwrap();
        let project = dir.path().join("snake");
        fs::create_dir(&project).unwrap();
        fs::write(project.join("main.lua"), b"").unwrap();
        let vfs = FsBacked::from_script_path(&project.join("main.lua"));
        assert_eq!(vfs.project_name_hint().as_deref(), Some("snake"));
    }

    #[test]
    fn fs_backed_name_hint_uses_stem_for_flat_script() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("hello.lua"), b"").unwrap();
        let vfs = FsBacked::from_script_path(&dir.path().join("hello.lua"));
        assert_eq!(vfs.project_name_hint().as_deref(), Some("hello"));
    }

    #[test]
    fn fs_backed_name_hint_falls_back_to_root_dir_when_no_script_filename() {
        let dir = TempDir::new().unwrap();
        let project = dir.path().join("my-project");
        fs::create_dir(&project).unwrap();
        let vfs = FsBacked::from_project_dir(project);
        assert_eq!(vfs.project_name_hint().as_deref(), Some("my-project"));
    }

    #[test]
    fn fs_backed_does_not_expose_a_bundle() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("main.lua"), b"").unwrap();
        let vfs = FsBacked::from_script_path(&dir.path().join("main.lua"));
        assert!(vfs.as_bundle().is_none());
    }

    #[test]
    fn bundle_backed_exposes_its_bundle_for_hashing() {
        let mut b = Bundle::new();
        b.insert("main.lua", b"-- a".to_vec());
        let vfs = BundleBacked::new(b);
        let exposed = vfs
            .as_bundle()
            .expect("BundleBacked must expose its bundle");
        // Sanity: bundle round-trip lookup via the trait sees the same data.
        assert_eq!(
            exposed.get("main.lua").map(<[u8]>::to_vec),
            Some(b"-- a".to_vec())
        );
    }

    #[test]
    fn bundle_backed_has_no_name_hint() {
        // In-memory bundles don't carry the original project name; the
        // resolver falls through to the bundle-hash layer instead.
        let vfs = BundleBacked::new(Bundle::new());
        assert!(vfs.project_name_hint().is_none());
    }
}
