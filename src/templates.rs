//! Runtime templates for `usagi export`: download or read a release
//! archive, extract it, locate the runtime files inside.
//!
//! Archives come from `.github/workflows/release.yml`:
//! - linux / macos: tar.gz with `usagi` at root
//! - windows: zip with `usagi.exe`
//! - wasm: tar.gz with `usagi.js`, `usagi.wasm`, `shell.html`
//!
//! `find_file` walks the extraction tree so the layout doesn't have to
//! be flat.

use crate::{Error, Result};
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// Names match the suffix in release artifacts (`usagi-<ver>-<target>.<ext>`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Linux,
    Macos,
    Windows,
    Wasm,
}

impl Target {
    pub const ALL: [Target; 4] = [Target::Linux, Target::Macos, Target::Windows, Target::Wasm];

    /// Short, user-facing name. Used for `--target` values, output zip
    /// basenames, and the cache key. A future `--target linux-arm` would
    /// be a new enum variant rather than a different `as_str` for `Linux`.
    pub fn as_str(self) -> &'static str {
        match self {
            Target::Linux => "linux",
            Target::Macos => "macos",
            Target::Windows => "windows",
            // User-facing label is "web" since that's the familiar term
            // for distribution. Release filename and runtime files keep
            // "wasm" (see `platform_str`).
            Target::Wasm => "web",
        }
    }

    /// Full platform string used in release-artifact filenames and the
    /// auto-fetch URL (`usagi-<ver>-<platform>.<ext>`). Carries the
    /// architecture so future multi-arch builds drop in without renaming:
    /// a Raspberry Pi binary would add a `Target::LinuxArm` => "linux-aarch64"
    /// row alongside this one.
    pub fn platform_str(self) -> &'static str {
        match self {
            Target::Linux => "linux-x86_64",
            Target::Macos => "macos-aarch64",
            Target::Windows => "windows-x86_64",
            Target::Wasm => "wasm",
        }
    }

    pub fn archive_ext(self) -> &'static str {
        match self {
            Target::Windows => "zip",
            _ => "tar.gz",
        }
    }

    /// The target matching the currently-running binary, if it's one of
    /// the platforms shipped as a release template. Used to short-circuit
    /// the auto-fetch path: a host-target export fuses against the
    /// running exe and never touches the network.
    pub fn host() -> Option<Target> {
        if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            Some(Target::Linux)
        } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            Some(Target::Macos)
        } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            Some(Target::Windows)
        } else {
            None
        }
    }
}

/// GitHub Releases page for the canonical project. Override at runtime
/// with `USAGI_TEMPLATE_BASE` for forks, mirrors, or air-gapped registries.
pub const DEFAULT_TEMPLATE_BASE: &str = "https://github.com/brettchalupa/usagi/releases/download";

pub fn template_base() -> String {
    std::env::var("USAGI_TEMPLATE_BASE").unwrap_or_else(|_| DEFAULT_TEMPLATE_BASE.to_string())
}

pub fn template_url(base_url: &str, version: &str, target: Target) -> String {
    let base = base_url.trim_end_matches('/');
    format!(
        "{base}/v{version}/usagi-{version}-{}.{}",
        target.platform_str(),
        target.archive_ext()
    )
}

/// Per-OS cache root: `<XDG_CACHE_HOME>/usagi/` on linux,
/// `~/Library/Caches/com.usagiengine.usagi/` on macOS,
/// `%LOCALAPPDATA%\usagiengine\usagi\cache\` on windows.
pub fn cache_dir() -> Result<PathBuf> {
    directories::ProjectDirs::from("com", "usagiengine", "usagi")
        .map(|d| d.cache_dir().to_path_buf())
        .ok_or_else(|| Error::Cli("could not resolve OS cache directory".into()))
}

/// Root of cached template extractions: `<cache_dir()>/templates/`.
pub fn cache_templates_root() -> Result<PathBuf> {
    Ok(cache_dir()?.join("templates"))
}

/// Prints `<version> <target> <bytes>` for every cached template entry.
/// Used by `usagi templates list`.
pub fn list_cache(root: &Path) -> Result<()> {
    if !root.exists() {
        println!("[usagi] no templates cached at {}", root.display());
        return Ok(());
    }
    let mut total: u64 = 0;
    let mut count = 0;
    for ver_entry in read_subdirs(root)? {
        let version = ver_entry.file_name();
        for tgt_entry in read_subdirs(&ver_entry.path())? {
            let target = tgt_entry.file_name();
            let bytes = dir_size(&tgt_entry.path())?;
            total += bytes;
            count += 1;
            println!(
                "{:<20} {:<10} {:>10} bytes",
                version.to_string_lossy(),
                target.to_string_lossy(),
                bytes,
            );
        }
    }
    println!(
        "[usagi] {count} cached, {total} bytes total at {}",
        root.display()
    );
    Ok(())
}

/// Wipes every cached template at `root`. Used by `usagi templates clear`.
pub fn clear_cache(root: &Path) -> Result<()> {
    if !root.exists() {
        println!("[usagi] nothing to clear at {}", root.display());
        return Ok(());
    }
    std::fs::remove_dir_all(root)
        .map_err(|e| Error::Cli(format!("clearing {}: {e}", root.display())))?;
    println!("[usagi] cleared {}", root.display());
    Ok(())
}

fn read_subdirs(dir: &Path) -> Result<Vec<std::fs::DirEntry>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)
        .map_err(|e| Error::Cli(format!("read_dir {}: {e}", dir.display())))?
    {
        let entry = entry.map_err(|e| Error::Cli(format!("read_dir entry: {e}")))?;
        if entry.path().is_dir() {
            out.push(entry);
        }
    }
    out.sort_by_key(|e| e.file_name());
    Ok(out)
}

fn dir_size(dir: &Path) -> Result<u64> {
    let mut total = 0;
    for entry in std::fs::read_dir(dir)
        .map_err(|e| Error::Cli(format!("read_dir {}: {e}", dir.display())))?
    {
        let entry = entry.map_err(|e| Error::Cli(format!("read_dir entry: {e}")))?;
        let path = entry.path();
        if path.is_dir() {
            total += dir_size(&path)?;
        } else {
            total += std::fs::metadata(&path)
                .map(|m| m.len())
                .map_err(|e| Error::Cli(format!("stat {}: {e}", path.display())))?;
        }
    }
    Ok(total)
}

/// Returns a cached, extracted template directory for `(version, target)`,
/// downloading + extracting if needed. `no_cache` forces a re-download.
pub fn ensure_cached(
    cache_root: &Path,
    base_url: &str,
    version: &str,
    target: Target,
    no_cache: bool,
) -> Result<PathBuf> {
    let dir = cache_root
        .join("templates")
        .join(version)
        .join(target.as_str());
    if !no_cache && dir.exists() && locate(&dir, target).is_ok() {
        return Ok(dir);
    }
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .map_err(|e| Error::Cli(format!("clearing stale cache {}: {e}", dir.display())))?;
    }
    let scratch =
        tempfile::tempdir().map_err(|e| Error::Cli(format!("creating download tmpdir: {e}")))?;
    let archive = scratch.path().join(format!(
        "usagi-{version}-{}.{}",
        target.platform_str(),
        target.archive_ext()
    ));
    let url = template_url(base_url, version, target);
    println!("[usagi] downloading {url}");
    download_with_verify(&url, &archive).map_err(|e| match e {
        Error::Cli(msg) => Error::Cli(format!(
            "{msg}. If this version isn't published, pass --template-path or --template-url."
        )),
        other => other,
    })?;
    extract(&archive, &dir)?;
    Ok(dir)
}

#[derive(Debug)]
pub enum Runtime {
    /// Native target: one executable to fuse a bundle onto.
    Native { exe: PathBuf },
    /// Web target: separate runtime files written next to the bundle.
    Web {
        js: PathBuf,
        wasm: PathBuf,
        html: PathBuf,
    },
}

/// Streams `url` to `dest`. ureq's default `http_status_as_error` means
/// 4xx/5xx surface as `call()` errors with the status in the message.
pub fn download(url: &str, dest: &Path) -> Result<()> {
    let mut response = ureq::get(url)
        .call()
        .map_err(|e| Error::Cli(format!("downloading {url}: {e}")))?;
    let mut out =
        File::create(dest).map_err(|e| Error::Cli(format!("creating {}: {e}", dest.display())))?;
    let mut body = response.body_mut().as_reader();
    std::io::copy(&mut body, &mut out)
        .map_err(|e| Error::Cli(format!("writing {}: {e}", dest.display())))?;
    Ok(())
}

/// Downloads `archive_url` to `dest` and verifies it against the
/// published `<archive_url>.sha256` sidecar. Used for auto-fetch and
/// `--template-url`. `--template-path` skips verification (the user is
/// supplying the archive directly).
pub fn download_with_verify(archive_url: &str, dest: &Path) -> Result<()> {
    download(archive_url, dest)?;
    let sidecar_url = format!("{archive_url}.sha256");
    let sidecar_text = fetch_text(&sidecar_url).map_err(|e| match e {
        Error::Cli(msg) => Error::Cli(format!("fetching sha256 sidecar: {msg}")),
        other => other,
    })?;
    let expected = parse_sha256_line(&sidecar_text)?;
    verify_sha256(dest, &expected)
}

fn fetch_text(url: &str) -> Result<String> {
    let mut response = ureq::get(url)
        .call()
        .map_err(|e| Error::Cli(format!("downloading {url}: {e}")))?;
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| Error::Cli(format!("reading body of {url}: {e}")))?;
    String::from_utf8(bytes).map_err(|e| Error::Cli(format!("non-utf8 body from {url}: {e}")))
}

/// Parses a `sha256sum`-style sidecar (`<hex>  <filename>`). Tolerates
/// single- or double-space separators and leading whitespace.
pub fn parse_sha256_line(text: &str) -> Result<String> {
    let token = text
        .split_whitespace()
        .next()
        .ok_or_else(|| Error::Cli("empty sha256 sidecar".into()))?;
    if token.len() != 64 || !token.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(Error::Cli(format!("malformed sha256 in sidecar: {token}")));
    }
    Ok(token.to_ascii_lowercase())
}

/// Streams `path` through SHA-256 and compares to `expected_hex`
/// (case-insensitive). Errors loudly on mismatch.
pub fn verify_sha256(path: &Path, expected_hex: &str) -> Result<()> {
    use sha2::{Digest, Sha256};
    let mut f = File::open(path)
        .map_err(|e| Error::Cli(format!("opening {} for verify: {e}", path.display())))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| Error::Cli(format!("reading {} for verify: {e}", path.display())))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hasher.finalize();
    let actual: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    if actual != expected_hex.to_ascii_lowercase() {
        return Err(Error::Cli(format!(
            "sha256 mismatch for {}: expected {expected_hex}, got {actual}",
            path.display()
        )));
    }
    Ok(())
}

/// Auto-detects tar.gz vs zip from the file extension.
pub fn extract(archive: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)
        .map_err(|e| Error::Cli(format!("create extraction dir {}: {e}", dest.display())))?;
    match ArchiveKind::detect(archive)? {
        ArchiveKind::TarGz => extract_tar_gz(archive, dest),
        ArchiveKind::Zip => extract_zip(archive, dest),
    }
}

/// Walks the extracted tree so the archive's layout can be flat or nested.
pub fn locate(dir: &Path, target: Target) -> Result<Runtime> {
    match target {
        Target::Linux | Target::Macos => Ok(Runtime::Native {
            exe: find_file(dir, "usagi")?,
        }),
        Target::Windows => Ok(Runtime::Native {
            exe: find_file(dir, "usagi.exe")?,
        }),
        Target::Wasm => {
            let js = find_file(dir, "usagi.js")?;
            let wasm = find_file(dir, "usagi.wasm")?;
            // Fall back to the source tree's shell.html for templates
            // built before release.yml started bundling it.
            let html = find_file(dir, "shell.html")
                .ok()
                .or_else(|| Some(PathBuf::from("web/shell.html")).filter(|p| p.is_file()))
                .ok_or_else(|| {
                    Error::Cli(
                        "shell.html missing from wasm template and no fallback at web/shell.html"
                            .into(),
                    )
                })?;
            Ok(Runtime::Web { js, wasm, html })
        }
    }
}

enum ArchiveKind {
    TarGz,
    Zip,
}

impl ArchiveKind {
    fn detect(path: &Path) -> Result<Self> {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| Error::Cli(format!("invalid archive path: {}", path.display())))?;
        if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
            Ok(Self::TarGz)
        } else if name.ends_with(".zip") {
            Ok(Self::Zip)
        } else {
            Err(Error::Cli(format!(
                "unsupported archive extension (want .tar.gz, .tgz, or .zip): {}",
                path.display()
            )))
        }
    }
}

fn extract_tar_gz(archive: &Path, dest: &Path) -> Result<()> {
    let f = File::open(archive)
        .map_err(|e| Error::Cli(format!("opening {}: {e}", archive.display())))?;
    let gz = flate2::read::GzDecoder::new(f);
    let mut tar = tar::Archive::new(gz);
    tar.unpack(dest)
        .map_err(|e| Error::Cli(format!("extracting {}: {e}", archive.display())))?;
    Ok(())
}

fn extract_zip(archive: &Path, dest: &Path) -> Result<()> {
    let f = File::open(archive)
        .map_err(|e| Error::Cli(format!("opening {}: {e}", archive.display())))?;
    let mut z = zip::ZipArchive::new(f)
        .map_err(|e| Error::Cli(format!("reading zip {}: {e}", archive.display())))?;
    z.extract(dest)
        .map_err(|e| Error::Cli(format!("extracting {}: {e}", archive.display())))?;
    Ok(())
}

/// Recursive search for a file by exact name. Returns the first match.
fn find_file(dir: &Path, name: &str) -> Result<PathBuf> {
    fn walk(dir: &Path, name: &str) -> io::Result<Option<PathBuf>> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let ft = entry.file_type()?;
            if ft.is_file() && entry.file_name() == name {
                return Ok(Some(entry.path()));
            } else if ft.is_dir()
                && let Some(found) = walk(&entry.path(), name)?
            {
                return Ok(Some(found));
            }
        }
        Ok(None)
    }
    walk(dir, name)
        .map_err(|e| Error::Cli(format!("scanning {} for {name}: {e}", dir.display())))?
        .ok_or_else(|| {
            Error::Cli(format!(
                "{name} not found in extracted template at {}",
                dir.display()
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use tempfile::tempdir;

    /// Spins up an HTTP/1.1 server on 127.0.0.1 that handles each
    /// request with the next response in `responses`. Returns the
    /// `http://...` base URL.
    fn canned_server(responses: Vec<(&'static str, Vec<u8>)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for resp in responses {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                let header = format!(
                    "HTTP/1.1 {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    resp.0,
                    resp.1.len(),
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(&resp.1);
            }
        });
        format!("http://127.0.0.1:{port}/")
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    /// Spins up a one-shot HTTP/1.1 server on 127.0.0.1, accepts a single
    /// connection, returns the canned response, then exits. Returns the
    /// `http://...` URL the caller should hit.
    fn one_shot_server(status_line: &'static str, body: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 4096];
            // Drain the request preamble; not parsed.
            let _ = stream.read(&mut buf);
            let header = format!(
                "HTTP/1.1 {status_line}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len(),
            );
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(&body);
        });
        format!("http://127.0.0.1:{port}/")
    }

    /// Build a tar.gz in-memory from `(name, contents)` pairs.
    fn make_tar_gz(files: &[(&str, &[u8])]) -> Vec<u8> {
        let buf = Vec::new();
        let gz = flate2::write::GzEncoder::new(buf, flate2::Compression::fast());
        let mut tar = tar::Builder::new(gz);
        for (name, contents) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            tar.append_data(&mut header, name, *contents).unwrap();
        }
        let gz = tar.into_inner().unwrap();
        gz.finish().unwrap()
    }

    fn make_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let cursor = std::io::Cursor::new(&mut buf);
            let mut z = zip::ZipWriter::new(cursor);
            let opts: zip::write::SimpleFileOptions =
                zip::write::SimpleFileOptions::default().unix_permissions(0o755);
            for (name, contents) in files {
                z.start_file(*name, opts).unwrap();
                z.write_all(contents).unwrap();
            }
            z.finish().unwrap();
        }
        buf
    }

    fn write_archive(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    }

    #[test]
    fn extracts_linux_tarball_and_locates_usagi() {
        let archive_dir = tempdir().unwrap();
        let archive = write_archive(
            archive_dir.path(),
            "usagi-x-linux.tar.gz",
            &make_tar_gz(&[("usagi", b"#!/bin/sh\nexit 0\n")]),
        );
        let extract_dir = tempdir().unwrap();
        extract(&archive, extract_dir.path()).unwrap();
        let rt = locate(extract_dir.path(), Target::Linux).unwrap();
        match rt {
            Runtime::Native { exe } => {
                assert!(exe.is_file());
                assert_eq!(exe.file_name().unwrap(), "usagi");
            }
            _ => panic!("expected Native runtime"),
        }
    }

    #[test]
    fn extracts_macos_tarball_with_same_layout_as_linux() {
        let archive_dir = tempdir().unwrap();
        let archive = write_archive(
            archive_dir.path(),
            "usagi-x-macos.tar.gz",
            &make_tar_gz(&[("usagi", b"\xCF\xFA\xED\xFEmacho-stub")]),
        );
        let extract_dir = tempdir().unwrap();
        extract(&archive, extract_dir.path()).unwrap();
        let rt = locate(extract_dir.path(), Target::Macos).unwrap();
        match rt {
            Runtime::Native { exe } => assert!(exe.is_file()),
            _ => panic!("expected Native runtime"),
        }
    }

    #[test]
    fn extracts_windows_zip_and_locates_usagi_exe() {
        let archive_dir = tempdir().unwrap();
        let archive = write_archive(
            archive_dir.path(),
            "usagi-x-windows.zip",
            &make_zip(&[("usagi.exe", b"MZpe-stub")]),
        );
        let extract_dir = tempdir().unwrap();
        extract(&archive, extract_dir.path()).unwrap();
        let rt = locate(extract_dir.path(), Target::Windows).unwrap();
        match rt {
            Runtime::Native { exe } => {
                assert!(exe.is_file());
                assert_eq!(exe.file_name().unwrap(), "usagi.exe");
            }
            _ => panic!("expected Native runtime"),
        }
    }

    #[test]
    fn extracts_wasm_tarball_and_locates_all_three_files() {
        let archive_dir = tempdir().unwrap();
        let archive = write_archive(
            archive_dir.path(),
            "usagi-x-wasm.tar.gz",
            &make_tar_gz(&[
                ("usagi.js", b"// js"),
                ("usagi.wasm", b"\x00asm"),
                ("shell.html", b"<!doctype html>"),
            ]),
        );
        let extract_dir = tempdir().unwrap();
        extract(&archive, extract_dir.path()).unwrap();
        let rt = locate(extract_dir.path(), Target::Wasm).unwrap();
        match rt {
            Runtime::Web { js, wasm, html } => {
                assert!(js.is_file());
                assert!(wasm.is_file());
                assert!(html.is_file());
            }
            _ => panic!("expected Web runtime"),
        }
    }

    #[test]
    fn locate_finds_nested_runtime_files() {
        // Some archivers (7-zip with absolute paths, in particular) preserve
        // a directory prefix. The locator must walk to find the file.
        let archive_dir = tempdir().unwrap();
        let archive = write_archive(
            archive_dir.path(),
            "nested.tar.gz",
            &make_tar_gz(&[("target/release/usagi", b"stub")]),
        );
        let extract_dir = tempdir().unwrap();
        extract(&archive, extract_dir.path()).unwrap();
        let rt = locate(extract_dir.path(), Target::Linux).unwrap();
        match rt {
            Runtime::Native { exe } => {
                // Found nested at target/release/usagi.
                assert!(exe.is_file());
                assert!(exe.ends_with("target/release/usagi"));
            }
            _ => panic!("expected Native runtime"),
        }
    }

    #[test]
    fn locate_errors_when_runtime_missing() {
        let archive_dir = tempdir().unwrap();
        let archive = write_archive(
            archive_dir.path(),
            "empty.tar.gz",
            &make_tar_gz(&[("README", b"unrelated")]),
        );
        let extract_dir = tempdir().unwrap();
        extract(&archive, extract_dir.path()).unwrap();
        let err = locate(extract_dir.path(), Target::Linux).unwrap_err();
        match err {
            Error::Cli(msg) => assert!(msg.contains("usagi"), "got: {msg}"),
            _ => panic!("expected Cli error"),
        }
    }

    #[test]
    fn detect_rejects_unknown_extension() {
        let dir = tempdir().unwrap();
        let bogus = dir.path().join("template.7z");
        std::fs::write(&bogus, b"").unwrap();
        let dest = tempdir().unwrap();
        let err = extract(&bogus, dest.path()).unwrap_err();
        match err {
            Error::Cli(msg) => assert!(msg.contains("unsupported"), "got: {msg}"),
            _ => panic!("expected Cli error"),
        }
    }

    #[test]
    fn download_writes_response_body_to_dest() {
        let payload = make_tar_gz(&[("usagi", b"stub")]);
        let url = one_shot_server("200 OK", payload.clone());
        let dir = tempdir().unwrap();
        let dest = dir.path().join("template.tar.gz");
        download(&url, &dest).unwrap();
        let got = std::fs::read(&dest).unwrap();
        assert_eq!(got, payload);
    }

    #[test]
    fn download_then_extract_then_locate_round_trip() {
        let payload = make_tar_gz(&[("usagi", b"stub")]);
        let url = one_shot_server("200 OK", payload);
        let dir = tempdir().unwrap();
        let archive = dir.path().join("template.tar.gz");
        download(&url, &archive).unwrap();
        let extract_dir = dir.path().join("extracted");
        extract(&archive, &extract_dir).unwrap();
        let rt = locate(&extract_dir, Target::Linux).unwrap();
        assert!(matches!(rt, Runtime::Native { .. }));
    }

    #[test]
    fn download_errors_on_404_and_includes_status() {
        let url = one_shot_server("404 Not Found", b"missing".to_vec());
        let dir = tempdir().unwrap();
        let dest = dir.path().join("template.tar.gz");
        let err = download(&url, &dest).unwrap_err();
        match err {
            Error::Cli(msg) => assert!(msg.contains("404"), "got: {msg}"),
            _ => panic!("expected Cli error"),
        }
    }

    #[test]
    fn download_errors_on_connection_refused_with_url_in_message() {
        // Bind, capture port, then drop the listener so the port is closed.
        let port = {
            let l = TcpListener::bind("127.0.0.1:0").unwrap();
            l.local_addr().unwrap().port()
        };
        let url = format!("http://127.0.0.1:{port}/");
        let dir = tempdir().unwrap();
        let dest = dir.path().join("template.tar.gz");
        let err = download(&url, &dest).unwrap_err();
        match err {
            Error::Cli(msg) => assert!(msg.contains(&url), "got: {msg}"),
            _ => panic!("expected Cli error"),
        }
    }

    #[test]
    fn host_matches_export_target() {
        let host = Target::host();
        if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            assert_eq!(host, Some(Target::Linux));
        } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            assert_eq!(host, Some(Target::Macos));
        } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            assert_eq!(host, Some(Target::Windows));
        } else {
            assert_eq!(host, None);
        }
    }

    #[test]
    fn template_url_includes_architecture_in_filename() {
        let url = template_url("https://example.com/dl", "0.1.0", Target::Linux);
        assert_eq!(
            url,
            "https://example.com/dl/v0.1.0/usagi-0.1.0-linux-x86_64.tar.gz"
        );
    }

    #[test]
    fn template_url_strips_trailing_slash() {
        let url = template_url("https://example.com/dl/", "0.1.0", Target::Windows);
        assert_eq!(
            url,
            "https://example.com/dl/v0.1.0/usagi-0.1.0-windows-x86_64.zip"
        );
    }

    #[test]
    fn wasm_template_url_has_no_arch_suffix() {
        let url = template_url("https://example.com", "0.1.0", Target::Wasm);
        assert_eq!(url, "https://example.com/v0.1.0/usagi-0.1.0-wasm.tar.gz");
    }

    #[test]
    fn ensure_cached_fetches_extracts_on_cold_cache() {
        let payload = make_tar_gz(&[("usagi", b"stub")]);
        let hash = sha256_hex(&payload);
        let sidecar = format!("{hash}  archive.tar.gz\n").into_bytes();
        let base_url = canned_server(vec![("200 OK", payload), ("200 OK", sidecar)]);
        let base = base_url.trim_end_matches('/');
        let cache = tempdir().unwrap();
        let dir = ensure_cached(cache.path(), base, "0.1.0", Target::Linux, false).unwrap();
        assert!(dir.join("usagi").is_file());
        assert_eq!(dir, cache.path().join("templates/0.1.0/linux"));
    }

    #[test]
    fn ensure_cached_skips_network_on_warm_cache() {
        // Pre-populate cache with a synthetic runtime; point base at a
        // closed port. If ensure_cached tries to fetch, the test fails.
        let cache = tempdir().unwrap();
        let dir = cache.path().join("templates/0.1.0/linux");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("usagi"), b"prefilled").unwrap();
        let dead_base = "http://127.0.0.1:1";
        let got = ensure_cached(cache.path(), dead_base, "0.1.0", Target::Linux, false).unwrap();
        assert_eq!(got, dir);
        assert_eq!(std::fs::read(dir.join("usagi")).unwrap(), b"prefilled");
    }

    #[test]
    fn ensure_cached_forces_redownload_when_no_cache() {
        let cache = tempdir().unwrap();
        let dir = cache.path().join("templates/0.1.0/linux");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("usagi"), b"old").unwrap();
        let payload = make_tar_gz(&[("usagi", b"fresh")]);
        let hash = sha256_hex(&payload);
        let sidecar = format!("{hash}  archive.tar.gz\n").into_bytes();
        let base_url = canned_server(vec![("200 OK", payload), ("200 OK", sidecar)]);
        let base = base_url.trim_end_matches('/');
        let got = ensure_cached(cache.path(), base, "0.1.0", Target::Linux, true).unwrap();
        assert_eq!(std::fs::read(got.join("usagi")).unwrap(), b"fresh");
    }

    #[test]
    fn ensure_cached_surfaces_404_with_helpful_hint() {
        let url = one_shot_server("404 Not Found", b"missing".to_vec());
        let base = url.trim_end_matches('/');
        let cache = tempdir().unwrap();
        let err = ensure_cached(cache.path(), base, "0.1.0", Target::Linux, false).unwrap_err();
        match err {
            Error::Cli(msg) => {
                assert!(msg.contains("404"), "got: {msg}");
                assert!(msg.contains("--template-path"), "got: {msg}");
            }
            _ => panic!("expected Cli error"),
        }
    }

    #[test]
    fn verify_sha256_matches_expected_hex() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("data");
        std::fs::write(&p, b"hello").unwrap();
        let expected = sha256_hex(b"hello");
        verify_sha256(&p, &expected).unwrap();
    }

    #[test]
    fn verify_sha256_errors_on_mismatch() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("data");
        std::fs::write(&p, b"hello").unwrap();
        let err = verify_sha256(&p, &"0".repeat(64)).unwrap_err();
        match err {
            Error::Cli(msg) => assert!(msg.contains("mismatch"), "got: {msg}"),
            _ => panic!("expected Cli error"),
        }
    }

    #[test]
    fn parse_sha256_line_accepts_double_space_format() {
        let hex = "a".repeat(64);
        let line = format!("{hex}  some-file.tar.gz\n");
        assert_eq!(parse_sha256_line(&line).unwrap(), hex);
    }

    #[test]
    fn parse_sha256_line_rejects_non_hex() {
        let line = "zzz  bogus";
        assert!(parse_sha256_line(line).is_err());
    }

    #[test]
    fn download_with_verify_passes_when_sidecar_matches() {
        let payload = b"archive-bytes".to_vec();
        let hash = sha256_hex(&payload);
        let sidecar = format!("{hash}  archive.tar.gz\n").into_bytes();
        let base = canned_server(vec![("200 OK", payload.clone()), ("200 OK", sidecar)]);
        let archive_url = format!("{}{}", base, "archive.tar.gz");
        let dir = tempdir().unwrap();
        let dest = dir.path().join("archive.tar.gz");
        download_with_verify(&archive_url, &dest).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), payload);
    }

    #[test]
    fn download_with_verify_errors_on_hash_mismatch() {
        let payload = b"actual bytes".to_vec();
        let bad_sidecar = format!("{}  archive.tar.gz\n", "0".repeat(64)).into_bytes();
        let base = canned_server(vec![("200 OK", payload), ("200 OK", bad_sidecar)]);
        let archive_url = format!("{}{}", base, "archive.tar.gz");
        let dir = tempdir().unwrap();
        let dest = dir.path().join("archive.tar.gz");
        let err = download_with_verify(&archive_url, &dest).unwrap_err();
        match err {
            Error::Cli(msg) => assert!(msg.contains("mismatch"), "got: {msg}"),
            _ => panic!("expected Cli error"),
        }
    }

    #[test]
    fn download_with_verify_errors_when_sidecar_missing() {
        let payload = b"archive".to_vec();
        let base = canned_server(vec![("200 OK", payload), ("404 Not Found", b"".to_vec())]);
        let archive_url = format!("{}{}", base, "archive.tar.gz");
        let dir = tempdir().unwrap();
        let dest = dir.path().join("archive.tar.gz");
        let err = download_with_verify(&archive_url, &dest).unwrap_err();
        match err {
            Error::Cli(msg) => assert!(msg.contains("sha256 sidecar"), "got: {msg}"),
            _ => panic!("expected Cli error"),
        }
    }

    #[test]
    fn detect_accepts_tgz_short_form() {
        let dir = tempdir().unwrap();
        let archive = write_archive(
            dir.path(),
            "template.tgz",
            &make_tar_gz(&[("usagi", b"stub")]),
        );
        let dest = tempdir().unwrap();
        extract(&archive, dest.path()).unwrap();
        let rt = locate(dest.path(), Target::Linux).unwrap();
        assert!(matches!(rt, Runtime::Native { .. }));
    }
}
