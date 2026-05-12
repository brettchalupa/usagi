//! User-facing CLI logging with the `[usagi]` prefix. Three levels:
//! `msg::info!` (green, stdout), `msg::warn!` (yellow, stderr), and
//! `msg::err!` (red, stderr). The `[usagi]` prefix is dimmed so the
//! message itself reads as the foreground content.
//!
//! Color is auto-disabled when stdout/stderr aren't terminals (so
//! piped output and CI logs stay clean) or when `NO_COLOR` is set
//! in the environment, per the de facto cross-CLI convention from
//! <https://no-color.org>. On the web (emscripten) color is forced
//! off since the browser devtools console prints stdout verbatim
//! and would render the escape bytes as garbage.
//!
//! ANSI escapes are written by hand here (no extra crate) since
//! we control all the call sites and the styling is uniform.
//! Modern Windows Terminal, PowerShell, cmd (Win10+), and every
//! Unix terminal handle these correctly without extra setup; if
//! pre-Win10 cmd ever becomes a target we can flip on virtual
//! terminal processing once at startup.

use std::io::IsTerminal;

const PREFIX: &str = "[usagi]";

const RESET: &str = "\x1b[0m";
const DIM: &str = "\x1b[2m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RED: &str = "\x1b[31m";

fn no_color_env() -> bool {
    std::env::var_os("NO_COLOR").is_some()
}

fn color_stdout() -> bool {
    if cfg!(target_os = "emscripten") {
        return false;
    }
    !no_color_env() && std::io::stdout().is_terminal()
}

fn color_stderr() -> bool {
    if cfg!(target_os = "emscripten") {
        return false;
    }
    !no_color_env() && std::io::stderr().is_terminal()
}

/// Hidden helper called by the `info!` macro. Writes the formatted
/// message to stdout, dimming the prefix and colorizing the body
/// green when stdout is a terminal.
pub fn __info_impl(args: std::fmt::Arguments) {
    if color_stdout() {
        println!("{DIM}{PREFIX}{RESET} {GREEN}{args}{RESET}");
    } else {
        println!("{PREFIX} {args}");
    }
}

/// Hidden helper called by the `warn!` macro.
pub fn __warn_impl(args: std::fmt::Arguments) {
    if color_stderr() {
        eprintln!("{DIM}{PREFIX}{RESET} {YELLOW}{args}{RESET}");
    } else {
        eprintln!("{PREFIX} {args}");
    }
}

/// Hidden helper called by the `err!` macro.
pub fn __err_impl(args: std::fmt::Arguments) {
    if color_stderr() {
        eprintln!("{DIM}{PREFIX}{RESET} {RED}{args}{RESET}");
    } else {
        eprintln!("{PREFIX} {args}");
    }
}

/// `msg::info!("reloaded {}", path)` — stdout, green message.
#[macro_export]
macro_rules! __msg_info {
    ($($arg:tt)*) => { $crate::msg::__info_impl(format_args!($($arg)*)) };
}
pub use __msg_info as info;

/// `msg::warn!("settings write failed: {e}")` — stderr, yellow message.
#[macro_export]
macro_rules! __msg_warn {
    ($($arg:tt)*) => { $crate::msg::__warn_impl(format_args!($($arg)*)) };
}
pub use __msg_warn as warn;

/// `msg::err!("audio init failed: {e}")` — stderr, red message.
#[macro_export]
macro_rules! __msg_err {
    ($($arg:tt)*) => { $crate::msg::__err_impl(format_args!($($arg)*)) };
}
pub use __msg_err as err;
