//! Usagi: rapid 2D game prototyping with Lua.
//!
//! The binary has two modes of operation:
//!
//! 1. **Normal mode** parses the CLI and dispatches to a subcommand
//!    (`run` / `dev` / `tools` / `compile`).
//! 2. **Fused mode** (when a `usagi compile` output has appended a bundle)
//!    detects the bundle at startup and runs the embedded game directly,
//!    skipping the CLI entirely. This is how shipped game binaries work.
//!
//! On the web build (target_os = "emscripten") there is no CLI: the JS
//! shell fetches a `.usagi` bundle and writes it to `/game.usagi` in the
//! wasm virtual FS before calling `main()`, and we run that bundle.

mod api;
mod assets;
mod bundle;
mod cli;
mod error;
mod input;
mod palette;
mod render;
mod session;
mod tools;
mod vfs;

pub use error::{Error, Result};

use bundle::Bundle;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use vfs::{BundleBacked, FsBacked};

/// Game render dimensions, in pixels. The internal RT is always this size;
/// the window upscales to fit.
pub const GAME_WIDTH: f32 = 320.;
pub const GAME_HEIGHT: f32 = 180.;

#[derive(Parser)]
#[command(name = "usagi", version, about = "Rapid 2D game prototyping with Lua")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a game (no live-reload).
    Run {
        /// Path to a .lua file or a directory with main.lua.
        path: String,
    },
    /// Run a game with live-reload on save. F5 resets state.
    Dev {
        /// Path to a .lua file or a directory with main.lua.
        path: String,
    },
    /// Open the Usagi tools window (jukebox, tile picker).
    Tools {
        /// Optional path to the game project (dir or .lua file). Future
        /// tools use this to locate sprites.png, sfx/, etc.
        path: Option<String>,
    },
    /// Compile a game into a shippable artifact.
    Compile {
        /// Path to a .lua file or a directory with main.lua.
        path: String,
        /// Output path. Default depends on `--target`: `<name>-export/`
        /// for `all`, `<name>` for `exe`, `<name>.usagi` for `bundle`,
        /// `<name>-web/` for `web`.
        #[arg(short, long)]
        output: Option<String>,
        /// What to produce. `all` (default) emits exe + bundle + web in
        /// one directory. `exe` is a fused standalone binary for the
        /// current platform. `bundle` is a portable `.usagi` file (run
        /// with `usagi run`). `web` is a directory ready to upload to
        /// itch.io.
        #[arg(long, value_enum, default_value_t = CompileTarget::All)]
        target: CompileTarget,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum CompileTarget {
    /// Exe + bundle + web in one export directory.
    All,
    /// Standalone fused executable for the current platform.
    Exe,
    /// Portable `.usagi` bundle file.
    Bundle,
    /// Web (wasm) directory: `index.html`, `usagi.{js,wasm}`, `game.usagi`.
    Web,
}

fn main() -> ExitCode {
    // Web build: there is no CLI, no fused-exe trick, no compile mode. The
    // JS shell preloads the bundle at `/game.usagi` in the wasm virtual FS
    // before calling main(); we just load and run it. See `web/shell.html`
    // and `docs/web-build.md`.
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
            Command::Run { path } => start_session(&path, false),
            Command::Dev { path } => start_session(&path, true),
            Command::Tools { path } => tools::run(path.as_deref()),
            Command::Compile {
                path,
                output,
                target,
            } => compile(&path, output.as_deref(), target),
        };
        finish(result)
    }
}

fn finish(result: Result<()>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[usagi] {e}");
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
    let vfs = Box::new(FsBacked::from_script_path(Path::new(&script_path)));
    session::run(vfs, dev)
}

fn run_bundled(bundle: Bundle) -> Result<()> {
    let vfs = Box::new(BundleBacked::new(bundle));
    session::run(vfs, false)
}

fn compile(path_arg: &str, output: Option<&str>, target: CompileTarget) -> Result<()> {
    let script_path = cli::resolve_script_path(path_arg)?;
    let script_path = PathBuf::from(script_path);

    let bundle = Bundle::from_project(&script_path).map_err(|e| {
        Error::Cli(format!(
            "building bundle from {}: {e}",
            script_path.display()
        ))
    })?;
    let name = project_name(&script_path).to_owned();

    let out_path = output
        .map(PathBuf::from)
        .unwrap_or_else(|| default_output_path(&name, target));

    match target {
        CompileTarget::All => compile_all(&bundle, &name, &out_path)?,
        CompileTarget::Exe => write_exe(&bundle, &out_path)?,
        CompileTarget::Bundle => write_bundle(&bundle, &out_path)?,
        CompileTarget::Web => write_web(&bundle, &out_path)?,
    }
    Ok(())
}

fn write_exe(bundle: &Bundle, out_path: &Path) -> Result<()> {
    let current_exe =
        std::env::current_exe().map_err(|e| Error::Cli(format!("locating current exe: {e}")))?;
    bundle
        .fuse(&current_exe, out_path)
        .map_err(|e| Error::Cli(format!("fusing bundle onto base exe: {e}")))?;
    println!(
        "[usagi] compiled {} ({} file(s), {} bytes bundled)",
        out_path.display(),
        bundle.file_count(),
        bundle.total_bytes(),
    );
    Ok(())
}

fn write_bundle(bundle: &Bundle, out_path: &Path) -> Result<()> {
    bundle
        .write_to_path(out_path)
        .map_err(|e| Error::Cli(format!("writing bundle to {}: {e}", out_path.display())))?;
    println!(
        "[usagi] wrote {} ({} file(s), {} bytes)",
        out_path.display(),
        bundle.file_count(),
        bundle.total_bytes(),
    );
    Ok(())
}

/// Write the web runtime files (index.html, usagi.js, usagi.wasm) plus
/// game.usagi into `out_dir`. The runtime files come from one of:
///   - constants embedded at build time (release native binaries),
///   - `target/web/` on disk (developer working in the source tree).
fn write_web(bundle: &Bundle, out_dir: &Path) -> Result<()> {
    let (runtime, source) = load_web_runtime()?;
    std::fs::create_dir_all(out_dir)
        .map_err(|e| Error::Cli(format!("creating output dir {}: {e}", out_dir.display())))?;
    std::fs::write(out_dir.join("index.html"), runtime.index_html)
        .map_err(|e| Error::Cli(format!("writing index.html: {e}")))?;
    std::fs::write(out_dir.join("usagi.js"), runtime.usagi_js)
        .map_err(|e| Error::Cli(format!("writing usagi.js: {e}")))?;
    std::fs::write(out_dir.join("usagi.wasm"), runtime.usagi_wasm)
        .map_err(|e| Error::Cli(format!("writing usagi.wasm: {e}")))?;
    bundle
        .write_to_path(&out_dir.join("game.usagi"))
        .map_err(|e| Error::Cli(format!("writing game.usagi: {e}")))?;
    println!(
        "[usagi] wrote {}/ ({} game file(s), {} bundle bytes; runtime from {})",
        out_dir.display(),
        bundle.file_count(),
        bundle.total_bytes(),
        source,
    );
    Ok(())
}

fn compile_all(bundle: &Bundle, name: &str, out_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(out_dir)
        .map_err(|e| Error::Cli(format!("creating export dir {}: {e}", out_dir.display())))?;
    let exe_name = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };
    write_exe(bundle, &out_dir.join(exe_name))?;
    write_bundle(bundle, &out_dir.join(format!("{name}.usagi")))?;
    write_web(bundle, &out_dir.join("web"))?;
    println!("[usagi] export ready at {}/", out_dir.display());
    Ok(())
}

/// Embedded web runtime files. In release builds, `build.rs` copies the
/// real artifacts here. In debug builds these are 0-byte stubs and we
/// fall back to reading the runtime from `target/web/` on disk.
const EMBEDDED_INDEX_HTML: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/web_runtime/index.html"));
const EMBEDDED_USAGI_JS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/web_runtime/usagi.js"));
const EMBEDDED_USAGI_WASM: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/web_runtime/usagi.wasm"));

struct WebRuntime<'a> {
    index_html: &'a [u8],
    usagi_js: &'a [u8],
    usagi_wasm: &'a [u8],
}

/// Returns the runtime byte blobs and a label describing where they came
/// from (for the post-compile log line).
fn load_web_runtime() -> Result<(WebRuntime<'static>, String)> {
    if !EMBEDDED_USAGI_WASM.is_empty() {
        return Ok((
            WebRuntime {
                index_html: EMBEDDED_INDEX_HTML,
                usagi_js: EMBEDDED_USAGI_JS,
                usagi_wasm: EMBEDDED_USAGI_WASM,
            },
            "embedded".to_owned(),
        ));
    }
    let dir = PathBuf::from("target/web");
    if !dir.join("usagi.wasm").is_file() {
        return Err(Error::Cli(
            "web runtime not available. Either: (a) run `just build-web-release` \
             then `cargo build --release` so the runtime gets embedded into a \
             release binary, or (b) run `just build-web` so the on-disk runtime \
             at `target/web/` is available as a fallback for debug builds."
                .into(),
        ));
    }
    // Leak the bytes so the borrow has 'static lifetime, matching the
    // embedded path. The CLI process exits right after, so the leak is
    // immaterial.
    let index_html: &'static [u8] = Box::leak(
        std::fs::read(dir.join("index.html"))
            .map_err(|e| Error::Cli(format!("reading target/web/index.html: {e}")))?
            .into_boxed_slice(),
    );
    let usagi_js: &'static [u8] = Box::leak(
        std::fs::read(dir.join("usagi.js"))
            .map_err(|e| Error::Cli(format!("reading target/web/usagi.js: {e}")))?
            .into_boxed_slice(),
    );
    let usagi_wasm: &'static [u8] = Box::leak(
        std::fs::read(dir.join("usagi.wasm"))
            .map_err(|e| Error::Cli(format!("reading target/web/usagi.wasm: {e}")))?
            .into_boxed_slice(),
    );
    Ok((
        WebRuntime {
            index_html,
            usagi_js,
            usagi_wasm,
        },
        format!("{}", dir.display()),
    ))
}

/// Project base name from a script path. Uses the parent directory's
/// name when the script is `main.lua` (so `examples/spr/main.lua` -> `spr`)
/// and the file stem otherwise (`examples/snake.lua` -> `snake`).
fn project_name(script_path: &Path) -> &str {
    let stem = script_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("game");
    if stem == "main" {
        script_path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or(stem)
    } else {
        stem
    }
}

fn default_output_path(name: &str, target: CompileTarget) -> PathBuf {
    match target {
        CompileTarget::All => PathBuf::from(format!("{name}-export")),
        CompileTarget::Bundle => PathBuf::from(format!("{name}.usagi")),
        CompileTarget::Web => PathBuf::from(format!("{name}-web")),
        CompileTarget::Exe if cfg!(windows) => PathBuf::from(format!("{name}.exe")),
        CompileTarget::Exe => PathBuf::from(name),
    }
}
