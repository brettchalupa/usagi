//! Usagi: rapid 2D game prototyping with Lua.
//!
//! The binary has two modes of operation:
//!
//! 1. **Normal mode** parses the CLI and dispatches to a subcommand
//!    (`run` / `dev` / `tools` / `compile`).
//! 2. **Fused mode** (when a `usagi compile` output has appended a bundle)
//!    detects the bundle at startup and runs the embedded game directly,
//!    skipping the CLI entirely. This is how shipped game binaries work.

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
use clap::{Parser, Subcommand};
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
    /// Compile a game into a standalone executable for the current platform.
    Compile {
        /// Path to a .lua file or a directory with main.lua.
        path: String,
        /// Output path for the compiled binary (defaults to the project
        /// name in the current directory).
        #[arg(short, long)]
        output: Option<String>,
    },
}

fn main() -> ExitCode {
    // If this binary has a fused bundle appended, run it in "shipped game"
    // mode and skip CLI parsing entirely.
    if let Some(bundle) = Bundle::load_from_current_exe() {
        return match run_bundled(bundle) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("[usagi] {e}");
                ExitCode::FAILURE
            }
        };
    }

    let cli = Cli::parse();
    let result = match cli.command {
        Command::Run { path } => start_session(&path, false),
        Command::Dev { path } => start_session(&path, true),
        Command::Tools { path } => tools::run(path.as_deref()),
        Command::Compile { path, output } => compile(&path, output.as_deref()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[usagi] {e}");
            ExitCode::FAILURE
        }
    }
}

fn start_session(path_arg: &str, dev: bool) -> Result<()> {
    let script_path = cli::resolve_script_path(path_arg)?;
    let vfs = FsBacked::from_script_path(Path::new(&script_path));
    session::run(&vfs, dev)
}

fn run_bundled(bundle: Bundle) -> Result<()> {
    let vfs = BundleBacked::new(bundle);
    session::run(&vfs, false)
}

fn compile(path_arg: &str, output: Option<&str>) -> Result<()> {
    let script_path = cli::resolve_script_path(path_arg)?;
    let script_path = PathBuf::from(script_path);

    let bundle = Bundle::from_project(&script_path).map_err(|e| {
        Error::Cli(format!(
            "building bundle from {}: {e}",
            script_path.display()
        ))
    })?;

    let out_path = output
        .map(PathBuf::from)
        .unwrap_or_else(|| default_output_path(&script_path));

    let current_exe =
        std::env::current_exe().map_err(|e| Error::Cli(format!("locating current exe: {e}")))?;
    bundle
        .fuse(&current_exe, &out_path)
        .map_err(|e| Error::Cli(format!("fusing bundle onto base exe: {e}")))?;

    println!(
        "[usagi] compiled {} ({} file(s), {} bytes bundled)",
        out_path.display(),
        bundle.file_count(),
        bundle.total_bytes(),
    );
    Ok(())
}

/// Default output name: if the script is `main.lua`, use the parent
/// directory's name (e.g. `examples/spr/main.lua` -> `spr`). Otherwise use
/// the script's stem (e.g. `examples/snake.lua` -> `snake`). Adds `.exe`
/// on Windows.
fn default_output_path(script_path: &Path) -> PathBuf {
    let stem = script_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("game");
    let name = if stem == "main" {
        script_path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or(stem)
    } else {
        stem
    };
    if cfg!(windows) {
        PathBuf::from(format!("{name}.exe"))
    } else {
        PathBuf::from(name)
    }
}
