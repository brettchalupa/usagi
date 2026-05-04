//! User-visible engine config returned by `_config()`. Loaded once
//! per session at startup (and once per `usagi export` invocation),
//! and consumed by every other module that cares about
//! project-level settings (window title, save namespace, app icon,
//! pixel-perfect scaling).
//!
//! Two read paths share the same field-extraction logic:
//!
//! - **Runtime:** `Config::read_from_lua` against the live session
//!   Lua VM. Errors flow into `last_error` for the on-screen overlay.
//! - **Export:** `Config::read_for_export` spins up a throwaway VM,
//!   loads `main.lua`, calls `_config()`, and reads the same fields.
//!   Failures are silent so the export path can keep going with
//!   defaults rather than abort over a broken `_config()`.
//!
//! Centralizing here keeps the spin-up-a-VM-just-to-read-one-field
//! sprawl from creeping into `game_id`, `icon`, `macos_app`, etc.
//! `usagi export` reads the config once and passes the resulting
//! struct down to every consumer.

use mlua::prelude::*;

/// Fully-resolved project config, with defaults filled in for any
/// fields `_config()` didn't set.
#[derive(Debug, Clone, Default)]
pub struct Config {
    /// Display name from `_config().name`. Resolved (with the project
    /// directory as fallback) by `crate::project_name::ProjectName`.
    pub name: Option<String>,
    /// When `true`, the render target upscales at integer multiples
    /// only with letterbox bars filling any leftover window space.
    /// When `false` (default) the game fills the window while
    /// preserving aspect ratio, so bars only show on the axis with
    /// extra room.
    pub pixel_perfect: bool,
    /// Reverse-DNS id like `com.brettmakesgames.snake`. Optional;
    /// `GameId::resolve` falls back to a project-name-derived id
    /// when missing.
    pub game_id: Option<String>,
    /// 1-based tile index into `sprites.png` (same indexing as
    /// `gfx.spr`). `None` means "use the embedded usagi default
    /// icon".
    pub icon: Option<u32>,
}

impl Config {
    /// Reads `_config()` out of an already-running Lua VM. Missing
    /// `_config()` returns defaults; a `_config()` that raises or
    /// returns a non-table fills `error_sink` (when `Some`) and
    /// also returns defaults. Per-field misses silently keep the
    /// default for that field, matching the runtime overlay's
    /// "broken config doesn't tear the session down" stance.
    pub fn read_from_lua(lua: &Lua, error_sink: Option<&mut Option<String>>) -> Self {
        let mut config = Self::default();
        let Ok(config_fn) = lua.globals().get::<LuaFunction>("_config") else {
            return config;
        };
        match config_fn.call::<LuaTable>(()) {
            Ok(tbl) => {
                // Use `Option<T>` so missing fields stay None (and the
                // Default value sticks). Reading a bool field directly
                // would coerce a missing/nil value to `false`, silently
                // overriding the default.
                if let Ok(Some(t)) = tbl.get::<Option<String>>("name") {
                    config.name = Some(t);
                }
                if let Ok(Some(t)) = tbl.get::<Option<bool>>("pixel_perfect") {
                    config.pixel_perfect = t;
                }
                if let Ok(Some(t)) = tbl.get::<Option<String>>("game_id") {
                    config.game_id = Some(t);
                }
                if let Ok(Some(n)) = tbl.get::<Option<u32>>("icon") {
                    config.icon = Some(n);
                }
            }
            Err(e) => {
                let msg = format!("_config: {}", e);
                crate::msg::err!("{}", msg);
                if let Some(sink) = error_sink {
                    *sink = Some(msg);
                }
            }
        }
        config
    }

    /// Reads the project's config off-thread of any running session,
    /// for export-time consumers (`game_id::resolve_for_export`,
    /// `icon::resolve_icns_for_export`, future bundle metadata).
    /// Spins up a throwaway Lua VM, runs `main.lua`, and pulls the
    /// `_config()` table. Any failure (script load error,
    /// `_config()` raising, etc.) returns `Self::default()` so the
    /// export keeps moving rather than aborting over a broken
    /// project file.
    #[cfg(not(target_os = "emscripten"))]
    pub fn read_for_export(script_path: &std::path::Path) -> Self {
        use crate::api::setup_api;
        use crate::assets::{install_require, load_script};
        use crate::vfs::{FsBacked, VirtualFs};
        use std::rc::Rc;

        let vfs: Rc<dyn VirtualFs> = Rc::new(FsBacked::from_script_path(script_path));
        let lua = Lua::new();
        if setup_api(&lua, false).is_err() {
            return Self::default();
        }
        if install_require(&lua, vfs.clone()).is_err() {
            return Self::default();
        }
        if load_script(&lua, vfs.as_ref()).is_err() {
            return Self::default();
        }
        Self::read_from_lua(&lua, None)
    }
}
