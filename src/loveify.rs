//! `usagi loveify` implementation. Ports an Usagi project to a
//! Love2D 11.5 project by copying the source tree to a new
//! destination, applying Lua source transforms (compound assignment
//! expansion via [`crate::preprocess`]) for LuaJIT compat, dropping in
//! the Love shim runtime files, and bundling the engine's default
//! monogram font when the source has no custom `font.png`.
//!
//! Refuses to overwrite an existing destination. The Lua sources, shim,
//! and `conf.lua` are all embedded into the engine binary via
//! `include_str!` / `include_bytes!` so the canonical files live in
//! `examples/love_shim/` and stay the single source of truth.

use std::fs;
use std::path::{Path, PathBuf};

/// Canonical Love shim source. Single source of truth — edits in the
/// example dir get baked in on the next `cargo build`.
const SHIM_LUA: &str = include_str!("../examples/loveify/usagi_shim.lua");
const CONF_LUA: &str = include_str!("../examples/loveify/conf.lua");

/// Bundled default font, in Usagi's baked PNG-with-zTXt-metadata
/// format. The shim's font loader expects exactly this layout.
const DEFAULT_FONT_PNG: &[u8] = include_bytes!("../assets/monogram.png");

pub fn run(src: &str, dst: &str) -> Result<(), String> {
    let src_path = Path::new(src);
    let dst_path = Path::new(dst);

    if !src_path.exists() {
        return Err(format!("source path '{src}' does not exist"));
    }
    if !src_path.is_dir() {
        return Err(format!("source path '{src}' is not a directory"));
    }
    let main_lua = src_path.join("main.lua");
    if !main_lua.exists() {
        return Err(format!(
            "source '{src}' has no main.lua at its root. \
             loveify expects an Usagi project directory."
        ));
    }
    if dst_path.exists() {
        return Err(format!(
            "destination '{dst}' already exists. \
             Refusing to overwrite — remove it or pick a different path."
        ));
    }
    // Refuse when the destination lives inside the source: the walk
    // would descend into the freshly-created dst and copy forever
    // (classic `usagi loveify . love_example` from within a project).
    if dst_inside_src(src_path, dst_path) {
        return Err(format!(
            "destination '{dst}' is inside the source project '{src}'. \
             loveify can't port a project into itself. Pick a \
             destination outside the source directory."
        ));
    }

    fs::create_dir_all(dst_path).map_err(|e| format!("creating destination '{dst}': {e}"))?;

    let mut stats = Stats::default();
    walk_and_port(src_path, src_path, dst_path, &mut stats)?;

    // Embedded runtime files.
    write_file(&dst_path.join("usagi_shim.lua"), SHIM_LUA.as_bytes())
        .map_err(|e| format!("writing usagi_shim.lua: {e}"))?;
    write_file(&dst_path.join("conf.lua"), CONF_LUA.as_bytes())
        .map_err(|e| format!("writing conf.lua: {e}"))?;

    // Default monogram font when the source had no custom font.
    let dst_font = dst_path.join("font.png");
    let mut dropped_default_font = false;
    if !dst_font.exists() {
        write_file(&dst_font, DEFAULT_FONT_PNG).map_err(|e| format!("writing font.png: {e}"))?;
        dropped_default_font = true;
    }

    println!("loveify: {src} -> {dst}");
    println!(
        "  {} file(s) copied, {} .lua file(s) transformed",
        stats.files_copied, stats.lua_transformed
    );
    println!("  + usagi_shim.lua and conf.lua dropped at the destination root");
    println!("  + 'require \"usagi_shim\"' prepended to main.lua");
    if dropped_default_font {
        println!("  + font.png (bundled monogram) dropped at the destination root");
    } else {
        println!("  + font.png from source preserved");
    }
    if !stats.warnings.is_empty() {
        println!();
        println!(
            "warnings: {} feature(s) the shim doesn't transform automatically:",
            stats.warnings.len()
        );
        for w in &stats.warnings {
            println!("  {}:{}  {}", w.file, w.line, w.feature);
            println!("    {}", w.hint);
        }
    }
    println!();
    println!("next step:  cd {dst} && love .");
    Ok(())
}

/// True when `dst` is the same as `src` or nested inside it. Resolves
/// both to absolute paths first so `.`, `..`, and relative forms don't
/// fool the comparison. `dst` need not exist yet.
fn dst_inside_src(src: &Path, dst: &Path) -> bool {
    match (abs_lenient(src), abs_lenient(dst)) {
        (Some(s), Some(d)) => d.starts_with(&s),
        _ => false,
    }
}

/// Best-effort absolute path for a path that may not exist yet:
/// canonicalize the longest existing ancestor and re-join the missing
/// tail. Falls back to a lexical join against the current dir.
fn abs_lenient(p: &Path) -> Option<PathBuf> {
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(p)
    };
    if let Ok(c) = fs::canonicalize(&abs) {
        return Some(c);
    }
    for ancestor in abs.ancestors().skip(1) {
        if let Ok(base) = fs::canonicalize(ancestor) {
            let rest = abs.strip_prefix(ancestor).ok()?;
            return Some(base.join(rest));
        }
    }
    Some(abs)
}

#[derive(Default)]
struct Stats {
    files_copied: usize,
    lua_transformed: usize,
    warnings: Vec<Warning>,
}

struct Warning {
    file: String,
    line: usize,
    feature: &'static str,
    hint: &'static str,
}

/// Directories at the project root that we skip entirely. These are
/// Usagi-specific or VCS / OS junk that would just bloat the Love
/// port. Top-level only — a `meta/` subdir inside `data/` would still
/// be copied.
const SKIP_TOP_LEVEL_DIRS: &[&str] = &[
    "export", // `usagi export` artifacts (.zip, .usagi)
    "meta",   // LSP stubs (Usagi-specific)
    ".git",   // VCS
];

fn walk_and_port(
    root: &Path,
    cur: &Path,
    dst_root: &Path,
    stats: &mut Stats,
) -> Result<(), String> {
    let entries = fs::read_dir(cur).map_err(|e| format!("reading dir '{}': {e}", cur.display()))?;
    let mut sorted: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    sorted.sort_by_key(|e| e.path());

    for entry in sorted {
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .map_err(|e| format!("strip_prefix: {e}"))?;
        let dst_path = dst_root.join(rel);
        let ft = entry.file_type().map_err(|e| format!("file_type: {e}"))?;
        if ft.is_dir() {
            // Skip Usagi-specific dirs at the project root.
            if path.parent() == Some(root)
                && let Some(name) = path.file_name().and_then(|n| n.to_str())
                && SKIP_TOP_LEVEL_DIRS.contains(&name)
            {
                continue;
            }
            fs::create_dir_all(&dst_path)
                .map_err(|e| format!("mkdir '{}': {e}", dst_path.display()))?;
            walk_and_port(root, &path, dst_root, stats)?;
        } else if ft.is_file() {
            port_file(&path, &dst_path, rel, stats)?;
        }
        // Symlinks: skip; Usagi projects don't use them and following
        // would complicate the loop with little upside.
    }
    Ok(())
}

fn port_file(src: &Path, dst: &Path, rel: &Path, stats: &mut Stats) -> Result<(), String> {
    let bytes = fs::read(src).map_err(|e| format!("reading '{}': {e}", src.display()))?;
    let rel_str = rel.to_string_lossy().to_string();
    let is_lua = rel.extension().is_some_and(|e| e == "lua");
    let is_main = rel_str == "main.lua";

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir '{}': {e}", parent.display()))?;
    }

    let final_bytes = if is_lua {
        let transformed = crate::preprocess::preprocess(&bytes);
        if transformed != bytes {
            stats.lua_transformed += 1;
        }
        scan_for_warnings(&transformed, &rel_str, &mut stats.warnings);
        if is_main {
            // Prepend `require "usagi_shim"` so the shim takes over
            // Love's callbacks.
            let mut out = Vec::with_capacity(transformed.len() + 32);
            out.extend_from_slice(b"require \"usagi_shim\"\n\n");
            out.extend_from_slice(&transformed);
            out
        } else {
            transformed
        }
    } else {
        bytes
    };

    write_file(dst, &final_bytes).map_err(|e| format!("writing '{}': {e}", dst.display()))?;
    stats.files_copied += 1;
    Ok(())
}

fn write_file(dst: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(dst, bytes)
}

/// Surface features LuaJIT (Love's runtime) doesn't support so the
/// developer can hand-fix them. Cheap line-level scan; doesn't try to
/// be precise about strings or comments, so false positives are
/// possible — the user reads and decides.
fn scan_for_warnings(bytes: &[u8], file: &str, out: &mut Vec<Warning>) {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return;
    };
    for (i, raw_line) in text.lines().enumerate() {
        let line_no = i + 1;
        // Strip a trailing short comment so we don't false-positive on
        // operators in comment text. Doesn't handle long comments.
        let line = match raw_line.find("--") {
            Some(j) => &raw_line[..j],
            None => raw_line,
        };
        if line.contains("//") {
            out.push(Warning {
                file: file.to_string(),
                line: line_no,
                feature: "// (integer division, Lua 5.3+)",
                hint: "rewrite as math.floor(a / b); LuaJIT has no // operator",
            });
        }
        if line.contains("string.pack") || line.contains("string.unpack") {
            out.push(Warning {
                file: file.to_string(),
                line: line_no,
                feature: "string.pack / string.unpack (Lua 5.3+)",
                hint: "no LuaJIT equivalent; rewrite by hand using string.byte/char",
            });
        }
        if line.contains("<const>") || line.contains("<close>") {
            out.push(Warning {
                file: file.to_string(),
                line: line_no,
                feature: "<const>/<close> attribute (Lua 5.4+)",
                hint: "strip the attribute; LuaJIT will reject the syntax",
            });
        }
        if looks_like_bitwise(line) {
            out.push(Warning {
                file: file.to_string(),
                line: line_no,
                feature: "bitwise operator (Lua 5.3+)",
                hint: "use LuaJIT's bit module: bit.band, bor, bxor, lshift, rshift",
            });
        }
    }
}

fn looks_like_bitwise(line: &str) -> bool {
    // Avoid the common false-positives: `or` (could match `|`-ish only
    // if we matched `|`), bitwise `~` vs unary not, table key `[~x]`.
    // Heuristic: a binary `&`, `|`, `<<`, or `>>` between two
    // word-ish tokens.
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if (c == b'&' || c == b'|') && i > 0 && i + 1 < bytes.len() {
            // Not `&&` or `||` (Lua doesn't have those; presence
            // probably means we're looking at the wrong language file
            // anyway, but skip just in case).
            if bytes[i + 1] == c {
                i += 2;
                continue;
            }
            // Surrounded by spaces (or alnum) on both sides → looks
            // like a binary op rather than syntactic punctuation.
            if is_token_boundary_byte(bytes[i - 1])
                && i + 1 < bytes.len()
                && is_token_boundary_byte(bytes[i + 1])
            {
                return true;
            }
        }
        if c == b'<' && i + 1 < bytes.len() && bytes[i + 1] == b'<' {
            return true;
        }
        if c == b'>' && i + 1 < bytes.len() && bytes[i + 1] == b'>' {
            return true;
        }
        i += 1;
    }
    false
}

fn is_token_boundary_byte(b: u8) -> bool {
    b == b' '
        || b == b'\t'
        || b.is_ascii_alphanumeric()
        || b == b'_'
        || b == b')'
        || b == b']'
        || b == b'('
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn refuses_when_destination_exists() {
        let tmp = tempdir().unwrap();
        let src = tmp.path().join("src");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("main.lua"), b"function _draw() end\n").unwrap();
        let dst = tmp.path().join("dst");
        fs::create_dir(&dst).unwrap();
        let err = run(src.to_str().unwrap(), dst.to_str().unwrap()).unwrap_err();
        assert!(err.contains("already exists"), "got: {err}");
    }

    #[test]
    fn refuses_when_destination_is_inside_source() {
        let tmp = tempdir().unwrap();
        let src = tmp.path().join("src");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("main.lua"), b"function _draw() end\n").unwrap();
        // dst nested under src, as with `usagi loveify . love_example`.
        let dst = src.join("love_example");
        let err = run(src.to_str().unwrap(), dst.to_str().unwrap()).unwrap_err();
        assert!(err.contains("inside the source"), "got: {err}");
        assert!(!dst.exists(), "nothing should be written");
    }

    #[test]
    fn refuses_when_source_lacks_main_lua() {
        let tmp = tempdir().unwrap();
        let src = tmp.path().join("src");
        fs::create_dir(&src).unwrap();
        // No main.lua written.
        let dst = tmp.path().join("dst");
        let err = run(src.to_str().unwrap(), dst.to_str().unwrap()).unwrap_err();
        assert!(err.contains("no main.lua"), "got: {err}");
    }

    #[test]
    fn copies_main_and_drops_shim_files() {
        let tmp = tempdir().unwrap();
        let src = tmp.path().join("src");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("main.lua"), b"function _draw() end\n").unwrap();
        let dst = tmp.path().join("dst");
        run(src.to_str().unwrap(), dst.to_str().unwrap()).unwrap();

        assert!(dst.join("main.lua").exists());
        assert!(dst.join("usagi_shim.lua").exists());
        assert!(dst.join("conf.lua").exists());
        // No source font.png → default monogram should land.
        assert!(dst.join("font.png").exists());

        let main = fs::read_to_string(dst.join("main.lua")).unwrap();
        assert!(
            main.starts_with("require \"usagi_shim\""),
            "main.lua should start with require; got:\n{main}"
        );
    }

    #[test]
    fn preserves_user_font_png_when_present() {
        let tmp = tempdir().unwrap();
        let src = tmp.path().join("src");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("main.lua"), b"function _draw() end\n").unwrap();
        let user_font = b"USER FONT BYTES";
        fs::write(src.join("font.png"), user_font).unwrap();

        let dst = tmp.path().join("dst");
        run(src.to_str().unwrap(), dst.to_str().unwrap()).unwrap();

        let out_font = fs::read(dst.join("font.png")).unwrap();
        assert_eq!(out_font, user_font, "user's font.png should be preserved");
    }

    #[test]
    fn applies_compound_assignment_transform() {
        let tmp = tempdir().unwrap();
        let src = tmp.path().join("src");
        fs::create_dir(&src).unwrap();
        fs::write(
            src.join("main.lua"),
            b"function _update(dt)\n  State.t += dt\nend\n",
        )
        .unwrap();

        let dst = tmp.path().join("dst");
        run(src.to_str().unwrap(), dst.to_str().unwrap()).unwrap();

        let main = fs::read_to_string(dst.join("main.lua")).unwrap();
        assert!(
            main.contains("State.t = State.t + (dt)"),
            "compound op should be rewritten; got:\n{main}"
        );
    }

    #[test]
    fn skips_top_level_usagi_dirs() {
        let tmp = tempdir().unwrap();
        let src = tmp.path().join("src");
        fs::create_dir_all(src.join("export")).unwrap();
        fs::create_dir_all(src.join("meta")).unwrap();
        fs::create_dir_all(src.join(".git")).unwrap();
        fs::write(src.join("main.lua"), b"function _draw() end\n").unwrap();
        fs::write(src.join("export/build.zip"), b"ZIP").unwrap();
        fs::write(src.join("meta/usagi.lua"), b"-- stubs").unwrap();
        fs::write(src.join(".git/HEAD"), b"ref").unwrap();

        let dst = tmp.path().join("dst");
        run(src.to_str().unwrap(), dst.to_str().unwrap()).unwrap();

        assert!(!dst.join("export").exists(), "export/ should be skipped");
        assert!(!dst.join("meta").exists(), "meta/ should be skipped");
        assert!(!dst.join(".git").exists(), ".git/ should be skipped");
        assert!(dst.join("main.lua").exists());
    }

    #[test]
    fn does_not_skip_dirs_named_export_at_nested_paths() {
        let tmp = tempdir().unwrap();
        let src = tmp.path().join("src");
        // A nested data/export/ should be copied even though the
        // top-level `export/` is in the skip list.
        fs::create_dir_all(src.join("data/export")).unwrap();
        fs::write(src.join("main.lua"), b"function _draw() end\n").unwrap();
        fs::write(src.join("data/export/level.json"), b"{}").unwrap();

        let dst = tmp.path().join("dst");
        run(src.to_str().unwrap(), dst.to_str().unwrap()).unwrap();

        assert!(
            dst.join("data/export/level.json").exists(),
            "nested data/export/ should still be copied"
        );
    }

    #[test]
    fn copies_nested_assets() {
        let tmp = tempdir().unwrap();
        let src = tmp.path().join("src");
        fs::create_dir_all(src.join("sfx")).unwrap();
        fs::create_dir_all(src.join("data")).unwrap();
        fs::write(src.join("main.lua"), b"function _draw() end\n").unwrap();
        fs::write(src.join("sfx/jump.wav"), b"WAV_BYTES").unwrap();
        fs::write(src.join("data/levels.json"), b"{\"hi\":1}").unwrap();

        let dst = tmp.path().join("dst");
        run(src.to_str().unwrap(), dst.to_str().unwrap()).unwrap();

        assert_eq!(fs::read(dst.join("sfx/jump.wav")).unwrap(), b"WAV_BYTES");
        assert_eq!(
            fs::read(dst.join("data/levels.json")).unwrap(),
            b"{\"hi\":1}"
        );
    }
}
