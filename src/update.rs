//! `usagi update` replaces the running binary with the latest tagged release
//! from GitHub. Reuses the templates download + verify + extract pipeline (same
//! archives, same SHA-256 sidecars), then hands the extracted binary to
//! `self_replace` for the platform-specific in-place swap.

use crate::templates::{self, Runtime, Target};
use crate::{Error, Result, msg};
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};

const RELEASES_API: &str = "https://api.github.com/repos/brettchalupa/usagi/releases/latest";

/// Cap on the JSON body returned by the GitHub releases API. The
/// real response is ~16 KB; 1 MiB is comfortable headroom and bounds
/// the worst case if something upstream goes sideways.
const MAX_API_BYTES: u64 = 1024 * 1024;

pub fn run() -> Result<()> {
    let target = Target::host().ok_or_else(|| {
        Error::Cli(format!(
            "no published binary for this platform ({} / {}). \
             See https://github.com/brettchalupa/usagi/releases",
            std::env::consts::OS,
            std::env::consts::ARCH,
        ))
    })?;
    if matches!(target, Target::Wasm) {
        return Err(Error::Cli(
            "`usagi update` is not supported on the web build".into(),
        ));
    }

    let current = env!("CARGO_PKG_VERSION");
    if current.contains('-') {
        msg::warn!(
            "running pre-release version v{current}; skipping update. Rebuild from source or check out a release tag."
        );
        return Ok(());
    }

    let current_exe = std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .map_err(|e| Error::Cli(format!("resolving current_exe: {e}")))?;
    refuse_managed_install(&current_exe)?;
    let parent = current_exe.parent().ok_or_else(|| {
        Error::Cli(format!(
            "current_exe has no parent directory: {}",
            current_exe.display()
        ))
    })?;
    // Stage the download next to the running binary so the final
    // rename stays on the same filesystem (cross-volume rename is
    // EXDEV / os error 17). Doubles as a writability pre-flight: if
    // the parent dir isn't writable, this fails before we touch the
    // network.
    let scratch = tempfile::Builder::new()
        .prefix(".usagi-update-")
        .tempdir_in(parent)
        .map_err(|e| {
            Error::Cli(format!(
                "cannot stage update next to {}: {e}. \
                 The binary's directory must be writable; try with elevated permissions.",
                current_exe.display()
            ))
        })?;

    msg::info!("checking {RELEASES_API}");
    let latest = fetch_latest_version()?;
    if !is_valid_version(&latest) {
        return Err(Error::Cli(format!(
            "unexpected tag {latest:?} from GitHub; expected MAJOR.MINOR.PATCH"
        )));
    }

    if latest == current {
        msg::info!("already on latest: v{current}");
        return Ok(());
    }

    msg::info!("updating v{current} -> v{latest}");

    let archive = scratch.path().join(format!(
        "usagi-{latest}-{}.{}",
        target.platform_str(),
        target.archive_ext()
    ));
    let url = templates::template_url(&templates::template_base(), &latest, target);
    msg::info!("downloading {url}");
    templates::download_with_verify(&url, &archive)?;

    let extract_dir = scratch.path().join("extract");
    templates::extract(&archive, &extract_dir)?;
    let new_exe = match templates::locate(&extract_dir, target)? {
        Runtime::Native { exe } => exe,
        Runtime::Web { .. } => {
            return Err(Error::Cli(
                "expected native binary in update archive, found web runtime".into(),
            ));
        }
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&new_exe)
            .map_err(|e| Error::Cli(format!("stat {}: {e}", new_exe.display())))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&new_exe, perms)
            .map_err(|e| Error::Cli(format!("chmod {}: {e}", new_exe.display())))?;
    }

    let backup = backup_path(&current_exe);
    std::fs::copy(&current_exe, &backup).map_err(|e| {
        Error::Cli(format!(
            "backing up current binary to {}: {e}",
            backup.display()
        ))
    })?;

    self_replace::self_replace(&new_exe)
        .map_err(|e| Error::Cli(format!("replacing current executable: {e}")))?;

    msg::info!(
        "updated to v{latest}; previous binary saved to {}",
        backup.display()
    );
    Ok(())
}

/// Appends `.bak` to the path without replacing the existing extension,
/// so `usagi.exe` becomes `usagi.exe.bak` on Windows (still recognized
/// as a non-executable so it can't accidentally be invoked, and a `mv
/// usagi.exe.bak usagi.exe` is a clear rollback).
fn backup_path(exe: &Path) -> PathBuf {
    let mut s = OsString::from(exe);
    s.push(".bak");
    PathBuf::from(s)
}

/// Refuses to update binaries that look like they were placed by a
/// package manager, build tree, or shim. We don't ship to any of these
/// channels yet, so a hit here is almost certainly an end user running
/// `usagi update` against the wrong binary (cargo install, a dev
/// build, a Nix store path) and we'd just clobber state they didn't
/// expect us to touch.
fn refuse_managed_install(exe: &Path) -> Result<()> {
    let path = exe.to_string_lossy().replace('\\', "/");
    let blocked: &[&str] = &[
        "/.cargo/bin/",
        "/target/debug/",
        "/target/release/",
        "/nix/store/",
        "/opt/homebrew/",
        "/home/linuxbrew/",
        "/usr/local/Cellar/",
    ];
    if let Some(marker) = blocked.iter().find(|m| path.contains(*m)) {
        return Err(Error::Cli(format!(
            "refusing to update {}: path contains '{marker}'. \
             `usagi update` only replaces binaries downloaded from GitHub releases.",
            exe.display()
        )));
    }
    // Real UNC paths only. `\\?\` (verbatim) and `\\.\` (DOS device)
    // prefixes also start with `//` after separator normalization, but
    // `std::fs::canonicalize` routinely returns `\\?\C:\...` for normal
    // Windows installs — those must be allowed through.
    if path.starts_with("//") && !path.starts_with("//?/") && !path.starts_with("//./") {
        return Err(Error::Cli(format!(
            "refusing to update {}: network paths are not supported.",
            exe.display()
        )));
    }
    Ok(())
}

/// Loosely validates a tag the GitHub API returns before it flows into
/// a URL. Accepts `MAJOR.MINOR.PATCH` with an optional pre-release tail
/// (we'd never see one given `releases/latest` skips prereleases, but
/// the check costs nothing).
fn is_valid_version(s: &str) -> bool {
    let core = s.split('-').next().unwrap_or("");
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    parts
        .iter()
        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn fetch_latest_version() -> Result<String> {
    match ureq::get(RELEASES_API)
        .header("User-Agent", concat!("usagi/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .call()
    {
        Ok(mut response) => {
            let mut bytes = Vec::new();
            response
                .body_mut()
                .as_reader()
                .take(MAX_API_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| Error::Cli(format!("reading body of {RELEASES_API}: {e}")))?;
            if bytes.len() as u64 > MAX_API_BYTES {
                return Err(Error::Cli(format!(
                    "response from {RELEASES_API} exceeds {MAX_API_BYTES}-byte cap"
                )));
            }
            let text = String::from_utf8(bytes)
                .map_err(|e| Error::Cli(format!("non-utf8 body from {RELEASES_API}: {e}")))?;
            parse_tag_name(&text)
        }
        Err(ureq::Error::StatusCode(403)) => Err(Error::Cli(
            "GitHub API returned 403; likely rate limited (60 req/hour per IP). Try again later."
                .into(),
        )),
        Err(ureq::Error::StatusCode(404)) => Err(Error::Cli(
            "GitHub API returned 404; no published release found.".into(),
        )),
        Err(e) => Err(Error::Cli(format!("fetching {RELEASES_API}: {e}"))),
    }
}

fn parse_tag_name(json: &str) -> Result<String> {
    let v: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| Error::Cli(format!("parsing GitHub release JSON: {e}")))?;
    let tag = v
        .get("tag_name")
        .and_then(|t| t.as_str())
        .ok_or_else(|| Error::Cli("GitHub release JSON missing tag_name".into()))?;
    Ok(tag.trim_start_matches('v').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tag_name_strips_v_prefix() {
        let json = r#"{"tag_name": "v0.5.0", "name": "v0.5.0"}"#;
        assert_eq!(parse_tag_name(json).unwrap(), "0.5.0");
    }

    #[test]
    fn parses_tag_name_without_v_prefix() {
        let json = r#"{"tag_name": "0.5.0"}"#;
        assert_eq!(parse_tag_name(json).unwrap(), "0.5.0");
    }

    #[test]
    fn errors_when_tag_name_missing() {
        let json = r#"{"name": "v0.5.0"}"#;
        let err = parse_tag_name(json).unwrap_err();
        assert!(format!("{err}").contains("tag_name"));
    }

    #[test]
    fn errors_on_invalid_json() {
        let err = parse_tag_name("not json").unwrap_err();
        assert!(format!("{err}").contains("parsing GitHub release JSON"));
    }

    #[test]
    fn version_validator_accepts_release_and_prerelease() {
        assert!(is_valid_version("0.7.0"));
        assert!(is_valid_version("12.34.56"));
        assert!(is_valid_version("1.0.0-rc.1"));
        assert!(is_valid_version("0.7.0-dev"));
    }

    #[test]
    fn version_validator_rejects_garbage() {
        assert!(!is_valid_version("nightly"));
        assert!(!is_valid_version("v0.7.0"));
        assert!(!is_valid_version("0.7"));
        assert!(!is_valid_version("0.7.0.1"));
        assert!(!is_valid_version("0.a.0"));
        assert!(!is_valid_version(""));
        assert!(!is_valid_version("../../etc/passwd"));
    }

    #[test]
    fn backup_path_appends_bak_without_replacing_extension() {
        assert_eq!(
            backup_path(Path::new("/usr/local/bin/usagi")),
            PathBuf::from("/usr/local/bin/usagi.bak")
        );
        assert_eq!(
            backup_path(Path::new(r"C:\Tools\usagi.exe")),
            PathBuf::from(r"C:\Tools\usagi.exe.bak")
        );
    }

    #[test]
    fn refuses_cargo_install_path() {
        let p = PathBuf::from("/home/alice/.cargo/bin/usagi");
        let err = refuse_managed_install(&p).unwrap_err();
        assert!(format!("{err}").contains(".cargo/bin"));
    }

    #[test]
    fn refuses_cargo_target_release_build() {
        let p = PathBuf::from("/home/alice/code/usagi/target/release/usagi");
        let err = refuse_managed_install(&p).unwrap_err();
        assert!(format!("{err}").contains("target/release"));
    }

    #[test]
    fn refuses_homebrew_path() {
        let p = PathBuf::from("/opt/homebrew/bin/usagi");
        assert!(refuse_managed_install(&p).is_err());
    }

    #[test]
    fn refuses_nix_store_path() {
        let p = PathBuf::from("/nix/store/abc-usagi/bin/usagi");
        assert!(refuse_managed_install(&p).is_err());
    }

    #[test]
    fn refuses_unc_path() {
        // Path::to_string_lossy on Windows preserves backslashes; we
        // normalize them to '/' so the UNC check is portable.
        let p = PathBuf::from(r"\\server\share\usagi.exe");
        let err = refuse_managed_install(&p).unwrap_err();
        assert!(format!("{err}").contains("network paths"));
    }

    #[test]
    fn allows_normal_user_install() {
        assert!(refuse_managed_install(Path::new("/usr/local/bin/usagi")).is_ok());
        assert!(refuse_managed_install(Path::new("/home/alice/bin/usagi")).is_ok());
        assert!(refuse_managed_install(Path::new(r"C:\Tools\usagi.exe")).is_ok());
    }

    #[test]
    fn allows_windows_verbatim_local_path() {
        // `std::fs::canonicalize` on Windows returns these for normal
        // installs; they must not be rejected as UNC.
        assert!(refuse_managed_install(Path::new(r"\\?\C:\Tools\usagi.exe")).is_ok());
        assert!(refuse_managed_install(Path::new(r"\\.\C:\Tools\usagi.exe")).is_ok());
    }
}
