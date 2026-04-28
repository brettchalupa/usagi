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
    fn script_mtime(&self) -> Option<SystemTime>;

    fn read_sprites(&self) -> Option<Vec<u8>>;
    fn sprites_mtime(&self) -> Option<SystemTime>;

    fn sfx_stems(&self) -> Vec<String>;
    fn read_sfx(&self, stem: &str) -> Option<Vec<u8>>;
    fn sfx_manifest(&self) -> HashMap<String, SystemTime>;

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

    /// Whether filesystem reload checks are meaningful on this vfs.
    /// `FsBacked` returns true; `BundleBacked` always returns false.
    fn supports_reload(&self) -> bool;
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

/// Disk-backed vfs. `root` is the directory that holds `sprites.png` and
/// `sfx/`. `script_filename` is the main Lua file inside `root` (None when
/// the vfs is used purely for asset browsing, e.g. the tools window).
pub struct FsBacked {
    root: PathBuf,
    script_filename: Option<String>,
}

impl FsBacked {
    pub fn from_script_path(script_path: &Path) -> Self {
        let root = script_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
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

    fn sfx_dir(&self) -> PathBuf {
        self.root.join("sfx")
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

    fn script_mtime(&self) -> Option<SystemTime> {
        std::fs::metadata(self.script_path()?)
            .and_then(|m| m.modified())
            .ok()
    }

    fn read_sprites(&self) -> Option<Vec<u8>> {
        std::fs::read(self.sprites_path()).ok()
    }

    fn sprites_mtime(&self) -> Option<SystemTime> {
        std::fs::metadata(self.sprites_path())
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

    fn supports_reload(&self) -> bool {
        true
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

    fn script_mtime(&self) -> Option<SystemTime> {
        None
    }

    fn read_sprites(&self) -> Option<Vec<u8>> {
        self.bundle.get("sprites.png").map(<[u8]>::to_vec)
    }

    fn sprites_mtime(&self) -> Option<SystemTime> {
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

    fn read_module(&self, mod_name: &str) -> Option<(Vec<u8>, String)> {
        let candidates = module_candidates(mod_name)?;
        for rel in candidates {
            if let Some(bytes) = self.bundle.get(&rel) {
                return Some((bytes.to_vec(), rel));
            }
        }
        None
    }

    fn supports_reload(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn fs_backed_reads_script_and_mtime() {
        let dir = TempDir::new().unwrap();
        let script = dir.path().join("game.lua");
        fs::write(&script, b"-- hello").unwrap();
        let vfs = FsBacked::from_script_path(&script);
        assert_eq!(vfs.read_script().as_deref(), Some(b"-- hello".as_slice()));
        assert!(vfs.script_mtime().is_some());
        assert!(vfs.supports_reload());
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
        assert!(vfs.script_mtime().is_none());
    }
}
