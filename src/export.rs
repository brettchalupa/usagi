//! `usagi export`: package a game for distribution. Resolves a runtime
//! template (cache, `--template-path`, `--template-url`, or the host
//! binary), fuses the bundle, zips the result.

use crate::bundle::Bundle;
use crate::cli;
use crate::game_id;
use crate::macos_app;
use crate::templates;
use crate::{Error, Result};
use clap::ValueEnum;
use std::path::{Path, PathBuf};

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum ExportTarget {
    /// All four platform zips plus the portable `.usagi` bundle.
    All,
    /// Portable `.usagi` bundle file (run with `usagi run`).
    Bundle,
    /// Linux x86_64 fused exe, packaged as `<name>-linux.zip`.
    Linux,
    /// macOS aarch64 fused exe, packaged as `<name>-macos.zip`.
    Macos,
    /// Windows x86_64 fused exe, packaged as `<name>-windows.zip`.
    Windows,
    /// Web export packaged as `<name>-web.zip` (index.html + usagi.{js,wasm} + game.usagi).
    Web,
}

/// Top-level entry from `Command::Export`. Validates flag combinations,
/// builds the bundle, then dispatches to the target-specific path.
pub fn run(
    path_arg: &str,
    output: Option<&str>,
    target: ExportTarget,
    template_path: Option<&str>,
    template_url: Option<&str>,
    no_cache: bool,
    web_shell: Option<&str>,
) -> Result<()> {
    let script_path = PathBuf::from(cli::resolve_script_path(path_arg)?);
    // Canonicalize so `usagi export .` from inside the project dir gives
    // the dir's name, not "main" (project_name keys off the script's
    // parent, and "." has no file_name).
    let script_path = script_path.canonicalize().unwrap_or(script_path);
    let bundle = Bundle::from_project(&script_path).map_err(|e| {
        Error::Cli(format!(
            "building bundle from {}: {e}",
            script_path.display()
        ))
    })?;
    let path_hint = path_name_hint(&script_path).to_owned();

    let template_target = template_target_for(target);
    if template_target.is_none() && (template_path.is_some() || template_url.is_some()) {
        return Err(Error::Cli(
            "--template-path / --template-url only apply to \
             --target {linux,macos,windows,web}"
                .into(),
        ));
    }
    if web_shell.is_some() && !target_produces_web(target) {
        return Err(Error::Cli(
            "--web-shell only applies to --target {web,all}".into(),
        ));
    }

    let web_shell_override = resolve_web_shell_override(&script_path, web_shell)?;
    // Read `_config()` once for the whole export (game_id, icon, name,
    // and any future bundle metadata all consume this struct).
    // Failures fall back to defaults so a broken project file
    // doesn't fail the export.
    let project_config = crate::config::Config::read_for_export(&script_path);
    let project_name =
        crate::project_name::ProjectName::resolve(project_config.name.as_deref(), Some(&path_hint));
    // GameId keys off the path hint (or `_config().game_id`), not the
    // display name, so renaming the game doesn't migrate save data.
    let bundle_id = game_id::resolve_for_export(&project_config, &path_hint, &bundle);
    // Slice the configured sprite tile (or use the embedded
    // default) and pack it as a multi-resolution ICNS for the
    // macOS bundle. Errors are logged and the export continues
    // without an icon.
    let app_icns: Option<Vec<u8>> =
        match crate::icon::resolve_icns_for_export(&project_config, &script_path) {
            Ok(bytes) => Some(bytes),
            Err(e) => {
                eprintln!("[usagi] icon: {e}; macOS bundle will ship without an icon");
                None
            }
        };
    let opts = Opts {
        template_path,
        template_url,
        no_cache,
        web_shell_override: web_shell_override.as_deref(),
        // CFBundleIdentifier and platform package identifiers want a
        // plain string; we hand the inner str off here rather than
        // pass `GameId` through every export-target helper.
        bundle_id: bundle_id.as_str(),
        icns_bytes: app_icns.as_deref(),
    };
    let out_path = output
        .map(PathBuf::from)
        .unwrap_or_else(|| default_output_path(&project_name, target));

    match target {
        ExportTarget::All => export_all(&bundle, &project_name, &out_path, &opts),
        ExportTarget::Bundle => write_bundle(&bundle, &out_path),
        ExportTarget::Linux | ExportTarget::Macos | ExportTarget::Windows | ExportTarget::Web => {
            let target_kind = template_target.expect("validated above");
            export_one_target(&bundle, &project_name, target_kind, &opts, &out_path)
        }
    }
}

/// Inputs that flow from the CLI into per-target export steps. Grouped
/// to keep call sites readable as the option set grows.
struct Opts<'a> {
    template_path: Option<&'a str>,
    template_url: Option<&'a str>,
    no_cache: bool,
    web_shell_override: Option<&'a Path>,
    /// Pre-resolved id from `game_id::resolve`. Same string the save layer
    /// keys off, so save data and CFBundleIdentifier stay aligned.
    bundle_id: &'a str,
    /// Pre-encoded ICNS bytes for the macOS bundle. `None` means the
    /// `.app` ships without an icon (Linux/Windows/web targets ignore
    /// this field).
    icns_bytes: Option<&'a [u8]>,
}

/// Builds every cross-platform zip plus the portable `.usagi` bundle.
/// The host target fuses against the running binary (offline); the
/// others come from the cache, downloading on first use.
///
/// Per-target failures are logged and the loop keeps going. The common
/// case for this is a dev checkout exporting at a version that hasn't
/// been published yet (`0.x-dev`): the network template fetch 404s,
/// but the host-fuse zip plus the portable `.usagi` bundle should
/// still land. The whole call only fails if every target failed.
fn export_all(
    bundle: &Bundle,
    project_name: &crate::project_name::ProjectName,
    out_dir: &Path,
    opts: &Opts,
) -> Result<()> {
    std::fs::create_dir_all(out_dir)
        .map_err(|e| Error::Cli(format!("creating export dir {}: {e}", out_dir.display())))?;
    // --target all walks every platform via the cache; per-target archive
    // overrides don't apply.
    let inner = Opts {
        template_path: None,
        template_url: None,
        no_cache: opts.no_cache,
        web_shell_override: opts.web_shell_override,
        bundle_id: opts.bundle_id,
        icns_bytes: opts.icns_bytes,
    };
    let slug = project_name.slug();
    let mut succeeded = 0;
    let mut last_err: Option<Error> = None;
    for target in templates::Target::ALL {
        let zip = out_dir.join(format!("{slug}-{}.zip", target.as_str()));
        match export_one_target(bundle, project_name, target, &inner, &zip) {
            Ok(()) => succeeded += 1,
            Err(e) => {
                eprintln!("[usagi] skipping {target:?}: {e}");
                last_err = Some(e);
            }
        }
    }
    // The portable bundle never depends on a runtime template, so it stands
    // on its own as a successful artifact.
    write_bundle(bundle, &out_dir.join(format!("{slug}.usagi")))?;
    if succeeded == 0
        && let Some(e) = last_err
    {
        return Err(e);
    }
    println!("[usagi] export ready at {}/", out_dir.display());
    Ok(())
}

/// Resolves a runtime for `target` from one of: explicit `--template-path`
/// archive, explicit `--template-url` download, the running binary (when
/// `target` matches the host, no network), or the shared cache
/// (auto-fetched by version).
fn export_one_target(
    bundle: &Bundle,
    project_name: &crate::project_name::ProjectName,
    target: templates::Target,
    opts: &Opts,
    out_path: &Path,
) -> Result<()> {
    if let Some(p) = opts.template_path {
        let path = Path::new(p);
        // A directory is treated as a pre-extracted template; a file goes
        // through extract first. This is what makes local web iteration
        // ergonomic (`--template-path target/wasm32-.../release`).
        if path.is_dir() {
            return export_from_runtime_dir(bundle, project_name, path, target, opts, out_path);
        }
        return export_from_archive(bundle, project_name, path, target, opts, out_path);
    }
    if let Some(url) = opts.template_url {
        let dl = tempfile::tempdir()
            .map_err(|e| Error::Cli(format!("creating download tmpdir: {e}")))?;
        let archive = dl.path().join(archive_name_from_url(url));
        println!("[usagi] downloading {url}");
        templates::download_with_verify(url, &archive)?;
        return export_from_archive(bundle, project_name, &archive, target, opts, out_path);
    }
    if templates::Target::host() == Some(target) {
        return export_from_host_exe(bundle, project_name, target, opts, out_path);
    }
    let cache_root = templates::cache_dir()?;
    let base = templates::template_base();
    let runtime_dir = templates::ensure_cached(
        &cache_root,
        &base,
        env!("CARGO_PKG_VERSION"),
        target,
        opts.no_cache,
    )?;
    export_from_runtime_dir(bundle, project_name, &runtime_dir, target, opts, out_path)
}

/// Fuses against the currently-running binary. Used when the requested
/// target matches the host: no network, no cache lookup.
fn export_from_host_exe(
    bundle: &Bundle,
    project_name: &crate::project_name::ProjectName,
    target: templates::Target,
    opts: &Opts,
    out_path: &Path,
) -> Result<()> {
    let current_exe =
        std::env::current_exe().map_err(|e| Error::Cli(format!("locating current exe: {e}")))?;
    let stage =
        tempfile::tempdir().map_err(|e| Error::Cli(format!("creating zip stage dir: {e}")))?;
    let staged_exe = staged_binary_path(
        stage.path(),
        project_name,
        target,
        opts.bundle_id,
        opts.icns_bytes,
    )?;
    fuse_exe(bundle, &current_exe, &staged_exe)?;
    ensure_parent(out_path)?;
    zip_dir(stage.path(), out_path)?;
    println!(
        "[usagi] wrote {} (target: {target:?}, host fuse, {} game file(s), {} bundle bytes)",
        out_path.display(),
        bundle.file_count(),
        bundle.total_bytes(),
    );
    Ok(())
}

/// Extracts `archive` to a tempdir, then delegates to `export_from_runtime_dir`.
fn export_from_archive(
    bundle: &Bundle,
    project_name: &crate::project_name::ProjectName,
    archive: &Path,
    target: templates::Target,
    opts: &Opts,
    out_path: &Path,
) -> Result<()> {
    if !archive.is_file() {
        return Err(Error::Cli(format!(
            "template archive not found: {}",
            archive.display()
        )));
    }
    let scratch = tempfile::tempdir()
        .map_err(|e| Error::Cli(format!("creating template scratch dir: {e}")))?;
    let extract_dir = scratch.path().join("extracted");
    templates::extract(archive, &extract_dir)?;
    export_from_runtime_dir(bundle, project_name, &extract_dir, target, opts, out_path)
}

/// Fuses a bundle onto the runtime in `runtime_dir` and zips the result.
/// `runtime_dir` is either a tempdir (from `--template-path`/`url`) or
/// the shared cache dir (from auto-fetch). `web_shell_override` only
/// applies to the web target.
fn export_from_runtime_dir(
    bundle: &Bundle,
    project_name: &crate::project_name::ProjectName,
    runtime_dir: &Path,
    target: templates::Target,
    opts: &Opts,
    out_path: &Path,
) -> Result<()> {
    let runtime = templates::locate(runtime_dir, target)?;
    let stage =
        tempfile::tempdir().map_err(|e| Error::Cli(format!("creating zip stage dir: {e}")))?;
    match runtime {
        templates::Runtime::Native { exe } => {
            let staged_exe = staged_binary_path(
                stage.path(),
                project_name,
                target,
                opts.bundle_id,
                opts.icns_bytes,
            )?;
            fuse_exe(bundle, &exe, &staged_exe)?;
        }
        templates::Runtime::Web { js, wasm, html } => {
            let html_src = opts.web_shell_override.unwrap_or(&html);
            stage_file(html_src, &stage.path().join("index.html"))?;
            stage_file(&js, &stage.path().join("usagi.js"))?;
            stage_file(&wasm, &stage.path().join("usagi.wasm"))?;
            bundle
                .write_to_path(&stage.path().join("game.usagi"))
                .map_err(|e| Error::Cli(format!("staging game.usagi: {e}")))?;
        }
    }
    ensure_parent(out_path)?;
    zip_dir(stage.path(), out_path)?;
    println!(
        "[usagi] wrote {} (target: {target:?}, {} game file(s), {} bundle bytes)",
        out_path.display(),
        bundle.file_count(),
        bundle.total_bytes(),
    );
    Ok(())
}

fn fuse_exe(bundle: &Bundle, base_exe: &Path, out_path: &Path) -> Result<()> {
    bundle
        .fuse(base_exe, out_path)
        .map_err(|e| Error::Cli(format!("fusing bundle onto {}: {e}", base_exe.display())))?;
    println!(
        "[usagi] fused {} ({} file(s), {} bytes bundled)",
        out_path.display(),
        bundle.file_count(),
        bundle.total_bytes(),
    );
    Ok(())
}

fn write_bundle(bundle: &Bundle, out_path: &Path) -> Result<()> {
    bundle
        .write_to_path(out_path)
        .map_err(|e| Error::Cli(format!("writing bundle to {}: {e}", out_path.display())))?;
    println!(
        "[usagi] wrote {} ({} file(s), {} bytes)",
        out_path.display(),
        bundle.file_count(),
        bundle.total_bytes(),
    );
    Ok(())
}

fn stage_file(src: &Path, dst: &Path) -> Result<()> {
    std::fs::copy(src, dst).map_err(|e| {
        Error::Cli(format!(
            "staging {}: {e}",
            dst.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("<unknown>")
        ))
    })?;
    Ok(())
}

fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .map_err(|e| Error::Cli(format!("creating output dir {}: {e}", parent.display())))?;
    }
    Ok(())
}

fn target_produces_web(target: ExportTarget) -> bool {
    matches!(target, ExportTarget::All | ExportTarget::Web)
}

/// Maps the CLI export-target enum to the template-module enum. Returns
/// `None` for targets that don't use templates (`all`, `bundle`).
fn template_target_for(target: ExportTarget) -> Option<templates::Target> {
    match target {
        ExportTarget::Linux => Some(templates::Target::Linux),
        ExportTarget::Macos => Some(templates::Target::Macos),
        ExportTarget::Windows => Some(templates::Target::Windows),
        ExportTarget::Web => Some(templates::Target::Wasm),
        _ => None,
    }
}

/// Picks the web export's shell.html source: the explicit `--web-shell`
/// flag wins, then a sibling `shell.html` next to the script, otherwise
/// None (the template's default shell is used).
fn resolve_web_shell_override(script_path: &Path, flag: Option<&str>) -> Result<Option<PathBuf>> {
    if let Some(p) = flag {
        let path = PathBuf::from(p);
        if !path.is_file() {
            return Err(Error::Cli(format!(
                "--web-shell file not found: {}",
                path.display()
            )));
        }
        return Ok(Some(path));
    }
    let auto = script_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("shell.html");
    Ok(auto.is_file().then_some(auto))
}

/// Path-derived name hint: parent directory when the script is
/// `main.lua` (so `examples/spr/main.lua` -> `spr`), file stem
/// otherwise (`examples/snake.lua` -> `snake`). Used as fallback
/// for `ProjectName` and as input to `GameId` resolution.
fn path_name_hint(script_path: &Path) -> &str {
    let stem = script_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("game");
    if stem == "main" {
        script_path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or(stem)
    } else {
        stem
    }
}

fn default_output_path(
    project_name: &crate::project_name::ProjectName,
    target: ExportTarget,
) -> PathBuf {
    let slug = project_name.slug();
    match target {
        // Project-agnostic so one gitignore entry covers any game.
        ExportTarget::All => PathBuf::from("export"),
        ExportTarget::Bundle => PathBuf::from(format!("{slug}.usagi")),
        ExportTarget::Linux => PathBuf::from(format!("{slug}-linux.zip")),
        ExportTarget::Macos => PathBuf::from(format!("{slug}-macos.zip")),
        ExportTarget::Windows => PathBuf::from(format!("{slug}-windows.zip")),
        ExportTarget::Web => PathBuf::from(format!("{slug}-web.zip")),
    }
}

fn staged_exe_name(slug: &str, target: templates::Target) -> String {
    match target {
        templates::Target::Windows => format!("{slug}.exe"),
        _ => slug.to_owned(),
    }
}

/// Where in `stage` the fused binary should land. macOS gets the
/// `<display>.app/Contents/MacOS/<slug>` layout (with Info.plist +
/// PkgInfo + optional `Resources/AppIcon.icns` written as side
/// effects); other native targets stay flat at the stage root.
fn staged_binary_path(
    stage: &Path,
    project_name: &crate::project_name::ProjectName,
    target: templates::Target,
    bundle_id: &str,
    icns_bytes: Option<&[u8]>,
) -> Result<PathBuf> {
    match target {
        templates::Target::Macos => macos_app::stage_app_layout(
            stage,
            project_name.display(),
            project_name.slug(),
            bundle_id,
            icns_bytes,
        ),
        _ => Ok(stage.join(staged_exe_name(project_name.slug(), target))),
    }
}

/// Picks a local filename for a downloaded template, preserving the URL's
/// extension so `templates::extract` can dispatch by suffix. Falls back
/// to a generic name when the URL has no usable basename.
fn archive_name_from_url(url: &str) -> String {
    let trimmed = url.split(['?', '#']).next().unwrap_or(url);
    let last = trimmed.rsplit('/').next().unwrap_or("");
    if last.ends_with(".tar.gz") || last.ends_with(".tgz") || last.ends_with(".zip") {
        last.to_owned()
    } else {
        "template.tar.gz".to_owned()
    }
}

/// Zips every file under `src_dir` into `out_zip`. Preserves the unix
/// executable bit so a fused binary stays runnable after unzip.
fn zip_dir(src_dir: &Path, out_zip: &Path) -> Result<()> {
    let f = std::fs::File::create(out_zip)
        .map_err(|e| Error::Cli(format!("creating {}: {e}", out_zip.display())))?;
    let mut w = zip::ZipWriter::new(f);
    walk_into_zip(src_dir, src_dir, &mut w)?;
    w.finish()
        .map_err(|e| Error::Cli(format!("finalizing {}: {e}", out_zip.display())))?;
    Ok(())
}

fn walk_into_zip(root: &Path, dir: &Path, w: &mut zip::ZipWriter<std::fs::File>) -> Result<()> {
    for entry in std::fs::read_dir(dir)
        .map_err(|e| Error::Cli(format!("read_dir {}: {e}", dir.display())))?
    {
        let entry = entry.map_err(|e| Error::Cli(format!("read_dir entry: {e}")))?;
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .map_err(|e| Error::Cli(format!("strip_prefix: {e}")))?
            .to_string_lossy()
            .replace('\\', "/");
        if path.is_dir() {
            walk_into_zip(root, &path, w)?;
        } else {
            let mode = exec_mode_of(&path);
            let mut opts: zip::write::SimpleFileOptions =
                zip::write::SimpleFileOptions::default().unix_permissions(mode);
            if let Some(dt) = entry_modified_time(&path) {
                opts = opts.last_modified_time(dt);
            }
            w.start_file(&rel, opts)
                .map_err(|e| Error::Cli(format!("zip start_file {rel}: {e}")))?;
            let mut f = std::fs::File::open(&path)
                .map_err(|e| Error::Cli(format!("open {}: {e}", path.display())))?;
            std::io::copy(&mut f, w).map_err(|e| Error::Cli(format!("zip copy {rel}: {e}")))?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn exec_mode_of(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o7777)
        .unwrap_or(0o644)
}

#[cfg(not(unix))]
fn exec_mode_of(_path: &Path) -> u32 {
    0o644
}

/// Source mtime as a zip-format timestamp. Without this, zip entries
/// default to the DOS epoch (1980-01-01) and unzip shows a 40+-year-old
/// timestamp. Best-effort: any failure falls through to that default.
fn entry_modified_time(path: &Path) -> Option<zip::DateTime> {
    let mtime = std::fs::metadata(path).ok()?.modified().ok()?;
    let odt = time::OffsetDateTime::from(mtime);
    let pdt = time::PrimitiveDateTime::new(odt.date(), odt.time());
    zip::DateTime::try_from(pdt).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn web_shell_override_uses_explicit_flag_when_given() {
        let dir = tempdir().unwrap();
        let custom = dir.path().join("custom.html");
        std::fs::write(&custom, b"<!doctype html>").unwrap();
        let script = dir.path().join("main.lua");
        std::fs::write(&script, b"-- game").unwrap();
        let resolved = resolve_web_shell_override(&script, Some(custom.to_str().unwrap())).unwrap();
        assert_eq!(resolved.as_deref(), Some(custom.as_path()));
    }

    #[test]
    fn web_shell_override_errors_when_explicit_flag_points_at_missing_file() {
        let dir = tempdir().unwrap();
        let script = dir.path().join("main.lua");
        std::fs::write(&script, b"-- game").unwrap();
        let err =
            resolve_web_shell_override(&script, Some("/nope/does-not-exist.html")).unwrap_err();
        match err {
            Error::Cli(msg) => assert!(msg.contains("--web-shell"), "got: {msg}"),
            _ => panic!("expected Cli error"),
        }
    }

    #[test]
    fn web_shell_override_auto_picks_up_sibling_shell_html() {
        let dir = tempdir().unwrap();
        let auto = dir.path().join("shell.html");
        std::fs::write(&auto, b"<!doctype html>").unwrap();
        let script = dir.path().join("main.lua");
        std::fs::write(&script, b"-- game").unwrap();
        let resolved = resolve_web_shell_override(&script, None).unwrap();
        assert_eq!(resolved.as_deref(), Some(auto.as_path()));
    }

    #[test]
    fn web_shell_override_returns_none_when_no_flag_and_no_sibling() {
        let dir = tempdir().unwrap();
        let script = dir.path().join("main.lua");
        std::fs::write(&script, b"-- game").unwrap();
        let resolved = resolve_web_shell_override(&script, None).unwrap();
        assert!(resolved.is_none());
    }

    #[test]
    fn target_produces_web_table() {
        assert!(target_produces_web(ExportTarget::All));
        assert!(target_produces_web(ExportTarget::Web));
        assert!(!target_produces_web(ExportTarget::Bundle));
        assert!(!target_produces_web(ExportTarget::Linux));
        assert!(!target_produces_web(ExportTarget::Macos));
        assert!(!target_produces_web(ExportTarget::Windows));
    }

    #[test]
    fn path_name_hint_uses_parent_for_main_lua() {
        let p = Path::new("examples/snake/main.lua");
        assert_eq!(path_name_hint(p), "snake");
    }

    #[test]
    fn path_name_hint_uses_stem_for_flat_script() {
        let p = Path::new("examples/hello.lua");
        assert_eq!(path_name_hint(p), "hello");
    }

    #[test]
    fn archive_name_from_url_preserves_known_extensions() {
        assert_eq!(
            archive_name_from_url("https://x.test/v1/usagi-1.0-linux-x86_64.tar.gz"),
            "usagi-1.0-linux-x86_64.tar.gz"
        );
        assert_eq!(
            archive_name_from_url("https://x.test/v1/usagi-1.0-windows-x86_64.zip"),
            "usagi-1.0-windows-x86_64.zip"
        );
    }

    #[test]
    fn archive_name_from_url_falls_back_when_unrecognized() {
        assert_eq!(
            archive_name_from_url("https://x.test/blob"),
            "template.tar.gz"
        );
    }
}
