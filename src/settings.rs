//! Per-game settings persisted to JSON. Storage matches `save.rs`:
//! native writes `settings.json` next to `save.json` in the per-game
//! OS data dir; web routes through `localStorage` under
//! `usagi.settings.<game_id>`. Load is best-effort (missing or
//! malformed blob falls back to defaults).

use crate::game_id::GameId;

/// First-boot music volume. Also the Shift+M unmute target for music.
pub const DEFAULT_MUSIC_VOLUME: f32 = 0.8;

/// First-boot sfx volume. Also the Shift+M unmute target for sfx.
pub const DEFAULT_SFX_VOLUME: f32 = 0.8;

/// First-boot fullscreen state. False so the player picks via Alt+Enter.
const DEFAULT_FULLSCREEN: bool = false;

#[cfg(not(target_os = "emscripten"))]
const SETTINGS_FILE: &str = "settings.json";

/// User-tunable settings, loaded once at session creation and held
/// on the session for hotkeys to read/mutate. JSON marshaling is
/// hand-rolled to avoid pulling `serde` as a direct dep.
#[derive(Debug, Clone)]
pub struct Settings {
    /// Music output volume, clamped to `0.0..=1.0` on apply.
    /// `0.0` is muted; Shift+M flips between `0.0` and the defaults.
    pub music_volume: f32,
    /// SFX output volume, clamped to `0.0..=1.0` on apply.
    pub sfx_volume: f32,
    /// Borderless fullscreen state. Alt+Enter toggles and persists.
    pub fullscreen: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            music_volume: DEFAULT_MUSIC_VOLUME,
            sfx_volume: DEFAULT_SFX_VOLUME,
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
    parse(&value)
}

// `volume` is the legacy single-channel key written by Usagi <= v0.4.0.
// On load, missing `music_volume` / `sfx_volume` fields fall back to
// this so users don't lose their preferences when they update.
fn parse(value: &serde_json::Value) -> Settings {
    let defaults = Settings::default();
    let legacy = value
        .get("volume")
        .and_then(|v| v.as_f64())
        .map(|v| v as f32);
    let read_f32 = |key: &str| value.get(key).and_then(|v| v.as_f64()).map(|v| v as f32);
    Settings {
        music_volume: read_f32("music_volume")
            .or(legacy)
            .unwrap_or(defaults.music_volume),
        sfx_volume: read_f32("sfx_volume")
            .or(legacy)
            .unwrap_or(defaults.sfx_volume),
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
        "music_volume": settings.music_volume,
        "sfx_volume": settings.sfx_volume,
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
    fn default_volumes_are_eighty_percent() {
        let s = Settings::default();
        assert_eq!(s.music_volume, 0.8);
        assert_eq!(s.sfx_volume, 0.8);
    }

    #[test]
    fn load_returns_default_for_missing_game_id() {
        let gid = GameId::resolve(Some("com.usagiengine.test-missing-settings"), None, None);
        let s = load(&gid);
        assert_eq!(s.music_volume, DEFAULT_MUSIC_VOLUME);
        assert_eq!(s.sfx_volume, DEFAULT_SFX_VOLUME);
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let body = r#"{ "music_volume": 0.25, "sfx_volume": 0.5, "future_field": "hello" }"#;
        let value: serde_json::Value = serde_json::from_str(body).unwrap();
        let parsed = parse(&value);
        assert_eq!(parsed.music_volume, 0.25);
        assert_eq!(parsed.sfx_volume, 0.5);
        assert!(!parsed.fullscreen);
    }

    #[test]
    fn fullscreen_round_trips_through_json_shape() {
        let body = r#"{ "music_volume": 0.5, "sfx_volume": 0.5, "fullscreen": true }"#;
        let value: serde_json::Value = serde_json::from_str(body).unwrap();
        let parsed = parse(&value);
        assert!(parsed.fullscreen);
    }

    #[test]
    fn legacy_volume_key_populates_both_channels() {
        // Settings written by Usagi <= v0.4.0 only had a single `volume`
        // key. On load, both channels should pick that up so users
        // don't get reset to defaults on upgrade.
        let body = r#"{ "volume": 0.3, "fullscreen": true }"#;
        let value: serde_json::Value = serde_json::from_str(body).unwrap();
        let parsed = parse(&value);
        assert!((parsed.music_volume - 0.3).abs() < 1e-6);
        assert!((parsed.sfx_volume - 0.3).abs() < 1e-6);
        assert!(parsed.fullscreen);
    }

    #[test]
    fn legacy_volume_overridden_by_new_keys_when_present() {
        let body = r#"{ "volume": 0.3, "music_volume": 0.6, "sfx_volume": 1.0 }"#;
        let value: serde_json::Value = serde_json::from_str(body).unwrap();
        let parsed = parse(&value);
        assert!((parsed.music_volume - 0.6).abs() < 1e-6);
        assert!((parsed.sfx_volume - 1.0).abs() < 1e-6);
    }

    #[test]
    fn missing_volume_keys_fall_back_to_defaults() {
        let body = r#"{ "fullscreen": false }"#;
        let value: serde_json::Value = serde_json::from_str(body).unwrap();
        let parsed = parse(&value);
        assert_eq!(parsed.music_volume, DEFAULT_MUSIC_VOLUME);
        assert_eq!(parsed.sfx_volume, DEFAULT_SFX_VOLUME);
    }
}
