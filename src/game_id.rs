//! Stable identifier for a game project. Same string namespaces save data
//! (`save.rs`), the macOS CFBundleIdentifier (`macos_app.rs`), capture
//! filenames (`capture.rs`), and any future per-game preference. Anywhere
//! a game needs a stable id, prefer this resolver over baking in the
//! project name.
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
//! `GameId` wraps the resolved string so callers get methods (`as_str`,
//! `short_name`) instead of free functions sprinkled across modules, and
//! function signatures that take `&GameId` document intent better than
//! `&str` (you can tell a "game id" param from any random string).

use crate::bundle::Bundle;
use crate::save::validate_game_id;
use sha2::{Digest, Sha256};

/// Stable per-game identifier. Hold one of these on the session and
/// pass `&GameId` to anything that namespaces by game (saves,
/// settings, captures). Wraps a validated string; the only legal
/// constructors are `resolve` / `resolve_for_export`, which run the
/// layered fallback chain so the inner value is always one of:
/// an explicitly-validated `_config().game_id`, a sanitized
/// `com.usagiengine.<name>`, an auto-hashed `com.usagiengine.auto<hex>`,
/// or the `com.usagiengine.unknown` sentinel.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GameId(String);

impl GameId {
    /// Resolves the best-available identifier given whatever inputs the
    /// caller has on hand. Walks the layered fallback chain documented
    /// at the module level.
    pub fn resolve(
        explicit: Option<&str>,
        name_hint: Option<&str>,
        bundle: Option<&Bundle>,
    ) -> Self {
        if let Some(id) = from_explicit(explicit) {
            return Self(id);
        }
        if let Some(id) = name_hint.and_then(from_name) {
            return Self(id);
        }
        if let Some(b) = bundle {
            return Self(from_bundle_hash(b));
        }
        Self("com.usagiengine.unknown".to_string())
    }

    /// Wraps an already-validated id string without running the
    /// resolver fallback chain. Returns `None` if the input fails
    /// `save::validate_game_id`. Used by tools / external surfaces
    /// that have an explicit id and want to either use it as-is or
    /// surface "no valid id", rather than fall back to a sentinel.
    pub fn try_from_explicit(id: &str) -> Option<Self> {
        validate_game_id(id).ok()?;
        Some(Self(id.to_string()))
    }

    /// Borrow the raw id string. Use sparingly; prefer methods on
    /// `GameId` and `&GameId`-taking APIs in `save` / `settings`.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Friendly short name extracted from the resolved id, suitable for
    /// use as a filename prefix on capture artifacts (gif, png, etc).
    /// Returns the last dot-separated segment so
    /// `com.brettmakesgames.snake` becomes `snake`. Substitutes `usagi`
    /// for the `com.usagiengine.unknown` sentinel because
    /// `unknown-20260101.gif` reads worse than `usagi-20260101.gif`.
    ///
    /// The id is always restricted to filesystem-safe characters by the
    /// resolver, so the returned slice is always a usable filename
    /// component without further sanitization.
    pub fn short_name(&self) -> &str {
        let last = self.0.rsplit('.').next().unwrap_or("");
        if last.is_empty() || last == "unknown" {
            "usagi"
        } else {
            last
        }
    }
}

impl AsRef<str> for GameId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for GameId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Convenience entry for `usagi export`. Resolves the game id from a
/// pre-read `Config` (so the export path doesn't spin up its own
/// throwaway Lua VM just for this one field) plus the bundle for
/// the hash fallback. Native-only because export itself is.
#[cfg(not(target_os = "emscripten"))]
pub fn resolve_for_export(config: &crate::config::Config, name: &str, bundle: &Bundle) -> GameId {
    GameId::resolve(config.game_id.as_deref(), Some(name), Some(bundle))
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
    fn sanitize_rewrites_disallowed_and_trims_separators() {
        assert_eq!(sanitize("My Game!"), "my-game");
        assert_eq!(sanitize("a/b.c"), "a-b-c");
        assert_eq!(sanitize("--game--"), "game");
        assert_eq!(sanitize("__game__"), "game");
    }

    #[test]
    fn sanitize_returns_empty_for_pure_punctuation() {
        assert_eq!(sanitize("!!!"), "");
    }

    #[test]
    fn resolve_prefers_explicit_when_valid() {
        let gid = GameId::resolve(Some("com.example.game"), Some("hint"), None);
        assert_eq!(gid.as_str(), "com.example.game");
    }

    #[test]
    fn resolve_falls_through_invalid_explicit_to_name_hint() {
        let gid = GameId::resolve(Some(""), Some("snake"), None);
        assert_eq!(gid.as_str(), "com.usagiengine.snake");
    }

    #[test]
    fn resolve_uses_unknown_sentinel_when_all_inputs_missing() {
        let gid = GameId::resolve(None, None, None);
        assert_eq!(gid.as_str(), "com.usagiengine.unknown");
    }

    #[test]
    fn short_name_returns_last_dot_segment() {
        assert_eq!(
            GameId::resolve(Some("com.brettmakesgames.snake"), None, None).short_name(),
            "snake"
        );
        assert_eq!(
            GameId::resolve(Some("com.usagiengine.notetris"), None, None).short_name(),
            "notetris"
        );
    }

    #[test]
    fn short_name_substitutes_usagi_for_unknown_sentinel() {
        let gid = GameId::resolve(None, None, None);
        assert_eq!(gid.short_name(), "usagi");
    }

    #[test]
    fn short_name_handles_id_without_dots() {
        let gid = GameId::resolve(Some("snake"), None, None);
        assert_eq!(gid.short_name(), "snake");
    }

    #[test]
    fn try_from_explicit_accepts_valid_id() {
        assert!(GameId::try_from_explicit("com.test.foo").is_some());
    }

    #[test]
    fn try_from_explicit_rejects_path_traversal() {
        assert!(GameId::try_from_explicit("../bad").is_none());
        assert!(GameId::try_from_explicit("a/b").is_none());
    }

    #[test]
    fn try_from_explicit_rejects_empty() {
        assert!(GameId::try_from_explicit("").is_none());
    }

    #[test]
    fn from_explicit_keeps_valid_id_verbatim() {
        assert_eq!(
            from_explicit(Some("com.test.foo")).as_deref(),
            Some("com.test.foo"),
        );
    }

    #[test]
    fn from_explicit_returns_none_for_invalid() {
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
    fn resolve_uses_name_when_no_explicit() {
        let gid = GameId::resolve(None, Some("snake"), None);
        assert_eq!(gid.as_str(), "com.usagiengine.snake");
    }

    #[test]
    fn resolve_uses_bundle_hash_when_no_useful_name() {
        let mut b = Bundle::new();
        b.insert("main.lua", b"-- contents".to_vec());
        let gid = GameId::resolve(None, Some("!!!"), Some(&b));
        assert!(gid.as_str().starts_with("com.usagiengine.auto"));
    }

    #[test]
    fn resolve_uses_bundle_hash_when_no_name_hint() {
        let mut b = Bundle::new();
        b.insert("main.lua", b"-- contents".to_vec());
        let gid = GameId::resolve(None, None, Some(&b));
        assert!(gid.as_str().starts_with("com.usagiengine.auto"));
    }

    #[cfg(not(target_os = "emscripten"))]
    mod export_path {
        use super::super::*;
        use std::fs;
        use tempfile::tempdir;

        /// Stages a temp project with the given main.lua body and
        /// reads its `_config()` table once, returning the
        /// `Config` the export-time resolvers operate on. Wraps the
        /// throwaway-VM dance in a single call so each test reads
        /// like `read → resolve → assert` without re-spinning Lua.
        fn config_from_main(body: &str) -> (tempfile::TempDir, crate::config::Config) {
            let dir = tempdir().unwrap();
            let script = dir.path().join("main.lua");
            fs::write(&script, body).unwrap();
            let cfg = crate::config::Config::read_for_export(&script);
            (dir, cfg)
        }

        #[test]
        fn resolve_for_export_reads_explicit_game_id_from_config() {
            let (_d, cfg) =
                config_from_main(r#"function _config() return { game_id = "com.test.foo" } end"#);
            let bundle = Bundle::new();
            assert_eq!(
                resolve_for_export(&cfg, "ignored", &bundle).as_str(),
                "com.test.foo",
            );
        }

        #[test]
        fn resolve_for_export_falls_through_invalid_explicit_to_name() {
            let (_d, cfg) =
                config_from_main(r#"function _config() return { game_id = "../bad" } end"#);
            let bundle = Bundle::new();
            assert_eq!(
                resolve_for_export(&cfg, "snake", &bundle).as_str(),
                "com.usagiengine.snake",
            );
        }

        #[test]
        fn resolve_for_export_falls_through_missing_config_to_name() {
            let (_d, cfg) = config_from_main(r#"-- no _config defined"#);
            let bundle = Bundle::new();
            assert_eq!(
                resolve_for_export(&cfg, "snake", &bundle).as_str(),
                "com.usagiengine.snake",
            );
        }

        #[test]
        fn resolve_for_export_falls_through_to_bundle_hash_when_name_empty() {
            let (_d, cfg) = config_from_main(r#"-- no _config defined"#);
            let mut bundle = Bundle::new();
            bundle.insert("main.lua", b"-- contents".to_vec());
            let id = resolve_for_export(&cfg, "!!!", &bundle);
            assert!(id.as_str().starts_with("com.usagiengine.auto"));
        }

        #[test]
        fn resolve_for_export_swallows_top_level_script_errors() {
            let (_d, cfg) = config_from_main(r#"function _config( -- broken"#);
            let bundle = Bundle::new();
            assert_eq!(
                resolve_for_export(&cfg, "snake", &bundle).as_str(),
                "com.usagiengine.snake",
            );
        }
    }
}
