//! Per-game settings persisted to JSON. Initially just master volume;
//! the file's a forward foothold for future user-tunable knobs (audio
//! mix, key remap, display options) without churning the file
//! location later.
//!
//! Storage layout matches `save.rs`:
//!
//! - **Native:** `settings.json` next to `save.json` in the per-game
//!   OS data dir (`~/Library/Application Support/<game_id>/` on
//!   macOS, `%APPDATA%\<game_id>\` on Windows, `~/.local/share/...`
//!   on Linux).
//! - **Web:** `localStorage` under the key `usagi.settings.<game_id>`,
//!   alongside the `usagi.save.<game_id>` key, sharing the same JS
//!   storage shim that powers `usagi.save` / `usagi.load`.
//!
//! Load is best-effort: a missing or malformed settings blob falls
//! back to defaults rather than erroring, so a fresh install (or a
//! game that ships without settings) Just Works. Native writes are
//! atomic via tempfile + rename so a crash mid-write can't leave a
//! truncated file.

use crate::game_id::GameId;

/// Default master volume on first boot (no settings stored yet).
/// 0.5 is a comfortable mid-point that won't blast someone who
/// forgot to turn their speakers down before launching the game.
/// Also used as the unmute target: Shift+M from a muted state
/// restores volume to this value rather than tracking the user's
/// last preferred level. A future volume slider would just write
/// directly to the `volume` field, with 0.0 == mute, so this single
/// number stays the source of truth.
pub const DEFAULT_VOLUME: f32 = 0.5;

#[cfg(not(target_os = "emscripten"))]
const SETTINGS_FILE: &str = "settings.json";

/// User-tunable settings. Loaded once at session creation, applied to
/// the engine's audio device, and held on the session for the global
/// mute hotkey to reference. Public fields because this is an
/// internal Rust-side struct (no Lua binding yet, by design).
///
/// Hand-rolled JSON marshaling rather than serde derive: we don't
/// pull `serde` as a direct dep (only `serde_json`), and a single f32
/// field doesn't justify adding it. When this grows to many fields
/// it'll be worth revisiting.
#[derive(Debug, Clone)]
pub struct Settings {
    /// Master output volume in `0.0..=1.0`. `0.0` means muted; any
    /// positive value plays at that level. Out-of-range values are
    /// clamped on apply, so a hand-edited file can't blow speakers
    /// or silently disable audio. Mute toggles persist via this
    /// single field, so a future volume slider stays compatible:
    /// sliding up from `0.0` unmutes, sliding to `0.0` mutes.
    pub volume: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: DEFAULT_VOLUME,
        }
    }
}

/// Absolute path to `settings.json` for `game_id`. Native-only; on
/// web there's no file path, just a `localStorage` key.
#[cfg(not(target_os = "emscripten"))]
pub fn settings_path(game_id: &GameId) -> std::io::Result<std::path::PathBuf> {
    Ok(crate::save::save_dir(game_id)?.join(SETTINGS_FILE))
}

/// Loads stored settings for `game_id`. Returns defaults on any
/// failure (missing storage, parse error, IO error). Parse errors
/// are logged to stderr so a developer can see why their hand-edited
/// file didn't take effect, but a broken blob never tears down the
/// session. Unknown JSON keys are ignored so older builds can read
/// settings written by newer ones (forward-compatible).
pub fn load(game_id: &GameId) -> Settings {
    let body = match read_blob(game_id) {
        Ok(Some(s)) => s,
        Ok(None) => return Settings::default(),
        Err(e) => {
            eprintln!("[usagi] settings: read error: {e}; using defaults");
            return Settings::default();
        }
    };
    let value: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[usagi] settings: parse error: {e}; using defaults");
            return Settings::default();
        }
    };
    let defaults = Settings::default();
    Settings {
        volume: value
            .get("volume")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(defaults.volume),
    }
}

/// Persists settings for `game_id`. Native: atomic write via
/// tempfile + rename so a crash mid-write leaves the previous file
/// intact (same pattern `save::write_save` uses). Web: writes through
/// the shared `localStorage` shim under `usagi.settings.<game_id>`.
/// Called by the Shift+M mute toggle so the new volume sticks across
/// quit/relaunch.
pub fn write(game_id: &GameId, settings: &Settings) -> std::io::Result<()> {
    let json = serde_json::json!({ "volume": settings.volume });
    let body = serde_json::to_string_pretty(&json)
        .map_err(|e| std::io::Error::other(format!("serialize settings: {e}")))?;
    write_blob(game_id, &body)
}

#[cfg(not(target_os = "emscripten"))]
fn read_blob(game_id: &GameId) -> std::io::Result<Option<String>> {
    let path = settings_path(game_id)?;
    match std::fs::read_to_string(&path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(not(target_os = "emscripten"))]
fn write_blob(game_id: &GameId, body: &str) -> std::io::Result<()> {
    let path = settings_path(game_id)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

#[cfg(target_os = "emscripten")]
fn read_blob(game_id: &GameId) -> std::io::Result<Option<String>> {
    crate::save::kv_read(&format!("usagi.settings.{}", game_id.as_str()))
}

#[cfg(target_os = "emscripten")]
fn write_blob(game_id: &GameId, body: &str) -> std::io::Result<()> {
    crate::save::kv_write(&format!("usagi.settings.{}", game_id.as_str()), body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_volume_is_half() {
        assert_eq!(Settings::default().volume, 0.5);
    }

    #[test]
    fn load_returns_default_for_missing_game_id() {
        // Use a game_id that's extremely unlikely to have a real
        // settings.json (or localStorage entry on web) on the test
        // runner.
        let gid = GameId::resolve(Some("com.usagiengine.test-missing-settings"), None, None);
        let s = load(&gid);
        assert_eq!(s.volume, 0.5);
    }

    #[test]
    fn unknown_keys_are_ignored() {
        // Forward-compat: a settings.json written by a newer build
        // that adds fields shouldn't break this build's load path,
        // just fall back to defaults for the missing fields.
        let body = r#"{ "volume": 0.25, "future_field": "hello" }"#;
        let value: serde_json::Value = serde_json::from_str(body).unwrap();
        let defaults = Settings::default();
        let parsed = Settings {
            volume: value
                .get("volume")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32)
                .unwrap_or(defaults.volume),
        };
        assert_eq!(parsed.volume, 0.25);
    }
}
