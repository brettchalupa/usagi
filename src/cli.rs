//! Command-line argument resolution.

use std::path::Path;

/// Resolves the CLI arg to a concrete script file. Accepts any of:
///   - path to a `.lua` file
///   - path to a directory containing `main.lua`
///   - path without extension that has a sibling `.lua` file
///
/// Errors with a helpful message if none match.
pub fn resolve_script_path(arg: &str) -> Result<String, String> {
    let path = Path::new(arg);
    if path.is_dir() {
        let main = path.join("main.lua");
        if main.exists() {
            return main
                .to_str()
                .map(String::from)
                .ok_or_else(|| format!("non-utf8 path: {}", main.display()));
        }
        return Err(format!(
            "no main.lua found in directory '{}'. Create a main.lua there, or pass a .lua file directly.",
            path.display()
        ));
    }
    if path.is_file() {
        return Ok(arg.to_string());
    }
    let with_lua = path.with_extension("lua");
    if with_lua.is_file() {
        return with_lua
            .to_str()
            .map(String::from)
            .ok_or_else(|| format!("non-utf8 path: {}", with_lua.display()));
    }
    Err(format!(
        "script not found: '{}'. Pass a .lua file, a directory with main.lua, or a name with a sibling .lua.",
        arg
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn resolves_direct_lua_file() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("game.lua");
        fs::write(&file, "-- test").unwrap();
        let arg = file.to_str().unwrap();
        assert_eq!(resolve_script_path(arg).unwrap(), arg);
    }

    #[test]
    fn resolves_dir_with_main_lua() {
        let dir = TempDir::new().unwrap();
        let main = dir.path().join("main.lua");
        fs::write(&main, "-- test").unwrap();
        let resolved = resolve_script_path(dir.path().to_str().unwrap()).unwrap();
        assert_eq!(resolved, main.to_str().unwrap());
    }

    #[test]
    fn errors_for_dir_without_main_lua() {
        let dir = TempDir::new().unwrap();
        let err = resolve_script_path(dir.path().to_str().unwrap()).unwrap_err();
        assert!(err.contains("no main.lua"), "got: {err}");
    }

    #[test]
    fn errors_for_missing_path() {
        let err = resolve_script_path("/definitely/not/a/real/path/xyzzy").unwrap_err();
        assert!(err.contains("not found"), "got: {err}");
    }

    #[test]
    fn adds_lua_extension_when_missing() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("foo.lua");
        fs::write(&file, "-- test").unwrap();
        let bare = dir.path().join("foo");
        let resolved = resolve_script_path(bare.to_str().unwrap()).unwrap();
        assert_eq!(resolved, file.to_str().unwrap());
    }

    #[test]
    fn dir_takes_precedence_over_sibling_lua() {
        // If `foo/` exists as a dir with main.lua AND `foo.lua` exists, the
        // dir should win because is_dir() is checked first.
        let dir = TempDir::new().unwrap();
        let subdir = dir.path().join("foo");
        fs::create_dir(&subdir).unwrap();
        let main = subdir.join("main.lua");
        fs::write(&main, "-- dir").unwrap();
        let sibling = dir.path().join("foo.lua");
        fs::write(&sibling, "-- sibling").unwrap();

        let resolved = resolve_script_path(subdir.to_str().unwrap()).unwrap();
        assert_eq!(resolved, main.to_str().unwrap());
    }
}
