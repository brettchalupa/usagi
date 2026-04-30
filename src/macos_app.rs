//! macOS .app bundle staging for `usagi export --target macos`.
//!
//! Produces the standard layout Finder and Launch Services expect:
//!
//! ```text
//! <name>.app/
//!   Contents/
//!     Info.plist
//!     PkgInfo
//!     MacOS/<name>             (fused exe, +x via bundle::fuse)
//!     Resources/AppIcon.icns   (optional; only when icns_bytes is supplied)
//! ```
//!
//! Out of scope for now: code signing, notarization. Without a
//! signature the app launches via right-click then Open (or one
//! Security & Privacy bypass) on the end user's machine.
//!
//! The `CFBundleIdentifier` written here is whatever the caller resolved
//! via `game_id::resolve`, the same string the save-data layer
//! namespaces on, so save data and the .app bundle stay in lockstep.

use crate::{Error, Result};
use std::path::{Path, PathBuf};

const APP_ICON_FILE: &str = "AppIcon";

/// Creates `<stage>/<name>.app/Contents/{Info.plist,PkgInfo,MacOS/}` and
/// returns the path the caller should fuse the binary onto
/// (`Contents/MacOS/<name>`). bundle::fuse handles the `+x` chmod.
/// When `icns_bytes` is `Some`, also writes `Resources/AppIcon.icns`
/// and adds `CFBundleIconFile = AppIcon` to `Info.plist` so the
/// Finder / Dock pick up the game's icon.
pub fn stage_app_layout(
    stage: &Path,
    name: &str,
    bundle_id: &str,
    icns_bytes: Option<&[u8]>,
) -> Result<PathBuf> {
    let app = stage.join(format!("{name}.app"));
    let contents = app.join("Contents");
    let macos_dir = contents.join("MacOS");
    std::fs::create_dir_all(&macos_dir)
        .map_err(|e| Error::Cli(format!("creating {}: {e}", macos_dir.display())))?;

    if let Some(bytes) = icns_bytes {
        let resources = contents.join("Resources");
        std::fs::create_dir_all(&resources)
            .map_err(|e| Error::Cli(format!("creating {}: {e}", resources.display())))?;
        let icns_path = resources.join(format!("{APP_ICON_FILE}.icns"));
        std::fs::write(&icns_path, bytes)
            .map_err(|e| Error::Cli(format!("writing {}: {e}", icns_path.display())))?;
    }

    std::fs::write(
        contents.join("Info.plist"),
        info_plist_xml(name, bundle_id, icns_bytes.is_some()),
    )
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
/// - CFBundleIconFile=AppIcon when `with_icon` is true, pointing at
///   `Resources/AppIcon.icns` written by the staging step.
fn info_plist_xml(name: &str, bundle_id: &str, with_icon: bool) -> String {
    let icon_block = if with_icon {
        format!("    <key>CFBundleIconFile</key>\n    <string>{APP_ICON_FILE}</string>\n")
    } else {
        String::new()
    };
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
{icon_block}    <key>CFBundleIdentifier</key>
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
        let exe_path =
            stage_app_layout(dir.path(), "snake", "com.usagiengine.snake", None).unwrap();
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
        stage_app_layout(dir.path(), "snake", "com.test.snake", None).unwrap();
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
        stage_app_layout(dir.path(), "x", "com.usagiengine.x", None).unwrap();
        let pkg = std::fs::read(dir.path().join("x.app/Contents/PkgInfo")).unwrap();
        assert_eq!(pkg, b"APPL????");
    }

    #[test]
    fn stage_app_layout_writes_icns_and_references_it_when_supplied() {
        let dir = tempdir().unwrap();
        let fake_icns = b"icns\x00\x00\x00\x08";
        stage_app_layout(dir.path(), "snake", "com.test.snake", Some(fake_icns)).unwrap();
        let icns_path = dir.path().join("snake.app/Contents/Resources/AppIcon.icns");
        assert!(icns_path.is_file(), "AppIcon.icns should be written");
        assert_eq!(std::fs::read(&icns_path).unwrap(), fake_icns);
        let plist =
            std::fs::read_to_string(dir.path().join("snake.app/Contents/Info.plist")).unwrap();
        assert!(
            plist.contains("CFBundleIconFile"),
            "plist should reference the icon file: {plist}"
        );
        assert!(plist.contains("<string>AppIcon</string>"));
    }

    #[test]
    fn info_plist_omits_icon_key_when_no_icns_supplied() {
        let xml = info_plist_xml("snake", "com.usagiengine.snake", false);
        assert!(!xml.contains("CFBundleIconFile"), "got: {xml}");
    }

    #[test]
    fn info_plist_includes_required_keys_and_substituted_values() {
        let xml = info_plist_xml("snake", "com.usagiengine.snake", false);
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
        let xml = info_plist_xml("R&D <demo>", "com.usagiengine.rd-demo", false);
        assert!(xml.contains("R&amp;D &lt;demo&gt;"), "got: {xml}");
        assert!(!xml.contains("R&D"), "raw ampersand should be escaped");
    }
}
