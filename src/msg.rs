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
//!
//! On Windows, conhost in Win10+ understands ANSI but only after a
//! process opts in via `SetConsoleMode(handle,
//! ENABLE_VIRTUAL_TERMINAL_PROCESSING)`. Windows Terminal and
//! PowerShell inherit this from the parent process; bare cmd.exe
//! does not, so without the opt-in our output rendered as raw
//! escape bytes there. We lazily call into the `enable-ansi-support`
//! crate the first time `color_stdout` / `color_stderr` is consulted
//! and remember the result in a `OnceLock`. If the enable fails
//! (truly pre-Win10), color output is suppressed and we fall back to
//! the plain `[usagi]` prefix path.

use std::io::IsTerminal;
use std::sync::OnceLock;

const PREFIX: &str = "[usagi]";

const RESET: &str = "\x1b[0m";
const DIM: &str = "\x1b[2m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RED: &str = "\x1b[31m";
const CYAN: &str = "\x1b[36m";

/// True when `USAGI_VERBOSE` is set in the environment. Gates `msg::dbg!`
/// output so verbose-only diagnostics (per-second frame budget, Lua
/// heap size, startup snapshot) cost nothing when not requested.
pub fn dbg_enabled() -> bool {
    std::env::var_os("USAGI_VERBOSE").is_some()
}

fn no_color_env() -> bool {
    std::env::var_os("NO_COLOR").is_some()
}

/// True when ANSI escape sequences can be written to the console.
/// On Windows this lazily calls `enable_ansi_support::enable_ansi_support()`
/// the first time it's consulted, enabling VT processing for the current
/// process. Elsewhere it's unconditionally true (every Unix terminal we
/// care about handles ANSI without setup).
fn ansi_supported() -> bool {
    static SUPPORTED: OnceLock<bool> = OnceLock::new();
    *SUPPORTED.get_or_init(|| {
        #[cfg(windows)]
        {
            enable_ansi_support::enable_ansi_support().is_ok()
        }
        #[cfg(not(windows))]
        {
            true
        }
    })
}

fn color_stdout() -> bool {
    if cfg!(target_os = "emscripten") {
        return false;
    }
    ansi_supported() && !no_color_env() && std::io::stdout().is_terminal()
}

fn color_stderr() -> bool {
    if cfg!(target_os = "emscripten") {
        return false;
    }
    ansi_supported() && !no_color_env() && std::io::stderr().is_terminal()
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

/// Hidden helper called by the `dbg!` macro. Caller is expected to
/// gate on `dbg_enabled()` before formatting, so this always writes.
pub fn __dbg_impl(args: std::fmt::Arguments) {
    if color_stdout() {
        println!("{DIM}{PREFIX}{RESET} {CYAN}{args}{RESET}");
    } else {
        println!("{PREFIX} {args}");
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

/// `msg::dbg!("lua heap {kb} KB")` — stdout, cyan message, only emitted
/// when `USAGI_VERBOSE=1`. The gate short-circuits before `format_args!`
/// runs, so the macro is free at non-verbose call sites.
#[macro_export]
macro_rules! __msg_dbg {
    ($($arg:tt)*) => {
        if $crate::msg::dbg_enabled() {
            $crate::msg::__dbg_impl(format_args!($($arg)*));
        }
    };
}
pub use __msg_dbg as dbg;
