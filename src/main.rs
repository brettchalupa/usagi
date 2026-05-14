//! Usagi: rapid 2D game prototyping with Lua.
//!
//! The binary has two modes of operation:
//!
//! 1. **Normal mode** parses the CLI and dispatches to a subcommand
//!    (`run` / `dev` / `tools` / `templates` / `export` / `init`).
//! 2. **Fused mode** (when a `usagi export` output has appended a bundle)
//!    detects the bundle at startup and runs the embedded game directly,
//!    skipping the CLI entirely. This is how shipped game binaries work.
//!
//! On the web build (target_os = "emscripten") there is no CLI: the JS
//! shell fetches a `.usagi` bundle and writes it to `/game.usagi` in the
//! wasm virtual FS before `main()` runs, and that bundle is executed.

mod api;
mod assets;
mod bundle;
mod cli;
mod config;
mod effect;
mod error;
mod font;
#[cfg(not(target_os = "emscripten"))]
mod font_bake;
mod icon;
mod input;
mod msg;
mod palette;
mod pause;
mod pixels;
mod preprocess;
mod project_name;
mod render;
mod save;
mod session;
mod shader;
mod vfs;

// `tools` don't run on web
#[cfg(not(target_os = "emscripten"))]
mod tools;

// In-game capture: GIF recorder (F9 / Cmd+G) + PNG screenshots
// (F8 / Cmd+F). Both write to `<cwd>/captures/`. Native-only:
// emscripten has no real filesystem to write artifacts to.
#[cfg(not(target_os = "emscripten"))]
mod capture;

// Per-game settings (volume, future user knobs). Native: persisted to
// `settings.json` next to save.json in the OS data dir. Web: routed
// through localStorage under `usagi.settings.<game_id>`, sharing the
// same JS storage shim that powers `usagi.save` / `usagi.load`.
mod settings;

// Per-game keyboard remapping. Sits next to settings.json in the same
// per-game data dir; the pause menu's Configure Keys flow writes into
// it and `input::action_*` reads through it to override defaults.
mod keymap;

// Per-game gamepad remapping for BTN1/BTN2/BTN3 only. Sibling to
// keymap.rs: same data dir, same shim on web, same Pico-8-style
// override-replaces-defaults semantics. Directional inputs stay on
// dpad + left stick.
mod pad_map;

// game_id is universal: same id namespaces save data on every target,
// including web localStorage, the macOS CFBundleIdentifier, and any future
// per-game preference. Runtime save/load resolves through this module too,
// so it's not export-only.
mod game_id;

// Export + templates aren't reachable from the wasm runtime (no CLI on
// web) and their dep chain (ureq -> rustls -> ring) doesn't build for
// emscripten anyway. Native-only.
#[cfg(not(target_os = "emscripten"))]
mod export;
#[cfg(not(target_os = "emscripten"))]
mod init;
#[cfg(not(target_os = "emscripten"))]
mod macos_app;
#[cfg(not(target_os = "emscripten"))]
mod refresh;
#[cfg(not(target_os = "emscripten"))]
mod templates;
#[cfg(not(target_os = "emscripten"))]
mod update;

pub use error::{Error, Result};

use bundle::Bundle;
use std::path::Path;
use std::process::ExitCode;
use vfs::{BundleBacked, FsBacked};

#[cfg(not(target_os = "emscripten"))]
use clap::{Parser, Subcommand};
#[cfg(not(target_os = "emscripten"))]
use export::ExportTarget;

#[cfg(not(target_os = "emscripten"))]
#[derive(Parser)]
#[command(name = "usagi", version, about = "Rapid 2D game prototyping with Lua")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[cfg(not(target_os = "emscripten"))]
#[derive(Subcommand)]
enum Command {
    /// Run a game (no live-reload). Defaults to the current directory.
    Run {
        /// Path to a .lua file or a directory with main.lua. Defaults to ".".
        path: Option<String>,
    },
    /// Run a game with live-reload on save. F5 resets state. Defaults to
    /// the current directory.
    Dev {
        /// Path to a .lua file or a directory with main.lua. Defaults to ".".
        path: Option<String>,
    },
    /// Bootstrap a new project (main.lua, .luarc.json, .gitignore, LSP
    /// stubs, embedded docs). Defaults to the current directory.
    Init {
        /// Directory to initialize. Defaults to ".". Created if missing.
        path: Option<String>,
    },
    /// Open the Usagi tools window (jukebox, tile picker). Defaults to
    /// the current directory.
    Tools {
        /// Path to the game project (dir or .lua file). Defaults to ".".
        /// Tools use this to locate sprites.png, sfx/, etc.
        path: Option<String>,
    },
    /// Inspect or wipe the local template cache.
    Templates {
        #[command(subcommand)]
        cmd: TemplatesCmd,
    },
    /// Export a game as shippable artifacts (zips per platform + `.usagi`
    /// bundle). Defaults to the current directory.
    Export {
        /// Path to a .lua file or a directory with main.lua. Defaults to ".".
        path: Option<String>,
        /// Output path. Defaults to `export/` for `all`,
        /// `<name>.usagi` for `bundle`, `<name>-<target>.zip` otherwise.
        #[arg(short, long)]
        output: Option<String>,
        /// What to produce. `all` (default) emits every platform zip plus
        /// the portable `.usagi` bundle. `bundle` writes only the bundle.
        /// `linux` / `macos` / `windows` / `web` write one platform zip.
        /// Templates auto-fetch by version on first use; override with
        /// `--template-path` or `--template-url`.
        #[arg(long, value_enum, default_value_t = ExportTarget::All)]
        target: ExportTarget,
        /// Local template, either a release archive (`.tar.gz` for
        /// linux/macos/web, `.zip` for windows) or an already-extracted
        /// directory containing the runtime files. Pointing at the local
        /// wasm build dir is the easy way to test web exports while
        /// iterating: `--template-path target/wasm32-unknown-emscripten/release`.
        #[arg(long, conflicts_with = "template_url")]
        template_path: Option<String>,
        /// HTTP(S) URL to fetch a release-archive template from. Useful
        /// for forks, mirrors, and air-gapped registries.
        #[arg(long)]
        template_url: Option<String>,
        /// Bypass the local template cache and re-download.
        #[arg(long)]
        no_cache: bool,
        /// Custom HTML shell for the web export (the page that hosts the
        /// canvas). Defaults to `<project>/shell.html` when present,
        /// otherwise the shell baked into the web template. Ignored
        /// for non-web targets.
        #[arg(long)]
        web_shell: Option<String>,
    },
    /// Update the usagi binary in place to the latest GitHub release.
    Update,
    /// Re-sync engine-managed files (USAGI.md, meta/usagi.lua,
    /// .luarc.json) from the running engine version. Interactive by
    /// default; never touches main.lua or .gitignore.
    Refresh {
        /// Path to the project root. Defaults to ".".
        path: Option<String>,
        /// Skip prompts and overwrite all engine-managed files that differ.
        #[arg(short, long, conflicts_with = "dry_run")]
        yes: bool,
        /// Report what would change without writing anything.
        #[arg(long, conflicts_with = "yes")]
        dry_run: bool,
    },
    /// Font asset commands.
    Font {
        #[command(subcommand)]
        cmd: FontCmd,
    },
}

#[cfg(not(target_os = "emscripten"))]
#[derive(Subcommand)]
enum FontCmd {
    /// Bake a TTF/OTF into a single PNG with embedded glyph metadata
    /// for use as a custom font. Drop the output next to your
    /// project's main.lua and the engine loads it automatically. Pass
    /// the font's natural design size for crispest output (e.g., 15
    /// for monogram, 18 for Silver, 8 for Misaki); FreeType-hinted
    /// rendering means most pixel fonts look right at their design
    /// size and slightly fuzzy at other sizes.
    Bake {
        /// Path to the .ttf or .otf file to bake.
        ttf: String,
        /// Pixel size to rasterize at.
        size: u32,
        /// Output PNG path. Defaults to `font.png` in the current
        /// directory so the result is immediately a project drop-in.
        #[arg(short, long, default_value = "font.png")]
        out: String,
        /// Skip the CJK Unified Ideographs block. Default is to
        /// include it; the cmap filter drops codepoints the font
        /// doesn't cover, so enabling it costs nothing for fonts
        /// without kanji/hanzi/hanja.
        #[arg(long)]
        no_cjk: bool,
    },
}

#[cfg(not(target_os = "emscripten"))]
#[derive(Subcommand)]
enum TemplatesCmd {
    /// Show what's cached and total disk usage.
    List,
    /// Wipe every cached template.
    Clear,
}

fn main() -> ExitCode {
    // Web build: there is no CLI, no fused-exe trick, no export mode. The
    // JS shell preloads the bundle at `/game.usagi` in the wasm virtual FS
    // before calling main(); the runtime then loads and runs it. See
    // `web/shell.html` and `docs/web-build.md`.
    #[cfg(target_os = "emscripten")]
    {
        return finish(start_session("/game.usagi", false));
    }

    // Native: if this binary has a fused bundle appended, run that;
    // otherwise dispatch on the CLI.
    #[cfg(not(target_os = "emscripten"))]
    {
        if let Some(bundle) = Bundle::load_from_current_exe() {
            return finish(run_bundled(bundle));
        }
        let cli = Cli::parse();
        let result = match cli.command {
            Command::Run { path } => start_session(path.as_deref().unwrap_or("."), false),
            Command::Dev { path } => start_session(path.as_deref().unwrap_or("."), true),
            Command::Init { path } => init::run(path.as_deref().unwrap_or(".")),
            Command::Tools { path } => tools::run(Some(path.as_deref().unwrap_or("."))),
            Command::Templates { cmd } => run_templates_cmd(cmd),
            Command::Export {
                path,
                output,
                target,
                template_path,
                template_url,
                no_cache,
                web_shell,
            } => export::run(
                path.as_deref().unwrap_or("."),
                output.as_deref(),
                target,
                template_path.as_deref(),
                template_url.as_deref(),
                no_cache,
                web_shell.as_deref(),
            ),
            Command::Update => update::run(),
            Command::Refresh { path, yes, dry_run } => {
                refresh::run(path.as_deref().unwrap_or("."), yes, dry_run)
            }
            Command::Font { cmd } => run_font_cmd(cmd),
        };
        finish(result)
    }
}

fn finish(result: Result<()>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            crate::msg::err!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn start_session(path_arg: &str, dev: bool) -> Result<()> {
    if Path::new(path_arg)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("usagi"))
    {
        if dev {
            return Err(Error::Cli(
                "live-reload (`usagi dev`) only works on source projects, not .usagi bundles"
                    .into(),
            ));
        }
        let bundle = Bundle::load_from_path(Path::new(path_arg))
            .map_err(|e| Error::Cli(format!("loading bundle from {path_arg}: {e}")))?;
        return run_bundled(bundle);
    }

    let script_path = cli::resolve_script_path(path_arg)?;
    let vfs = std::rc::Rc::new(FsBacked::from_script_path(Path::new(&script_path)));
    session::run(vfs, dev)
}

fn run_bundled(bundle: Bundle) -> Result<()> {
    let vfs = std::rc::Rc::new(BundleBacked::new(bundle));
    session::run(vfs, false)
}

#[cfg(not(target_os = "emscripten"))]
fn run_font_cmd(cmd: FontCmd) -> Result<()> {
    match cmd {
        FontCmd::Bake {
            ttf,
            size,
            out,
            no_cjk,
        } => font_bake::run(
            std::path::Path::new(&ttf),
            size,
            std::path::Path::new(&out),
            !no_cjk,
        )
        .map_err(crate::Error::Cli),
    }
}

#[cfg(not(target_os = "emscripten"))]
fn run_templates_cmd(cmd: TemplatesCmd) -> Result<()> {
    let root = templates::cache_templates_root()?;
    match cmd {
        TemplatesCmd::List => templates::list_cache(&root),
        TemplatesCmd::Clear => templates::clear_cache(&root),
    }
}
