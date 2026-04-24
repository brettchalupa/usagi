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

    /// Whether filesystem reload checks are meaningful on this vfs.
    /// `FsBacked` returns true; `BundleBacked` always returns false.
    fn supports_reload(&self) -> bool;
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
