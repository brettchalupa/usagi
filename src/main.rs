//! Usagi: rapid 2D game prototyping with Lua.
//!
//! The binary dispatches one of a few subcommands:
//!   - `usagi run <path>`    run a game without live-reload
//!   - `usagi dev <path>`    run a game with live-reload on save
//!   - `usagi tools`         open the Usagi tools (jukebox, tile picker)

mod api;
mod assets;
mod cli;
mod input;
mod palette;
mod render;
mod session;
mod tools;

use clap::{Parser, Subcommand};
use mlua::prelude::*;

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
}

fn main() -> LuaResult<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Run { path } => start_session(&path, false),
        Command::Dev { path } => start_session(&path, true),
        Command::Tools { path } => tools::run(path.as_deref()),
    }
}

fn start_session(path_arg: &str, dev: bool) -> LuaResult<()> {
    let script_path = match cli::resolve_script_path(path_arg) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[usagi] {}", e);
            std::process::exit(1);
        }
    };
    session::run(&script_path, dev)
}
