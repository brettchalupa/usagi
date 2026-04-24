//! Crate-wide error type. `Lua` is the common source (user script errors,
//! API binding failures); `Cli` covers non-Lua CLI failures (bad path,
//! missing file). Propagates via `?` through `From` impls.

use mlua::Error as LuaError;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug)]
pub enum Error {
    /// CLI-level problem (e.g. script path doesn't resolve). Usually
    /// reported once in main and exits non-zero.
    Cli(String),
    /// Any Lua-side error: syntax, runtime, binding setup.
    Lua(LuaError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Cli(msg) => f.write_str(msg),
            Error::Lua(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Cli(_) => None,
            Error::Lua(e) => Some(e),
        }
    }
}

impl From<LuaError> for Error {
    fn from(e: LuaError) -> Self {
        Error::Lua(e)
    }
}

/// `cli::resolve_script_path` returns `Result<_, String>`; this lets `?`
/// convert its error straight into a `Cli` variant.
impl From<String> for Error {
    fn from(msg: String) -> Self {
        Error::Cli(msg)
    }
}
