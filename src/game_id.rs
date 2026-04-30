//! Stable identifier for a game project. Same string namespaces save data
//! (`save.rs`), the macOS CFBundleIdentifier (`macos_app.rs`), and any
//! future per-game preference (audio, fullscreen, ...). Anywhere a game
//! needs a stable id, prefer this resolver over baking in the project name.
//!
//! Layered fallback (first match wins):
//!
//! 1. `_config().game_id` (explicit). Validated with `save::validate_game_id`
//!    before being accepted; an ill-formed value falls through to the next
//!    layer rather than erroring.
//! 2. `com.usagiengine.<sanitized-name-hint>`. Stable as long as the project
//!    name (directory or file stem) doesn't change.
//! 3. `com.usagiengine.auto<short-bundle-hash>`. Last resort, kicks in only
//!    when no name hint is available or it sanitizes to empty. Changes when
//!    the bundle's bytes change, so saves keyed off this id won't survive an
//!    update; better than every such game colliding on a single literal id.
//! 4. `com.usagiengine.unknown` if the caller had no inputs to chain
//!    against. Should not happen in practice, persistence is at least
//!    consistent within one runtime if it does.
//!
//! The runtime path (`session::Session::new`) and the export path
//! (`resolve_for_export` below) share the same chain. The runtime feeds
//! `_config().game_id` directly because the Lua VM is already up; the
//! export path spins up a throwaway Lua VM to read it.

use crate::bundle::Bundle;
use crate::save::validate_game_id;
use sha2::{Digest, Sha256};

/// Resolves the best-available identifier given whatever inputs the caller
/// has on hand. Walks the layered fallback documented at the module level.
pub fn resolve(explicit: Option<&str>, name_hint: Option<&str>, bundle: Option<&Bundle>) -> String {
    if let Some(id) = from_explicit(explicit) {
        return id;
    }
    if let Some(id) = name_hint.and_then(from_name) {
        return id;
    }
    if let Some(b) = bundle {
        return from_bundle_hash(b);
    }
    "com.usagiengine.unknown".to_string()
}

/// Friendly short name extracted from a resolved game id, suitable
/// for use as a filename prefix on capture artifacts (gif, png, etc).
/// Returns the last dot-separated segment of `game_id` so
/// `com.brettmakesgames.snake` becomes `snake`. Substitutes `usagi`
/// for the `com.usagiengine.unknown` sentinel because
/// `unknown-20260101.gif` reads worse than `usagi-20260101.gif`.
///
/// `validate_game_id` already restricts ids to filesystem-safe
/// characters, so the returned slice is always a usable filename
/// component without further sanitization.
pub fn short_name(game_id: &str) -> &str {
    let last = game_id.rsplit('.').next().unwrap_or("");
    if last.is_empty() || last == "unknown" {
        "usagi"
    } else {
        last
    }
}

/// Convenience entry for `usagi export`. Reads `_config().game_id` out of
/// the project's main.lua before delegating to `resolve`. Native-only
/// because export itself is native-only and pulling in `assets`+`vfs` here
/// would just be ceremony for the wasm build.
#[cfg(not(target_os = "emscripten"))]
pub fn resolve_for_export(script_path: &std::path::Path, name: &str, bundle: &Bundle) -> String {
    let explicit = read_game_id_from_project(script_path);
    resolve(explicit.as_deref(), Some(name), Some(bundle))
}

/// Spins up a throwaway Lua VM, runs the project's main.lua, calls
/// `_config()`, and reads `game_id`. Returns None on any failure (script
/// load error, no `_config`, non-table return, missing field, invalid id).
/// Errors are swallowed by design so the resolver can fall through.
#[cfg(not(target_os = "emscripten"))]
fn read_game_id_from_project(script_path: &std::path::Path) -> Option<String> {
    use crate::api::setup_api;
    use crate::assets::{install_require, load_script};
    use crate::vfs::{FsBacked, VirtualFs};
    use mlua::prelude::*;
    use std::rc::Rc;

    let vfs: Rc<dyn VirtualFs> = Rc::new(FsBacked::from_script_path(script_path));
    let lua = Lua::new();
    setup_api(&lua, false).ok()?;
    install_require(&lua, vfs.clone()).ok()?;
    load_script(&lua, vfs.as_ref()).ok()?;
    let config_fn: LuaFunction = lua.globals().get("_config").ok()?;
    let tbl: LuaTable = config_fn.call(()).ok()?;
    let id: String = tbl.get::<Option<String>>("game_id").ok()??;
    validate_game_id(&id).ok()?;
    Some(id)
}

/// Validates an explicit, dev-supplied id. Returns None when missing or
/// when `validate_game_id` rejects it; the caller falls through.
fn from_explicit(id: Option<&str>) -> Option<String> {
    let id = id?;
    validate_game_id(id).ok()?;
    Some(id.to_string())
}

/// `com.usagiengine.<sanitized-name>` when sanitization leaves something.
/// Returns None for inputs that sanitize to empty so the caller can drop
/// to the bundle-hash fallback.
fn from_name(name: &str) -> Option<String> {
    let s = sanitize(name);
    if s.is_empty() {
        None
    } else {
        Some(format!("com.usagiengine.{s}"))
    }
}

/// Hash of the serialized bundle, prefixed so the last segment starts with
/// a letter (Apple recommends identifier segments not start with a digit;
/// other targets don't care, but the rule is cheap to honor everywhere).
/// 8 bytes / 16 hex chars is plenty to avoid collisions among the games on
/// any one machine.
fn from_bundle_hash(bundle: &Bundle) -> String {
    let mut buf = Vec::new();
    // Bundle::serialize writes to any io::Write; into a Vec is infallible
    // in practice. If it ever returns Err we still want a reasonable id,
    // so fall through to the partial buffer rather than panic.
    let _ = bundle.serialize(&mut buf);
    let digest = Sha256::digest(&buf);
    let hex: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
    format!("com.usagiengine.auto{hex}")
}

/// Lower-cases and rewrites every non-`[a-z0-9-_]` char as `-`, then trims
/// leading/trailing `-` and `_`. Public because save data and macOS bundle
/// staging both use a sanitized form of the project name in places this
/// resolver isn't the right call (e.g. directory names on disk).
pub fn sanitize(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            let lower = c.to_ascii_lowercase();
            if lower.is_ascii_alphanumeric() || lower == '-' || lower == '_' {
                lower
            } else {
                '-'
            }
        })
        .collect();
    s.trim_matches(|c: char| c == '-' || c == '_').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_lowercases_and_keeps_hyphens_underscores() {
        assert_eq!(sanitize("My-Cool_Game"), "my-cool_game");
    }

    #[test]
    fn short_name_returns_last_dot_segment() {
        assert_eq!(short_name("com.brettmakesgames.snake"), "snake");
        assert_eq!(short_name("com.usagiengine.notetris"), "notetris");
    }

    #[test]
    fn short_name_handles_id_without_dots() {
        assert_eq!(short_name("snake"), "snake");
    }

    #[test]
    fn short_name_substitutes_usagi_for_unknown_sentinel() {
        assert_eq!(short_name("com.usagiengine.unknown"), "usagi");
    }

    #[test]
    fn short_name_substitutes_usagi_for_empty() {
        assert_eq!(short_name(""), "usagi");
    }

    #[test]
    fn sanitize_rewrites_disallowed_and_trims_separators() {
        assert_eq!(sanitize("My Game!"), "my-game");
        assert_eq!(sanitize("a/b.c"), "a-b-c");
        assert_eq!(sanitize("--game--"), "game");
        assert_eq!(sanitize("__game__"), "game");
    }

    #[test]
    fn sanitize_returns_empty_for_pure_punctuation() {
        assert_eq!(sanitize("!!!"), "");
        assert_eq!(sanitize(""), "");
    }

    #[test]
    fn from_explicit_returns_id_when_valid() {
        assert_eq!(
            from_explicit(Some("com.test.foo")).as_deref(),
            Some("com.test.foo"),
        );
    }

    #[test]
    fn from_explicit_returns_none_when_invalid_or_missing() {
        assert!(from_explicit(None).is_none());
        assert!(from_explicit(Some("")).is_none());
        assert!(from_explicit(Some("../bad")).is_none());
    }

    #[test]
    fn from_name_returns_namespaced_id_for_normal_name() {
        assert_eq!(from_name("snake").as_deref(), Some("com.usagiengine.snake"));
    }

    #[test]
    fn from_name_returns_none_when_sanitization_empties_the_name() {
        assert!(from_name("!!!").is_none());
        assert!(from_name("").is_none());
    }

    #[test]
    fn from_bundle_hash_is_deterministic_and_well_formed() {
        let mut b = Bundle::new();
        b.insert("main.lua", b"-- a".to_vec());
        let id1 = from_bundle_hash(&b);
        let id2 = from_bundle_hash(&b);
        assert_eq!(id1, id2);
        assert!(id1.starts_with("com.usagiengine.auto"));
        let suffix = id1.trim_start_matches("com.usagiengine.auto");
        assert_eq!(suffix.len(), 16);
        assert!(
            suffix
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase())
        );
    }

    #[test]
    fn from_bundle_hash_changes_with_bundle_contents() {
        let mut a = Bundle::new();
        a.insert("main.lua", b"-- a".to_vec());
        let mut b = Bundle::new();
        b.insert("main.lua", b"-- b".to_vec());
        assert_ne!(from_bundle_hash(&a), from_bundle_hash(&b));
    }

    #[test]
    fn resolve_prefers_explicit_when_set_and_valid() {
        assert_eq!(
            resolve(Some("com.test.foo"), Some("snake"), None),
            "com.test.foo",
        );
    }

    #[test]
    fn resolve_falls_through_when_explicit_is_invalid() {
        assert_eq!(
            resolve(Some("../bad"), Some("snake"), None),
            "com.usagiengine.snake",
        );
    }

    #[test]
    fn resolve_uses_name_when_no_explicit() {
        assert_eq!(resolve(None, Some("snake"), None), "com.usagiengine.snake");
    }

    #[test]
    fn resolve_uses_bundle_hash_when_no_explicit_and_no_useful_name() {
        let mut b = Bundle::new();
        b.insert("main.lua", b"-- contents".to_vec());
        let id = resolve(None, Some("!!!"), Some(&b));
        assert!(id.starts_with("com.usagiengine.auto"), "got: {id}");
    }

    #[test]
    fn resolve_uses_bundle_hash_when_no_name_hint() {
        let mut b = Bundle::new();
        b.insert("main.lua", b"-- contents".to_vec());
        let id = resolve(None, None, Some(&b));
        assert!(id.starts_with("com.usagiengine.auto"), "got: {id}");
    }

    #[test]
    fn resolve_returns_unknown_sentinel_when_no_inputs_chain() {
        // Pathological: caller has no explicit, no usable name, no bundle.
        // Better to be consistent within one runtime than to panic.
        assert_eq!(resolve(None, Some(""), None), "com.usagiengine.unknown");
        assert_eq!(resolve(None, None, None), "com.usagiengine.unknown");
    }

    #[cfg(not(target_os = "emscripten"))]
    mod export_path {
        use super::super::*;
        use std::fs;
        use tempfile::tempdir;

        fn project_with_main(body: &str) -> (tempfile::TempDir, std::path::PathBuf) {
            let dir = tempdir().unwrap();
            let script = dir.path().join("main.lua");
            fs::write(&script, body).unwrap();
            (dir, script)
        }

        #[test]
        fn resolve_for_export_reads_explicit_game_id_from_config() {
            let (_d, script) =
                project_with_main(r#"function _config() return { game_id = "com.test.foo" } end"#);
            let bundle = Bundle::new();
            assert_eq!(
                resolve_for_export(&script, "ignored", &bundle),
                "com.test.foo",
            );
        }

        #[test]
        fn resolve_for_export_falls_through_invalid_explicit_to_name() {
            let (_d, script) =
                project_with_main(r#"function _config() return { game_id = "../bad" } end"#);
            let bundle = Bundle::new();
            assert_eq!(
                resolve_for_export(&script, "snake", &bundle),
                "com.usagiengine.snake",
            );
        }

        #[test]
        fn resolve_for_export_falls_through_missing_config_to_name() {
            let (_d, script) = project_with_main(r#"-- no _config defined"#);
            let bundle = Bundle::new();
            assert_eq!(
                resolve_for_export(&script, "snake", &bundle),
                "com.usagiengine.snake",
            );
        }

        #[test]
        fn resolve_for_export_falls_through_to_bundle_hash_when_name_empty() {
            let (_d, script) = project_with_main(r#"-- no _config defined"#);
            let mut bundle = Bundle::new();
            bundle.insert("main.lua", b"-- contents".to_vec());
            let id = resolve_for_export(&script, "!!!", &bundle);
            assert!(id.starts_with("com.usagiengine.auto"), "got: {id}");
        }

        #[test]
        fn resolve_for_export_swallows_top_level_script_errors() {
            let (_d, script) = project_with_main(r#"function _config( -- broken"#);
            let bundle = Bundle::new();
            assert_eq!(
                resolve_for_export(&script, "snake", &bundle),
                "com.usagiengine.snake",
            );
        }
    }
}
