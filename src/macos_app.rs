//! macOS .app bundle staging for `usagi export --target macos`.
//!
//! Produces the standard layout Finder and Launch Services expect:
//!
//! ```text
//! <name>.app/
//!   Contents/
//!     Info.plist
//!     PkgInfo
//!     MacOS/<name>          (fused exe, +x via bundle::fuse)
//! ```
//!
//! Out of scope for now: code signing, notarization, icons, Resources.
//! Without a signature the app launches via right-click → Open (or one
//! Security & Privacy bypass) on the end user's machine.
//!
//! The `CFBundleIdentifier` written here is whatever the caller resolved
//! via `game_id::resolve` — same string the save-data layer namespaces
//! on, so save data and the .app bundle stay in lockstep.

use crate::{Error, Result};
use std::path::{Path, PathBuf};

/// Creates `<stage>/<name>.app/Contents/{Info.plist,PkgInfo,MacOS/}` and
/// returns the path the caller should fuse the binary onto
/// (`Contents/MacOS/<name>`). bundle::fuse handles the `+x` chmod.
pub fn stage_app_layout(stage: &Path, name: &str, bundle_id: &str) -> Result<PathBuf> {
    let app = stage.join(format!("{name}.app"));
    let contents = app.join("Contents");
    let macos_dir = contents.join("MacOS");
    std::fs::create_dir_all(&macos_dir)
        .map_err(|e| Error::Cli(format!("creating {}: {e}", macos_dir.display())))?;

    std::fs::write(contents.join("Info.plist"), info_plist_xml(name, bundle_id))
        .map_err(|e| Error::Cli(format!("writing Info.plist: {e}")))?;

    // PkgInfo is the legacy 8-byte type+creator code Launch Services still
    // reads on older paths. "APPL????" is the standard "generic application,
    // no creator" pair. Cheap to write, avoids edge-case recognition bugs
    // on tools that pre-date Info.plist-only bundles.
    std::fs::write(contents.join("PkgInfo"), b"APPL????")
        .map_err(|e| Error::Cli(format!("writing PkgInfo: {e}")))?;

    Ok(macos_dir.join(name))
}

/// Hand-rolled Info.plist XML. Keys here are the minimum set Finder /
/// Launch Services / Gatekeeper consult:
/// - CFBundle{Name,DisplayName,Executable,Identifier} for identity.
/// - CFBundlePackageType=APPL + Signature=???? to be recognized as an app.
/// - CFBundleShortVersionString + CFBundleVersion are required by macOS.
/// - LSMinimumSystemVersion=11.0 matches the macos-aarch64 release target.
/// - NSHighResolutionCapable so Retina backing-store kicks in (otherwise
///   raylib's framebuffer is upscaled by the OS and looks blurry).
fn info_plist_xml(name: &str, bundle_id: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleDisplayName</key>
    <string>{name_escaped}</string>
    <key>CFBundleExecutable</key>
    <string>{name_escaped}</string>
    <key>CFBundleIdentifier</key>
    <string>{bundle_id_escaped}</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>{name_escaped}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.games</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
"#,
        name_escaped = xml_escape(name),
        bundle_id_escaped = xml_escape(bundle_id),
    )
}

/// Escape the five characters that matter inside `<string>` values. Names
/// almost never contain these, but a stray `&` in a project name should
/// not produce a malformed plist that Launch Services silently rejects.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\'', "&apos;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn stage_app_layout_creates_expected_tree_and_returns_binary_path() {
        let dir = tempdir().unwrap();
        let exe_path = stage_app_layout(dir.path(), "snake", "com.usagiengine.snake").unwrap();
        let app = dir.path().join("snake.app");
        assert!(app.is_dir(), "snake.app/ should exist");
        assert!(app.join("Contents").is_dir());
        assert!(app.join("Contents/MacOS").is_dir());
        assert!(app.join("Contents/Info.plist").is_file());
        assert!(app.join("Contents/PkgInfo").is_file());
        assert_eq!(exe_path, app.join("Contents/MacOS/snake"));
        assert!(
            !exe_path.exists(),
            "binary not staged yet — caller fuses into it"
        );
    }

    #[test]
    fn stage_app_layout_writes_caller_supplied_bundle_id_into_plist() {
        let dir = tempdir().unwrap();
        stage_app_layout(dir.path(), "snake", "com.test.snake").unwrap();
        let plist =
            std::fs::read_to_string(dir.path().join("snake.app/Contents/Info.plist")).unwrap();
        assert!(
            plist.contains("<string>com.test.snake</string>"),
            "got: {plist}"
        );
    }

    #[test]
    fn pkginfo_has_appl_type_and_no_creator_code() {
        let dir = tempdir().unwrap();
        stage_app_layout(dir.path(), "x", "com.usagiengine.x").unwrap();
        let pkg = std::fs::read(dir.path().join("x.app/Contents/PkgInfo")).unwrap();
        assert_eq!(pkg, b"APPL????");
    }

    #[test]
    fn info_plist_includes_required_keys_and_substituted_values() {
        let xml = info_plist_xml("snake", "com.usagiengine.snake");
        for key in [
            "CFBundleExecutable",
            "CFBundleIdentifier",
            "CFBundleName",
            "CFBundlePackageType",
            "CFBundleShortVersionString",
            "CFBundleVersion",
            "LSMinimumSystemVersion",
            "NSHighResolutionCapable",
        ] {
            assert!(xml.contains(key), "missing key {key} in plist");
        }
        assert!(xml.contains("<string>snake</string>"));
        assert!(xml.contains("<string>com.usagiengine.snake</string>"));
    }

    #[test]
    fn info_plist_escapes_xml_special_chars_in_name() {
        let xml = info_plist_xml("R&D <demo>", "com.usagiengine.rd-demo");
        assert!(xml.contains("R&amp;D &lt;demo&gt;"), "got: {xml}");
        assert!(!xml.contains("R&D"), "raw ampersand should be escaped");
    }
}
