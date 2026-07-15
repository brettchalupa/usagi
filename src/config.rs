//! User-visible engine config. Loaded once per session at startup (and
//! once per `usagi export` / `usagi tools` invocation), and consumed by
//! every other module that cares about project-level settings (window
//! title, save namespace, app icon, pixel-perfect scaling).
//!
//! Three sources feed one `Config`, highest precedence first:
//!
//! 1. **Frontmatter** comments at the top of `main.lua` (`-- name = Foo`).
//!    Read as plain text, no Lua VM, so single-file games and dev tools
//!    can still configure themselves.
//! 2. **`usagi.conf`** at the project root, the same `key = value` format.
//!    Recommended for larger games; easy for external tools to parse.
//! 3. **`_config()`** returned from the game's Lua. Deprecated (it forces
//!    a VM boot to read config and has a load-order footgun where
//!    top-level `usagi.GAME_W` reads the default before `_config` runs).
//!    Still honored, with a deprecation warning.
//!
//! Sources merge per-field: a field set by more than one source takes the
//! highest-precedence value and logs a conflict warning. Missing fields
//! fall through to the engine default.
//!
//! Two read paths share the field-extraction logic:
//!
//! - **Runtime:** `Config::read_from_lua` against the live session Lua VM
//!   plus the session VFS. Errors flow into `last_error` for the on-screen
//!   overlay.
//! - **Export/tools:** `Config::read_for_export` reads the text sources
//!   directly; only when neither is present does it spin up a throwaway VM
//!   to read a deprecated `_config()`.

use crate::vfs::VirtualFs;
use mlua::prelude::*;

/// Game render dimensions in pixels. Travels as a unit through every
/// pipeline step (window sizing, RT creation, view transform, capture,
/// pause-menu layout) so call sites can't accidentally swap the two
/// floats.
#[derive(Debug, Clone, Copy)]
pub struct Resolution {
    pub w: f32,
    pub h: f32,
}

impl Resolution {
    /// Engine default. Mirrored into Lua as `usagi.GAME_W` /
    /// `usagi.GAME_H` when `game_width` / `game_height` are not set.
    pub const DEFAULT: Self = Self { w: 320.0, h: 180.0 };
}

impl Default for Resolution {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Default cell size, in pixels, of one tile in `sprites.png` when
/// `sprite_size` isn't set. Mirrored into Lua as `usagi.SPRITE_SIZE`.
/// Drives `gfx.spr` indexing, the tile-picker tool's grid, and the
/// window-icon slicer.
pub const DEFAULT_SPRITE_SIZE: i32 = 16;

/// Fully-resolved project config, with defaults filled in for any
/// fields no source set.
#[derive(Debug, Clone)]
pub struct Config {
    /// Display name from `name`. Resolved (with the project directory as
    /// fallback) by `crate::project_name::ProjectName`.
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
    /// Game render dimensions, defaulting to 320x180. Set via
    /// `game_width` / `game_height`. The internal RT is sized to this;
    /// the window upscales to fit, preserving aspect ratio. Tested
    /// range is roughly 160..640 on either axis; pause-menu and tools
    /// UI may overflow or look sparse outside that band.
    pub resolution: Resolution,
    /// Side length, in pixels, of one cell in `sprites.png`. Defaults
    /// to 16. Set via `sprite_size`. Drives `gfx.spr` indexing, the
    /// tile-picker tool's grid, and the window-icon slicer. The bundled
    /// `sprites.png` must use a multiple of this value on both axes;
    /// mismatches fall back to the default icon for the window-icon path.
    pub sprite_size: i32,
    /// When `true` (default) the engine intercepts Esc / P / Enter /
    /// gamepad Start to open its built-in pause menu. When `false` via
    /// `pause_menu = false`, those keys flow through to user code so
    /// games can roll their own menu with the existing `usagi.menu_item`,
    /// `usagi.toggle_fullscreen`, `usagi.quit`, and `input.key_*` APIs.
    /// Disabling also silences the Test / Configure Keys / Configure
    /// Gamepad screens, since they're sub-views of the same overlay.
    pub pause_menu: bool,
    /// Launch fullscreen by default. Only the default: once a player toggles
    /// fullscreen the saved setting wins. Set via `initial_fullscreen`.
    pub initial_fullscreen: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            name: None,
            pixel_perfect: false,
            game_id: None,
            icon: None,
            resolution: Resolution::DEFAULT,
            sprite_size: DEFAULT_SPRITE_SIZE,
            pause_menu: true,
            initial_fullscreen: false,
        }
    }
}

/// One config source's contribution: every field optional so "unset"
/// stays distinct from "set to the default value" during the merge.
#[derive(Debug, Default, Clone)]
struct Partial {
    name: Option<String>,
    pixel_perfect: Option<bool>,
    game_id: Option<String>,
    icon: Option<u32>,
    game_width: Option<f32>,
    game_height: Option<f32>,
    sprite_size: Option<i32>,
    pause_menu: Option<bool>,
    initial_fullscreen: Option<bool>,
}

impl Partial {
    /// Reads a `_config()` table. Per-field misses stay `None`; invalid
    /// values (non-positive dimensions) drop silently, matching the old
    /// runtime behavior of keeping the default rather than erroring.
    fn from_lua_table(tbl: &LuaTable) -> Self {
        let mut p = Self::default();
        if let Ok(Some(t)) = tbl.get::<Option<String>>("name") {
            p.name = Some(t);
        }
        if let Ok(Some(t)) = tbl.get::<Option<bool>>("pixel_perfect") {
            p.pixel_perfect = Some(t);
        }
        if let Ok(Some(t)) = tbl.get::<Option<String>>("game_id") {
            p.game_id = Some(t);
        }
        if let Ok(Some(n)) = tbl.get::<Option<u32>>("icon") {
            p.icon = Some(n);
        }
        if let Ok(Some(w)) = tbl.get::<Option<f32>>("game_width")
            && w >= 1.0
        {
            p.game_width = Some(w);
        }
        if let Ok(Some(h)) = tbl.get::<Option<f32>>("game_height")
            && h >= 1.0
        {
            p.game_height = Some(h);
        }
        if let Ok(Some(s)) = tbl.get::<Option<i32>>("sprite_size")
            && s >= 1
        {
            p.sprite_size = Some(s);
        }
        if let Ok(Some(b)) = tbl.get::<Option<bool>>("pause_menu") {
            p.pause_menu = Some(b);
        }
        if let Ok(Some(b)) = tbl.get::<Option<bool>>("initial_fullscreen") {
            p.initial_fullscreen = Some(b);
        }
        p
    }
}

/// Which text source a `key = value` pair came from. Only affects
/// warnings: `usagi.conf` is a dedicated config file, so bad values and
/// unknown keys warn; frontmatter shares the file with prose comments
/// that may legitimately contain `=`, so it parses quietly.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Source {
    Frontmatter,
    Conf,
}

/// Numeric field parser: parse to `$t`, accept only values satisfying
/// `$ok`, else warn. `warn` is true only for `usagi.conf` (frontmatter
/// parses quietly, see `apply_pair`), so the message names it directly.
macro_rules! num_parser {
    ($name:ident, $t:ty, $n:ident => $ok:expr, $expects:literal) => {
        fn $name(value: &str, key: &str, warn: bool) -> Option<$t> {
            match value.parse::<$t>() {
                Ok($n) if $ok => Some($n),
                _ => {
                    if warn {
                        crate::msg::warn!(
                            "usagi.conf: '{key}' expects {}, got '{value}'",
                            $expects
                        );
                    }
                    None
                }
            }
        }
    };
}

// `is_finite` rejects `inf`/`nan`; `>= 1.0` rejects zero and negatives.
num_parser!(parse_dim, f32, n => n.is_finite() && n >= 1.0, "a number >= 1");
num_parser!(parse_size, i32, n => n >= 1, "an integer >= 1");

fn parse_index(value: &str, key: &str, warn: bool) -> Option<u32> {
    match value.parse::<u32>() {
        Ok(n) => Some(n),
        Err(_) => {
            if warn {
                crate::msg::warn!("usagi.conf: '{key}' expects a whole number, got '{value}'");
            }
            None
        }
    }
}

fn parse_bool(value: &str, key: &str, warn: bool) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => {
            if warn {
                crate::msg::warn!("usagi.conf: '{key}' expects true or false, got '{value}'");
            }
            None
        }
    }
}

/// Applies one `key = value` pair to a partial, coercing per the field's
/// type. Empty values are ignored so `name =` falls through to a lower
/// source rather than clobbering it with an empty string.
fn apply_pair(p: &mut Partial, key: &str, value: &str, source: Source, warn: bool) {
    if value.is_empty() {
        return;
    }
    // Only usagi.conf warns; frontmatter may hold prose that looks like config.
    let warn = warn && source == Source::Conf;
    match key {
        "name" => p.name = Some(value.to_string()),
        "game_id" => p.game_id = Some(value.to_string()),
        "pixel_perfect" => p.pixel_perfect = parse_bool(value, key, warn),
        "pause_menu" => p.pause_menu = parse_bool(value, key, warn),
        "initial_fullscreen" => p.initial_fullscreen = parse_bool(value, key, warn),
        "game_width" => p.game_width = parse_dim(value, key, warn),
        "game_height" => p.game_height = parse_dim(value, key, warn),
        "sprite_size" => p.sprite_size = parse_size(value, key, warn),
        "icon" => p.icon = parse_index(value, key, warn),
        _ => {
            if warn {
                crate::msg::warn!("usagi.conf: unknown key '{key}'");
            }
        }
    }
}

/// Extracts config from the leading comment block of a Lua file. Reads
/// line by line until the first line that isn't a `--` comment (a blank
/// line ends it too). Comment lines without `=` are prose and ignored,
/// so descriptions can sit alongside `key = value` lines.
fn frontmatter_partial(script: &str, warn: bool) -> Partial {
    let mut p = Partial::default();
    for line in script.lines() {
        let Some(rest) = line.trim_start().strip_prefix("--") else {
            break;
        };
        let rest = rest.trim();
        if let Some((k, v)) = rest.split_once('=') {
            apply_pair(&mut p, k.trim(), v.trim(), Source::Frontmatter, warn);
        }
    }
    p
}

/// Parses a `usagi.conf` file. Blank lines and `#` full-line comments are
/// skipped; everything else must be `key = value`.
fn conf_partial(text: &str, warn: bool) -> Partial {
    let mut p = Partial::default();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match line.split_once('=') {
            Some((k, v)) => apply_pair(&mut p, k.trim(), v.trim(), Source::Conf, warn),
            None => {
                if warn {
                    crate::msg::warn!("usagi.conf: ignoring malformed line '{line}'");
                }
            }
        }
    }
    p
}

/// Drops a leading UTF-8 BOM so a byte-order mark some editors prepend
/// doesn't hide the first frontmatter/conf line.
fn strip_bom(s: &str) -> &str {
    s.strip_prefix('\u{feff}').unwrap_or(s)
}

fn frontmatter_from_vfs(vfs: &dyn VirtualFs, warn: bool) -> Partial {
    match vfs.read_script() {
        Some(bytes) => {
            let text = String::from_utf8_lossy(&bytes);
            frontmatter_partial(strip_bom(&text), warn)
        }
        None => Partial::default(),
    }
}

fn conf_from_vfs(vfs: &dyn VirtualFs, warn: bool) -> Partial {
    match vfs.read_file("usagi.conf") {
        Some(bytes) => {
            let text = String::from_utf8_lossy(&bytes);
            conf_partial(strip_bom(&text), warn)
        }
        None => Partial::default(),
    }
}

/// Picks the highest-precedence value for one field (frontmatter, then
/// `usagi.conf`, then `_config`), warning when more than one source set
/// it so the winner isn't a silent surprise.
fn pick<T: Clone>(
    field: &str,
    fm: Option<T>,
    conf: Option<T>,
    lua: Option<T>,
    warn: bool,
) -> Option<T> {
    if warn {
        let sources: Vec<&str> = [
            fm.as_ref().map(|_| "frontmatter"),
            conf.as_ref().map(|_| "usagi.conf"),
            lua.as_ref().map(|_| "_config"),
        ]
        .into_iter()
        .flatten()
        .collect();
        if sources.len() >= 2 {
            crate::msg::warn!(
                "config: '{field}' set by {}; using {}",
                sources.join(" and "),
                sources[0]
            );
        }
    }
    fm.or(conf).or(lua)
}

/// Merges the three sources into a resolved `Config`. `warn` gates the
/// conflict/invalid-value logging so the pre-load global seed can run
/// quietly and leave the authoritative logging to the full read.
fn combine(lua: Option<Partial>, conf: Partial, fm: Partial, warn: bool) -> Config {
    let lua = lua.unwrap_or_default();
    // Resolve one field across the three sources, highest precedence first.
    macro_rules! pick {
        ($field:ident) => {
            pick(stringify!($field), fm.$field, conf.$field, lua.$field, warn)
        };
    }
    let mut c = Config {
        name: pick!(name),
        game_id: pick!(game_id),
        icon: pick!(icon),
        ..Default::default()
    };
    // Scalar fields keep the engine default when no source set them.
    macro_rules! merge {
        ($field:ident => $target:expr) => {
            if let Some(v) = pick!($field) {
                $target = v;
            }
        };
    }
    merge!(pixel_perfect => c.pixel_perfect);
    merge!(pause_menu => c.pause_menu);
    merge!(initial_fullscreen => c.initial_fullscreen);
    merge!(game_width => c.resolution.w);
    merge!(game_height => c.resolution.h);
    merge!(sprite_size => c.sprite_size);
    c
}

/// Reads a `_config()` partial from a live VM. Missing `_config` returns
/// `None`; a `_config` that raises or returns a non-table fills
/// `error_sink` (when `Some`) and returns `None`. Emits the deprecation
/// warning whenever the function is present.
fn lua_partial(lua: &Lua, error_sink: Option<&mut Option<String>>) -> Option<Partial> {
    let config_fn = lua.globals().get::<LuaFunction>("_config").ok()?;
    crate::msg::warn!(
        "_config() is deprecated; move settings to usagi.conf or frontmatter comments"
    );
    match config_fn.call::<LuaTable>(()) {
        Ok(tbl) => Some(Partial::from_lua_table(&tbl)),
        Err(e) => {
            let msg = format!("_config: {}", e);
            crate::msg::err!("{}", msg);
            if let Some(sink) = error_sink {
                *sink = Some(msg);
            }
            None
        }
    }
}

/// Frontmatter + `usagi.conf` partials, parsed once. Held opaquely by the
/// session so the pre-`load_script` dimension seed and the authoritative
/// post-load resolve share one read of the files.
#[derive(Debug, Default, Clone)]
pub struct TextSources {
    fm: Partial,
    conf: Partial,
}

impl Config {
    /// Reads and parses the text config sources (frontmatter + `usagi.conf`)
    /// once. The session reuses the result for the pre-`load_script` seed and
    /// the authoritative post-load resolve, so the files aren't parsed twice
    /// per boot. `warn` reports bad values / unknown keys in `usagi.conf`.
    pub fn read_text_sources(vfs: &dyn VirtualFs, warn: bool) -> TextSources {
        TextSources {
            fm: frontmatter_from_vfs(vfs, warn),
            conf: conf_from_vfs(vfs, warn),
        }
    }

    /// Resolves config from text sources alone (no `_config`). Quiet, for the
    /// pre-`load_script` seed of `usagi.GAME_W/GAME_H/SPRITE_SIZE`; the
    /// authoritative `resolve` owns the conflict warnings.
    pub fn from_text(text: &TextSources) -> Self {
        combine(None, text.conf.clone(), text.fm.clone(), false)
    }

    /// Authoritative resolution: `_config()` from the live VM merged under
    /// the already-parsed text sources (frontmatter > usagi.conf > _config).
    /// Per-field conflicts and `_config` errors are logged; a broken source
    /// falls back to defaults rather than tearing the session down.
    pub fn resolve(lua: &Lua, text: &TextSources, error_sink: Option<&mut Option<String>>) -> Self {
        combine(
            lua_partial(lua, error_sink),
            text.conf.clone(),
            text.fm.clone(),
            true,
        )
    }

    /// Reads project config off-thread of any running session, for export-
    /// and tools-time consumers. Boots a throwaway Lua VM only when the
    /// script defines the deprecated `_config()`; otherwise the text sources
    /// are the whole story and no VM is needed. When `_config()` is present
    /// it's merged under the text sources, so export/tools resolve the same
    /// config the runtime does. Any failure returns `Self::default()` so the
    /// caller keeps moving.
    #[cfg(not(target_os = "emscripten"))]
    pub fn read_for_export(script_path: &std::path::Path) -> Self {
        use crate::api::{register_data_api, setup_api};
        use crate::assets::{install_require, load_script};
        use crate::vfs::FsBacked;
        use std::rc::Rc;

        let vfs: Rc<dyn VirtualFs> = Rc::new(FsBacked::from_script_path(script_path));
        let text = Self::read_text_sources(vfs.as_ref(), true);
        let has_config = vfs
            .read_script()
            .is_some_and(|b| String::from_utf8_lossy(&b).contains("_config"));
        if !has_config {
            return combine(None, text.conf.clone(), text.fm.clone(), true);
        }

        // Match the runtime: unsafe_new so user `_config()` code can use
        // `debug.*` without crashing the export step. See session.rs.
        let lua = unsafe { Lua::unsafe_new() };
        if setup_api(&lua, false).is_err()
            || install_require(&lua, vfs.clone()).is_err()
            || register_data_api(&lua, vfs.clone()).is_err()
            || load_script(&lua, vfs.as_ref()).is_err()
        {
            return Self::default();
        }
        Self::resolve(&lua, &text, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_reads_leading_comment_block() {
        let p = frontmatter_partial(
            "-- a little shmup\n-- name = NEOGEAR\n-- game_width = 320\n-- pixel_perfect = true\n\nw = usagi.GAME_W\n",
            false,
        );
        assert_eq!(p.name.as_deref(), Some("NEOGEAR"));
        assert_eq!(p.game_width, Some(320.0));
        assert_eq!(p.pixel_perfect, Some(true));
    }

    #[test]
    fn frontmatter_tolerates_spacing_around_equals() {
        assert_eq!(
            frontmatter_partial("--name=NEOGEAR\n", false)
                .name
                .as_deref(),
            Some("NEOGEAR")
        );
    }

    #[test]
    fn frontmatter_keeps_spaces_inside_value() {
        assert_eq!(
            frontmatter_partial("-- name = Foo Bar Biz\n", false)
                .name
                .as_deref(),
            Some("Foo Bar Biz")
        );
    }

    #[test]
    fn frontmatter_stops_at_first_non_comment_line() {
        // The blank line ends the block, so the second key is ignored.
        let p = frontmatter_partial("-- name = NEOGEAR\n\n-- game_width = 640\n", false);
        assert_eq!(p.name.as_deref(), Some("NEOGEAR"));
        assert_eq!(p.game_width, None);
    }

    #[test]
    fn frontmatter_ignores_luals_annotations() {
        // `---@meta` and prose comments have no `=` or an unknown key; the
        // real config line still lands.
        let p = frontmatter_partial("---@meta\n-- name = NEOGEAR\n", false);
        assert_eq!(p.name.as_deref(), Some("NEOGEAR"));
    }

    #[test]
    fn conf_skips_blank_and_hash_comment_lines() {
        let p = conf_partial("# my game\n\nname = NEOGEAR\nsprite_size = 8\n", false);
        assert_eq!(p.name.as_deref(), Some("NEOGEAR"));
        assert_eq!(p.sprite_size, Some(8));
    }

    #[test]
    fn empty_value_is_ignored() {
        assert!(conf_partial("name =\n", false).name.is_none());
    }

    #[test]
    fn invalid_numbers_drop_to_none() {
        let p = conf_partial("game_width = wide\nsprite_size = 0\n", false);
        assert_eq!(p.game_width, None);
        assert_eq!(p.sprite_size, None);
    }

    #[test]
    fn frontmatter_wins_over_conf_over_lua() {
        let fm = frontmatter_partial("-- name = FromFrontmatter\n", false);
        let conf = conf_partial("name = FromConf\ngame_id = com.conf.game\n", false);
        let lua = Partial {
            name: Some("FromLua".into()),
            sprite_size: Some(8),
            ..Partial::default()
        };
        let c = combine(Some(lua), conf, fm, false);
        assert_eq!(c.name.as_deref(), Some("FromFrontmatter"));
        assert_eq!(c.game_id.as_deref(), Some("com.conf.game"));
        assert_eq!(c.sprite_size, 8);
    }

    #[test]
    fn unset_fields_fall_back_to_defaults() {
        let c = combine(None, Partial::default(), Partial::default(), false);
        assert!(c.name.is_none());
        assert_eq!(c.resolution.w, Resolution::DEFAULT.w);
        assert_eq!(c.sprite_size, DEFAULT_SPRITE_SIZE);
        assert!(c.pause_menu);
    }

    #[test]
    fn non_finite_dimensions_are_rejected() {
        // `f32::parse` accepts inf/nan; they must not slip past validation.
        assert_eq!(conf_partial("game_width = inf\n", false).game_width, None);
        assert_eq!(conf_partial("game_height = nan\n", false).game_height, None);
    }

    #[test]
    fn leading_bom_does_not_hide_first_line() {
        assert_eq!(
            frontmatter_partial(strip_bom("\u{feff}-- name = NEOGEAR\n"), false)
                .name
                .as_deref(),
            Some("NEOGEAR")
        );
        assert_eq!(
            conf_partial(strip_bom("\u{feff}name = NEOGEAR\n"), false)
                .name
                .as_deref(),
            Some("NEOGEAR")
        );
    }

    #[test]
    fn read_text_merges_frontmatter_and_conf_through_vfs() {
        use crate::vfs::FsBacked;
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("main.lua"),
            "-- name = FromFrontmatter\nfunction _init() end\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("usagi.conf"),
            "name = FromConf\ngame_width = 200\n",
        )
        .unwrap();
        let vfs = FsBacked::from_script_path(&dir.path().join("main.lua"));
        let c = Config::from_text(&Config::read_text_sources(&vfs, false));
        // Frontmatter wins for name; usagi.conf fills game_width.
        assert_eq!(c.name.as_deref(), Some("FromFrontmatter"));
        assert_eq!(c.resolution.w, 200.0);
    }

    #[test]
    fn export_merges_config_under_text_sources() {
        // A mixed project (frontmatter + deprecated _config) must resolve the
        // same for export/tools as at runtime: frontmatter wins, _config fills
        // the rest, rather than _config being dropped.
        let dir = tempfile::tempdir().unwrap();
        let main = dir.path().join("main.lua");
        std::fs::write(
            &main,
            "-- name = FromFrontmatter\nfunction _config() return { game_id = \"com.x.y\", icon = 5 } end\n",
        )
        .unwrap();
        let c = Config::read_for_export(&main);
        assert_eq!(c.name.as_deref(), Some("FromFrontmatter"));
        assert_eq!(c.game_id.as_deref(), Some("com.x.y"));
        assert_eq!(c.icon, Some(5));
    }
}
