//! Bundle format for fused game binaries.
//!
//! A bundle is a flat collection of named byte blobs. It's written in a
//! simple sequential format (no offsets to patch up) so we can read it
//! with forward-only streams:
//!
//! ```text
//! [BUNDLE_MAGIC 8 bytes]
//! [version u32 LE]
//! [entry_count u32 LE]
//! repeated entry_count times:
//!   [name_len u32 LE]
//!   [name bytes]
//!   [data_len u64 LE]
//!   [data bytes]
//! ```
//!
//! When fused onto a base binary, the bundle is appended followed by a
//! 16-byte footer: `[bundle_size u64 LE][EXE_MAGIC 8 bytes]`. The runtime
//! reads the last 16 bytes of its own exe; if the magic matches it seeks
//! back `bundle_size` bytes and parses the bundle.

use std::collections::HashMap;
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use std::path::Path;

const BUNDLE_MAGIC: &[u8; 8] = b"USAGIBND";
const EXE_MAGIC: &[u8; 8] = b"USAGIEXE";
const VERSION: u32 = 1;

#[derive(Default)]
pub struct Bundle {
    files: HashMap<String, Vec<u8>>,
}

impl Bundle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, name: impl Into<String>, data: Vec<u8>) {
        self.files.insert(name.into(), data);
    }

    pub fn get(&self, name: &str) -> Option<&[u8]> {
        self.files.get(name).map(Vec::as_slice)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    pub fn total_bytes(&self) -> usize {
        self.files.values().map(Vec::len).sum()
    }

    pub fn serialize(&self, w: &mut impl Write) -> io::Result<()> {
        w.write_all(BUNDLE_MAGIC)?;
        w.write_all(&VERSION.to_le_bytes())?;
        w.write_all(&(self.files.len() as u32).to_le_bytes())?;
        let mut entries: Vec<_> = self.files.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        for (name, data) in entries {
            let name_bytes = name.as_bytes();
            w.write_all(&(name_bytes.len() as u32).to_le_bytes())?;
            w.write_all(name_bytes)?;
            w.write_all(&(data.len() as u64).to_le_bytes())?;
            w.write_all(data)?;
        }
        Ok(())
    }

    pub fn deserialize(r: &mut impl Read) -> io::Result<Self> {
        let mut magic = [0u8; 8];
        r.read_exact(&mut magic)?;
        if &magic != BUNDLE_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "not a usagi bundle",
            ));
        }
        let version = read_u32(r)?;
        if version != VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported bundle version {version}"),
            ));
        }
        let count = read_u32(r)?;
        let mut files = HashMap::new();
        for _ in 0..count {
            let name_len = read_u32(r)? as usize;
            let mut name_bytes = vec![0u8; name_len];
            r.read_exact(&mut name_bytes)?;
            let name = String::from_utf8(name_bytes).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "non-utf8 file name in bundle")
            })?;
            let data_len = read_u64(r)? as usize;
            let mut data = vec![0u8; data_len];
            r.read_exact(&mut data)?;
            files.insert(name, data);
        }
        Ok(Self { files })
    }

    /// Builds a bundle from a game's script path. Includes the script as
    /// `main.lua`, every other `.lua` under the project root (so `require`
    /// can find them at runtime), `sprites.png` if present, and any
    /// `sfx/*.wav` in the script's parent directory.
    pub fn from_project(script_path: &Path) -> io::Result<Self> {
        let mut bundle = Self::new();
        bundle.insert("main.lua", std::fs::read(script_path)?);

        let root = script_path.parent().unwrap_or_else(|| Path::new("."));

        // Walk the root for additional .lua files. Each one is keyed by
        // its relative path with `/` separators so the BundleBacked vfs
        // resolves `require "world.tiles"` to `world/tiles.lua` the same
        // way FsBacked does on disk. The script itself is skipped — it's
        // already inserted as `main.lua` above, regardless of its on-disk
        // filename.
        let script_canon = std::fs::canonicalize(script_path).ok();
        for (rel, path) in walk_lua_files(root) {
            if script_canon.as_deref() == std::fs::canonicalize(&path).ok().as_deref() {
                continue;
            }
            let bytes = std::fs::read(&path)?;
            // `---@meta` files are LSP type stubs (e.g. `meta/usagi.lua`
            // from `usagi init`). Bundling them would waste space and let
            // a stray `require "meta.usagi"` clobber the engine's real
            // globals at runtime.
            if crate::vfs::is_meta_chunk(&bytes) {
                continue;
            }
            bundle.insert(rel, bytes);
        }

        let sprites = root.join("sprites.png");
        if sprites.is_file() {
            bundle.insert("sprites.png", std::fs::read(&sprites)?);
        }

        let palette = root.join("palette.png");
        if palette.is_file() {
            bundle.insert("palette.png", std::fs::read(&palette)?);
        }

        let sfx_dir = root.join("sfx");
        if sfx_dir.is_dir() {
            for entry in std::fs::read_dir(&sfx_dir)?.flatten() {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) != Some("wav") {
                    continue;
                }
                let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                bundle.insert(format!("sfx/{name}"), std::fs::read(&p)?);
            }
        }

        let shaders_dir = root.join("shaders");
        if shaders_dir.is_dir() {
            for entry in std::fs::read_dir(&shaders_dir)?.flatten() {
                let p = entry.path();
                if !matches!(
                    p.extension().and_then(|e| e.to_str()),
                    Some("fs") | Some("vs")
                ) {
                    continue;
                }
                let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                bundle.insert(format!("shaders/{name}"), std::fs::read(&p)?);
            }
        }

        let music_dir = root.join("music");
        if music_dir.is_dir() {
            for entry in std::fs::read_dir(&music_dir)?.flatten() {
                let p = entry.path();
                let Some(ext) = p
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(str::to_ascii_lowercase)
                else {
                    continue;
                };
                if !crate::vfs::MUSIC_EXTS.contains(&ext.as_str()) {
                    continue;
                }
                let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                bundle.insert(format!("music/{name}"), std::fs::read(&p)?);
            }
        }

        Ok(bundle)
    }

    /// Copies `base` to `out` and appends this bundle + footer. On unix
    /// the output file is made executable.
    pub fn fuse(&self, base: &Path, out: &Path) -> io::Result<()> {
        std::fs::copy(base, out)?;
        let mut f = std::fs::OpenOptions::new().append(true).open(out)?;
        let start = f.metadata()?.len();
        self.serialize(&mut f)?;
        let bundle_size = f.metadata()?.len() - start;
        f.write_all(&bundle_size.to_le_bytes())?;
        f.write_all(EXE_MAGIC)?;
        drop(f);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(out)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(out, perms)?;
        }
        Ok(())
    }

    /// Loads a standalone bundle file from disk. Use this for shipping a
    /// game as a `.usagi` data file (run with `usagi run game.usagi`),
    /// the counterpart to a fused exe.
    pub fn load_from_path(path: &Path) -> io::Result<Self> {
        let mut f = std::fs::File::open(path)?;
        Self::deserialize(&mut f)
    }

    /// Writes this bundle to a standalone file (no exe fusing).
    pub fn write_to_path(&self, path: &Path) -> io::Result<()> {
        let mut f = std::fs::File::create(path)?;
        self.serialize(&mut f)
    }

    /// Checks the current executable for a fused bundle and loads it if
    /// present. Returns None if the exe isn't fused (normal Usagi dev run).
    pub fn load_from_current_exe() -> Option<Self> {
        let exe = std::env::current_exe().ok()?;
        let mut f = std::fs::File::open(&exe).ok()?;
        let size = f.metadata().ok()?.len();
        if size < 16 {
            return None;
        }
        f.seek(SeekFrom::End(-16)).ok()?;
        let mut footer = [0u8; 16];
        f.read_exact(&mut footer).ok()?;
        if &footer[8..16] != EXE_MAGIC {
            return None;
        }
        let bundle_size = u64::from_le_bytes(footer[0..8].try_into().ok()?);
        if bundle_size >= size - 16 {
            return None;
        }
        let offset: i64 = -(16 + bundle_size as i64);
        f.seek(SeekFrom::End(offset)).ok()?;
        let mut buf = vec![0u8; bundle_size as usize];
        f.read_exact(&mut buf).ok()?;
        Self::deserialize(&mut Cursor::new(buf)).ok()
    }
}

/// Collects `(relative_path, full_path)` pairs for every `.lua` file
/// under `root`. Relative paths use `/` separators on every platform so
/// bundle keys round-trip across Windows and Unix.
fn walk_lua_files(root: &Path) -> Vec<(String, std::path::PathBuf)> {
    let mut out = Vec::new();
    crate::vfs::for_each_lua_file(root, |entry| {
        let p = entry.path();
        let Ok(rel) = p.strip_prefix(root) else {
            return;
        };
        let rel_str: String = rel
            .components()
            .filter_map(|c| match c {
                std::path::Component::Normal(s) => s.to_str().map(String::from),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("/");
        out.push((rel_str, p));
    });
    out
}

fn read_u32(r: &mut impl Read) -> io::Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

fn read_u64(r: &mut impl Read) -> io::Result<u64> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b)?;
    Ok(u64::from_le_bytes(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn roundtrip_preserves_files() {
        let mut b = Bundle::new();
        b.insert("main.lua", b"print('hi')".to_vec());
        b.insert("sprites.png", vec![0x89, 0x50, 0x4E, 0x47]); // PNG magic
        b.insert("sfx/jump.wav", vec![1, 2, 3, 4, 5]);

        let mut buf = Vec::new();
        b.serialize(&mut buf).unwrap();

        let decoded = Bundle::deserialize(&mut Cursor::new(&buf)).unwrap();
        assert_eq!(decoded.get("main.lua"), Some(b"print('hi')".as_slice()));
        assert_eq!(
            decoded.get("sprites.png"),
            Some([0x89, 0x50, 0x4E, 0x47].as_slice())
        );
        assert_eq!(
            decoded.get("sfx/jump.wav"),
            Some([1, 2, 3, 4, 5].as_slice())
        );
        assert_eq!(decoded.file_count(), 3);
    }

    #[test]
    fn deserialize_rejects_non_bundle_bytes() {
        let garbage = b"this is not a bundle at all";
        assert!(Bundle::deserialize(&mut Cursor::new(garbage)).is_err());
    }

    #[test]
    fn from_project_picks_up_script_sprites_and_sfx() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::write(root.join("main.lua"), b"function _init() end").unwrap();
        fs::write(root.join("sprites.png"), b"fake png").unwrap();
        fs::create_dir(root.join("sfx")).unwrap();
        fs::write(root.join("sfx/jump.wav"), b"fake wav").unwrap();
        fs::write(root.join("sfx/coin.wav"), b"fake wav 2").unwrap();
        // Non-wav in sfx/ should be ignored.
        fs::write(root.join("sfx/notes.txt"), b"ignored").unwrap();

        let bundle = Bundle::from_project(&root.join("main.lua")).unwrap();
        assert_eq!(
            bundle.get("main.lua"),
            Some(b"function _init() end".as_slice())
        );
        assert_eq!(bundle.get("sprites.png"), Some(b"fake png".as_slice()));
        assert!(bundle.get("sfx/jump.wav").is_some());
        assert!(bundle.get("sfx/coin.wav").is_some());
        assert!(bundle.get("sfx/notes.txt").is_none());
    }

    #[test]
    fn from_project_picks_up_sibling_and_nested_lua_modules() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::write(root.join("main.lua"), b"-- main").unwrap();
        fs::write(root.join("enemies.lua"), b"-- enemies").unwrap();
        fs::create_dir_all(root.join("world")).unwrap();
        fs::write(root.join("world/tiles.lua"), b"-- tiles").unwrap();
        // Hidden directory contents must NOT be bundled.
        fs::create_dir_all(root.join(".zed")).unwrap();
        fs::write(root.join(".zed/secret.lua"), b"-- secret").unwrap();

        let bundle = Bundle::from_project(&root.join("main.lua")).unwrap();
        assert_eq!(bundle.get("main.lua"), Some(b"-- main".as_slice()));
        assert_eq!(bundle.get("enemies.lua"), Some(b"-- enemies".as_slice()));
        assert_eq!(bundle.get("world/tiles.lua"), Some(b"-- tiles".as_slice()));
        assert!(bundle.get(".zed/secret.lua").is_none());
    }

    #[test]
    fn from_project_excludes_meta_marked_files() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::write(root.join("main.lua"), b"-- main").unwrap();
        fs::create_dir_all(root.join("meta")).unwrap();
        fs::write(
            root.join("meta/usagi.lua"),
            b"---@meta\nfunction gfx.clear(c) end\n",
        )
        .unwrap();
        // A meta-marked file outside the conventional `meta/` dir must
        // also be excluded — the rule is the marker, not the path.
        fs::write(root.join("local_stubs.lua"), b"---@meta\nlocal _ = 1\n").unwrap();
        // A normal sibling file in the same project still gets bundled.
        fs::write(root.join("util.lua"), b"return {}").unwrap();

        let bundle = Bundle::from_project(&root.join("main.lua")).unwrap();
        assert!(bundle.get("meta/usagi.lua").is_none());
        assert!(bundle.get("local_stubs.lua").is_none());
        assert_eq!(bundle.get("util.lua"), Some(b"return {}".as_slice()));
    }

    #[test]
    fn from_project_renames_alt_script_to_main_without_duplicating() {
        // When the user runs `usagi export game.lua`, game.lua is the
        // entry point — it goes in the bundle as `main.lua`, and the walk
        // must NOT also drop a `game.lua` entry.
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::write(root.join("game.lua"), b"-- game").unwrap();
        fs::write(root.join("util.lua"), b"-- util").unwrap();

        let bundle = Bundle::from_project(&root.join("game.lua")).unwrap();
        assert_eq!(bundle.get("main.lua"), Some(b"-- game".as_slice()));
        assert!(
            bundle.get("game.lua").is_none(),
            "entry script should not be double-inserted under its source name"
        );
        assert_eq!(bundle.get("util.lua"), Some(b"-- util".as_slice()));
    }

    #[test]
    fn from_project_picks_up_shader_sources() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::write(root.join("main.lua"), b"-- main").unwrap();
        fs::create_dir(root.join("shaders")).unwrap();
        fs::write(root.join("shaders/crt.fs"), b"// frag").unwrap();
        fs::write(root.join("shaders/crt_es.fs"), b"// frag es").unwrap();
        fs::write(root.join("shaders/wave.vs"), b"// vert").unwrap();
        fs::write(root.join("shaders/notes.txt"), b"ignored").unwrap();

        let bundle = Bundle::from_project(&root.join("main.lua")).unwrap();
        assert_eq!(bundle.get("shaders/crt.fs"), Some(b"// frag".as_slice()));
        assert_eq!(
            bundle.get("shaders/crt_es.fs"),
            Some(b"// frag es".as_slice())
        );
        assert_eq!(bundle.get("shaders/wave.vs"), Some(b"// vert".as_slice()));
        assert!(bundle.get("shaders/notes.txt").is_none());
    }

    #[test]
    fn from_project_works_without_optional_assets() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::write(root.join("game.lua"), b"-- minimal").unwrap();

        let bundle = Bundle::from_project(&root.join("game.lua")).unwrap();
        assert_eq!(bundle.get("main.lua"), Some(b"-- minimal".as_slice()));
        assert!(bundle.get("sprites.png").is_none());
        assert_eq!(bundle.file_count(), 1);
    }

    #[test]
    fn write_and_load_standalone_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("game.usagi");

        let mut b = Bundle::new();
        b.insert("main.lua", b"print('hi')".to_vec());
        b.insert("sfx/jump.wav", vec![1, 2, 3]);
        b.write_to_path(&path).unwrap();

        let loaded = Bundle::load_from_path(&path).unwrap();
        assert_eq!(loaded.get("main.lua"), Some(b"print('hi')".as_slice()));
        assert_eq!(loaded.get("sfx/jump.wav"), Some([1, 2, 3].as_slice()));
        assert_eq!(loaded.file_count(), 2);
    }

    #[test]
    fn load_from_path_rejects_garbage_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("not-a-bundle");
        fs::write(&path, b"hello world").unwrap();
        assert!(Bundle::load_from_path(&path).is_err());
    }

    #[test]
    fn fuse_and_load_from_exe_roundtrip() {
        let dir = TempDir::new().unwrap();
        let base = dir.path().join("base");
        fs::write(&base, b"FAKE_EXE_CONTENT").unwrap();

        let mut bundle = Bundle::new();
        bundle.insert("main.lua", b"print(1)".to_vec());
        bundle.insert("sfx/a.wav", vec![9, 8, 7]);

        let fused = dir.path().join("fused");
        bundle.fuse(&base, &fused).unwrap();

        // The file should start with the base content.
        let fused_bytes = fs::read(&fused).unwrap();
        assert!(fused_bytes.starts_with(b"FAKE_EXE_CONTENT"));

        // Recover: read last 16 bytes, verify magic, pull the bundle.
        let size = fused_bytes.len();
        let footer = &fused_bytes[size - 16..];
        assert_eq!(&footer[8..16], EXE_MAGIC);
        let bundle_size = u64::from_le_bytes(footer[0..8].try_into().unwrap()) as usize;
        let bundle_start = size - 16 - bundle_size;
        let bundle_bytes = &fused_bytes[bundle_start..size - 16];
        let recovered = Bundle::deserialize(&mut Cursor::new(bundle_bytes)).unwrap();
        assert_eq!(recovered.get("main.lua"), Some(b"print(1)".as_slice()));
        assert_eq!(recovered.get("sfx/a.wav"), Some([9, 8, 7].as_slice()));
    }
}
