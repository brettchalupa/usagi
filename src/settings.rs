//! Per-game settings persisted to JSON. Storage matches `save.rs`:
//! native writes `settings.json` next to `save.json` in the per-game
//! OS data dir; web routes through `localStorage` under
//! `usagi.settings.<game_id>`. Load is best-effort (missing or
//! malformed blob falls back to defaults).

use crate::game_id::GameId;

/// First-boot master volume. Also the Shift+M unmute target.
pub const DEFAULT_VOLUME: f32 = 0.5;

/// First-boot fullscreen state. False so the player picks via Alt+Enter.
const DEFAULT_FULLSCREEN: bool = false;

#[cfg(not(target_os = "emscripten"))]
const SETTINGS_FILE: &str = "settings.json";

/// User-tunable settings, loaded once at session creation and held
/// on the session for hotkeys to read/mutate. JSON marshaling is
/// hand-rolled to avoid pulling `serde` as a direct dep.
#[derive(Debug, Clone)]
pub struct Settings {
    /// Master output volume, clamped to `0.0..=1.0` on apply.
    /// `0.0` is muted; Shift+M flips between `0.0` and `DEFAULT_VOLUME`.
    pub volume: f32,
    /// Borderless fullscreen state. Alt+Enter toggles and persists.
    pub fullscreen: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: DEFAULT_VOLUME,
            fullscreen: DEFAULT_FULLSCREEN,
        }
    }
}

/// Absolute path to `settings.json` for `game_id`. Native-only; on
/// web there's no file path, just a `localStorage` key.
#[cfg(not(target_os = "emscripten"))]
pub fn settings_path(game_id: &GameId) -> std::io::Result<std::path::PathBuf> {
    Ok(crate::save::save_dir(game_id)?.join(SETTINGS_FILE))
}

/// Loads stored settings. Returns defaults on any failure (missing,
/// parse error, IO error); errors log to stderr but never panic.
/// Unknown JSON keys are ignored for forward-compat.
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
        fullscreen: value
            .get("fullscreen")
            .and_then(|v| v.as_bool())
            .unwrap_or(defaults.fullscreen),
    }
}

/// Persists settings. Native writes are atomic (tempfile + rename);
/// web routes through the shared localStorage shim under
/// `usagi.settings.<game_id>`.
pub fn write(game_id: &GameId, settings: &Settings) -> std::io::Result<()> {
    let json = serde_json::json!({
        "volume": settings.volume,
        "fullscreen": settings.fullscreen,
    });
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
            fullscreen: value
                .get("fullscreen")
                .and_then(|v| v.as_bool())
                .unwrap_or(defaults.fullscreen),
        };
        assert_eq!(parsed.volume, 0.25);
        assert_eq!(parsed.fullscreen, defaults.fullscreen);
    }

    #[test]
    fn fullscreen_round_trips_through_json_shape() {
        let body = r#"{ "volume": 0.5, "fullscreen": true }"#;
        let value: serde_json::Value = serde_json::from_str(body).unwrap();
        let defaults = Settings::default();
        let parsed = Settings {
            volume: value
                .get("volume")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32)
                .unwrap_or(defaults.volume),
            fullscreen: value
                .get("fullscreen")
                .and_then(|v| v.as_bool())
                .unwrap_or(defaults.fullscreen),
        };
        assert!(parsed.fullscreen);
    }
}
