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
                crate::msg::warn!("icon: {e}; macOS bundle will ship without an icon");
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
        resolution: project_config.resolution,
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
    /// Game render dimensions read from `_config()`. The web export
    /// templates the canvas backing-store from this so non-default
    /// resolutions ship with the right aspect ratio. Other targets
    /// don't read it (the runtime resolves from `_config()` at boot).
    resolution: crate::config::Resolution,
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
        resolution: opts.resolution,
    };
    let slug = project_name.slug();
    let mut succeeded = 0;
    let mut last_err: Option<Error> = None;
    for target in templates::Target::ALL {
        let zip = out_dir.join(format!("{slug}-{}.zip", target.as_str()));
        match export_one_target(bundle, project_name, target, &inner, &zip) {
            Ok(()) => succeeded += 1,
            Err(e) => {
                crate::msg::warn!("skipping {target:?}: {e}");
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
    crate::msg::info!("export ready at {}/", out_dir.display());
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
        crate::msg::info!("downloading {url}");
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
    fuse_exe(bundle, &current_exe, &staged_exe, target)?;
    ensure_parent(out_path)?;
    zip_dir(stage.path(), out_path)?;
    crate::msg::info!(
        "wrote {} (target: {target:?}, host fuse, {} game file(s), {} bundle bytes)",
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
            fuse_exe(bundle, &exe, &staged_exe, target)?;
        }
        templates::Runtime::Web { js, wasm, html } => {
            let html_src = opts.web_shell_override.unwrap_or(&html);
            stage_web_shell(html_src, &stage.path().join("index.html"), opts.resolution)?;
            stage_file(&js, &stage.path().join("usagi.js"))?;
            stage_file(&wasm, &stage.path().join("usagi.wasm"))?;
            bundle
                .write_to_path(&stage.path().join("game.usagi"))
                .map_err(|e| Error::Cli(format!("staging game.usagi: {e}")))?;
        }
    }
    ensure_parent(out_path)?;
    zip_dir(stage.path(), out_path)?;
    crate::msg::info!(
        "wrote {} (target: {target:?}, {} game file(s), {} bundle bytes)",
        out_path.display(),
        bundle.file_count(),
        bundle.total_bytes(),
    );
    Ok(())
}

fn fuse_exe(
    bundle: &Bundle,
    base_exe: &Path,
    out_path: &Path,
    target: templates::Target,
) -> Result<()> {
    bundle
        .fuse(base_exe, out_path)
        .map_err(|e| Error::Cli(format!("fusing bundle onto {}: {e}", base_exe.display())))?;
    if target == templates::Target::Windows {
        // The shipped CLI runtime is console-subsystem so `usagi run`
        // / `usagi dev` and friends print to the terminal. Exported
        // games shouldn't pop up a black console window, so flip the
        // PE subsystem byte from CONSOLE (3) to WINDOWS_GUI (2)
        // here, after the bundle has been fused.
        patch_windows_subsystem_to_gui(out_path)?;
    }
    crate::msg::info!(
        "fused {} ({} file(s), {} bytes bundled)",
        out_path.display(),
        bundle.file_count(),
        bundle.total_bytes(),
    );
    Ok(())
}

/// Rewrites the PE optional header's `Subsystem` field from
/// `IMAGE_SUBSYSTEM_WINDOWS_CUI` (3, console) to
/// `IMAGE_SUBSYSTEM_WINDOWS_GUI` (2, windowed). Used during the
/// Windows export fuse step so end users running an exported game
/// don't get a console pop-up, while the engine's CLI binary stays
/// console-subsystem at the source level (so `usagi run` etc. print
/// to the terminal on Windows).
///
/// PE layout cribbed from the Microsoft "PE Format" spec:
/// `e_lfanew` at DOS offset 0x3C points at the PE signature; after
/// the 4-byte signature and the 20-byte COFF File Header comes the
/// Optional Header, whose Subsystem field sits at offset 0x44 for
/// both PE32 and PE32+ images.
fn patch_windows_subsystem_to_gui(path: &Path) -> Result<()> {
    let mut bytes = std::fs::read(path)
        .map_err(|e| Error::Cli(format!("reading {} for PE patch: {e}", path.display())))?;
    if bytes.len() < 0x40 || &bytes[0..2] != b"MZ" {
        return Err(Error::Cli(format!(
            "{} is not a PE file (missing MZ header)",
            path.display()
        )));
    }
    let pe_offset = u32::from_le_bytes(
        bytes[0x3C..0x40]
            .try_into()
            .expect("4-byte slice at 0x3C is always convertible"),
    ) as usize;
    if bytes.len() < pe_offset + 4 || &bytes[pe_offset..pe_offset + 4] != b"PE\0\0" {
        return Err(Error::Cli(format!(
            "{} missing PE signature at offset {pe_offset:#x}",
            path.display()
        )));
    }
    let subsystem_offset = pe_offset + 4 + 20 + 0x44;
    if bytes.len() < subsystem_offset + 2 {
        return Err(Error::Cli(format!(
            "{} truncated before PE Optional Header subsystem field",
            path.display()
        )));
    }
    bytes[subsystem_offset] = 0x02;
    bytes[subsystem_offset + 1] = 0x00;
    std::fs::write(path, &bytes)
        .map_err(|e| Error::Cli(format!("writing {} after PE patch: {e}", path.display())))?;
    Ok(())
}

fn write_bundle(bundle: &Bundle, out_path: &Path) -> Result<()> {
    bundle
        .write_to_path(out_path)
        .map_err(|e| Error::Cli(format!("writing bundle to {}: {e}", out_path.display())))?;
    crate::msg::info!(
        "wrote {} ({} file(s), {} bytes)",
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

/// Reads the web shell HTML, substitutes the canvas backing-store
/// dims to match `_config().game_width / game_height`, and writes the
/// result to `dst`. The aspect-ratio CSS in the bundled shell reads
/// the canvas attrs at load time, so a single replacement of
/// `width="640" height="360"` (the engine default's 2x backing store)
/// is enough to specialize layout for any resolution. Custom shells
/// that don't include the default attrs pass through unchanged;
/// users running their own HTML are expected to manage their own
/// dims.
fn stage_web_shell(src: &Path, dst: &Path, res: crate::config::Resolution) -> Result<()> {
    let html = std::fs::read_to_string(src)
        .map_err(|e| Error::Cli(format!("reading {}: {e}", src.display())))?;
    let (cw, ch) = web_canvas_dims(res);
    let rendered = html.replace(
        r#"width="640" height="360""#,
        &format!(r#"width="{cw}" height="{ch}""#),
    );
    std::fs::write(dst, rendered)
        .map_err(|e| Error::Cli(format!("writing {}: {e}", dst.display())))?;
    Ok(())
}

/// Canvas backing-store dimensions for a given game resolution. Mirrors
/// the native window-scale rule: 2x for default-and-smaller games (so
/// a 320x180 game ships a 640x360 canvas, the size we've been
/// shipping), 1x for anything bigger so the shell doesn't push past
/// reasonable embed sizes.
fn web_canvas_dims(res: crate::config::Resolution) -> (i32, i32) {
    let scale = if res.w.max(res.h) > crate::config::Resolution::DEFAULT.w * 2.0 {
        1.0
    } else {
        2.0
    };
    ((res.w * scale) as i32, (res.h * scale) as i32)
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

    /// Builds a minimal byte buffer that's just-enough-PE for
    /// `patch_windows_subsystem_to_gui` to traverse: DOS header with
    /// `e_lfanew` pointing at the PE signature, COFF File Header,
    /// and an Optional Header padded out past the subsystem field.
    fn build_minimal_pe_with_subsystem(value: u16) -> (Vec<u8>, usize) {
        let pe_offset: usize = 0x80;
        let mut bytes = vec![0u8; pe_offset + 4 + 20 + 0x46];
        bytes[0] = b'M';
        bytes[1] = b'Z';
        bytes[0x3C..0x40].copy_from_slice(&(pe_offset as u32).to_le_bytes());
        bytes[pe_offset..pe_offset + 4].copy_from_slice(b"PE\0\0");
        let subsystem_offset = pe_offset + 4 + 20 + 0x44;
        bytes[subsystem_offset..subsystem_offset + 2].copy_from_slice(&value.to_le_bytes());
        (bytes, subsystem_offset)
    }

    #[test]
    fn patch_windows_subsystem_to_gui_flips_subsystem_byte() {
        let dir = tempdir().unwrap();
        let exe = dir.path().join("fake.exe");
        let (bytes, subsystem_offset) = build_minimal_pe_with_subsystem(3);
        std::fs::write(&exe, &bytes).unwrap();
        patch_windows_subsystem_to_gui(&exe).unwrap();
        let patched = std::fs::read(&exe).unwrap();
        assert_eq!(patched[subsystem_offset], 0x02);
        assert_eq!(patched[subsystem_offset + 1], 0x00);
        // No other bytes touched.
        for (i, (a, b)) in bytes.iter().zip(patched.iter()).enumerate() {
            if i == subsystem_offset || i == subsystem_offset + 1 {
                continue;
            }
            assert_eq!(a, b, "byte {i} unexpectedly changed");
        }
    }

    #[test]
    fn patch_windows_subsystem_rejects_non_pe_files() {
        let dir = tempdir().unwrap();
        let f = dir.path().join("not_a_pe");
        std::fs::write(&f, b"this is just text, not a PE binary").unwrap();
        let err = patch_windows_subsystem_to_gui(&f).unwrap_err();
        assert!(format!("{err}").contains("MZ"), "got: {err}");
    }

    #[test]
    fn patch_windows_subsystem_rejects_truncated_pe() {
        let dir = tempdir().unwrap();
        let f = dir.path().join("truncated.exe");
        // Has MZ + e_lfanew but the file ends before the PE signature.
        let mut bytes = vec![0u8; 0x40];
        bytes[0] = b'M';
        bytes[1] = b'Z';
        bytes[0x3C..0x40].copy_from_slice(&0x100u32.to_le_bytes());
        std::fs::write(&f, &bytes).unwrap();
        let err = patch_windows_subsystem_to_gui(&f).unwrap_err();
        assert!(format!("{err}").contains("PE signature"), "got: {err}");
    }

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
    fn web_canvas_dims_doubles_default_size() {
        let (w, h) = web_canvas_dims(crate::config::Resolution::DEFAULT);
        assert_eq!((w, h), (640, 360));
    }

    #[test]
    fn web_canvas_dims_doubles_at_or_below_threshold() {
        let r = crate::config::Resolution { w: 640.0, h: 360.0 };
        assert_eq!(web_canvas_dims(r), (1280, 720));
        let r = crate::config::Resolution { w: 180.0, h: 320.0 };
        assert_eq!(web_canvas_dims(r), (360, 640));
        let r = crate::config::Resolution { w: 160.0, h: 90.0 };
        assert_eq!(web_canvas_dims(r), (320, 180));
    }

    #[test]
    fn web_canvas_dims_passes_through_above_threshold() {
        let r = crate::config::Resolution { w: 800.0, h: 450.0 };
        assert_eq!(web_canvas_dims(r), (800, 450));
        let r = crate::config::Resolution {
            w: 1280.0,
            h: 720.0,
        };
        assert_eq!(web_canvas_dims(r), (1280, 720));
    }

    #[test]
    fn stage_web_shell_substitutes_canvas_dims() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("shell.html");
        let dst = dir.path().join("index.html");
        std::fs::write(
            &src,
            r#"<canvas id="canvas" width="640" height="360"></canvas>"#,
        )
        .unwrap();
        let r = crate::config::Resolution { w: 480.0, h: 270.0 };
        stage_web_shell(&src, &dst, r).unwrap();
        let written = std::fs::read_to_string(&dst).unwrap();
        assert!(
            written.contains(r#"width="960" height="540""#),
            "got: {written}"
        );
        assert!(
            !written.contains(r#"width="640" height="360""#),
            "old dims should be gone: {written}"
        );
    }

    #[test]
    fn stage_web_shell_passes_through_custom_shell_without_default_dims() {
        // A user shell that doesn't include the literal `width="640"
        // height="360"` substring should be written verbatim. Users
        // running their own HTML are responsible for their canvas dims.
        let dir = tempdir().unwrap();
        let src = dir.path().join("shell.html");
        let dst = dir.path().join("index.html");
        let original = r#"<canvas id="canvas" width="800" height="450"></canvas>"#;
        std::fs::write(&src, original).unwrap();
        let r = crate::config::Resolution::DEFAULT;
        stage_web_shell(&src, &dst, r).unwrap();
        assert_eq!(std::fs::read_to_string(&dst).unwrap(), original);
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
