//! Offline generic shader compiler check for the native CLI.

use super::ShaderProfile;
use crate::{Error, Result};
use clap::ValueEnum;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum ShaderCheckTarget {
    /// Compile the desktop runtime target.
    Desktop,
    /// Compile the web runtime target.
    Web,
    /// Compile every supported generic shader profile for conformance.
    All,
}

impl ShaderCheckTarget {
    fn profiles(self) -> Vec<ShaderProfile> {
        match self {
            Self::Desktop => vec![ShaderProfile::DesktopGlsl330],
            Self::Web => vec![ShaderProfile::WebGlslEs100],
            Self::All => ShaderProfile::ALL.to_vec(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ShaderCheckReport {
    shader_count: usize,
    compile_count: usize,
    failures: Vec<ShaderCheckFailure>,
}

#[derive(Debug, PartialEq, Eq)]
struct ShaderCheckFailure {
    path: PathBuf,
    profile: Option<ShaderProfile>,
    diagnostic: String,
}

pub(crate) fn run(path_arg: &str, target: ShaderCheckTarget) -> Result<()> {
    let script_path = crate::cli::resolve_script_path(path_arg)?;
    let project_root = Path::new(&script_path)
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let profiles = target.profiles();
    let report = check_project(project_root, &profiles)?;

    if !report.failures.is_empty() {
        return Err(Error::Cli(format_failure_report(project_root, &report)));
    }

    if report.shader_count == 0 {
        crate::msg::info!(
            "shader check passed: no generic shaders found in {}",
            project_root.join("shaders").display()
        );
    } else {
        crate::msg::info!(
            "shader check passed: {} generic shader(s), {} target compile(s)",
            report.shader_count,
            report.compile_count
        );
    }
    Ok(())
}

fn check_project(project_root: &Path, profiles: &[ShaderProfile]) -> Result<ShaderCheckReport> {
    let shaders = collect_generic_shader_files(project_root)?;
    let mut failures = Vec::new();
    let mut compile_count = 0usize;

    for path in &shaders {
        let src = match read_shader_source(path) {
            Ok(src) => src,
            Err(diagnostic) => {
                failures.push(ShaderCheckFailure {
                    path: path.clone(),
                    profile: None,
                    diagnostic,
                });
                continue;
            }
        };

        for profile in profiles {
            compile_count += 1;
            if let Err(diagnostic) = super::compile_generic_fragment_with_metadata(&src, *profile) {
                failures.push(ShaderCheckFailure {
                    path: path.clone(),
                    profile: Some(*profile),
                    diagnostic,
                });
            }
        }
    }

    Ok(ShaderCheckReport {
        shader_count: shaders.len(),
        compile_count,
        failures,
    })
}

fn collect_generic_shader_files(project_root: &Path) -> Result<Vec<PathBuf>> {
    let shaders_dir = project_root.join("shaders");
    if !shaders_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut shaders = Vec::new();
    for entry in fs::read_dir(&shaders_dir)
        .map_err(|e| Error::Cli(format!("read_dir {}: {e}", shaders_dir.display())))?
    {
        let entry = entry.map_err(|e| Error::Cli(format!("read_dir entry: {e}")))?;
        let path = entry.path();
        if !entry
            .file_type()
            .map_err(|e| Error::Cli(format!("metadata {}: {e}", path.display())))?
            .is_file()
        {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.ends_with(".usagi.fs") {
            shaders.push(path);
        }
    }
    shaders.sort();
    Ok(shaders)
}

fn read_shader_source(path: &Path) -> std::result::Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("[source] reading file: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("[source] file is not valid utf-8: {e}"))
}

fn format_failure_report(project_root: &Path, report: &ShaderCheckReport) -> String {
    let mut out = format!(
        "shader check failed: {} error(s) across {} generic shader(s)",
        report.failures.len(),
        report.shader_count
    );

    for failure in &report.failures {
        out.push_str("\n\n");
        out.push_str(&display_shader_path(project_root, &failure.path));
        if let Some(profile) = failure.profile {
            out.push_str(" [");
            out.push_str(profile.label());
            out.push(']');
        }
        out.push('\n');
        out.push_str(failure.diagnostic.trim_end());
    }
    out
}

fn display_shader_path(project_root: &Path, shader_path: &Path) -> String {
    shader_path
        .strip_prefix(project_root)
        .unwrap_or(shader_path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const VALID_SHADER: &str = concat!(
        "#usagi shader 1\n\n",
        "uniform float u_time;\n",
        "vec4 usagi_main(vec2 uv, vec4 color) {\n",
        "    return usagi_texture(texture0, uv) * color * u_time;\n",
        "}\n",
    );

    fn project() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("main.lua"), "-- test").unwrap();
        fs::create_dir(dir.path().join("shaders")).unwrap();
        dir
    }

    #[test]
    fn check_targets_map_to_runtime_profiles() {
        assert_eq!(
            ShaderCheckTarget::Desktop.profiles(),
            vec![ShaderProfile::DesktopGlsl330]
        );
        assert_eq!(
            ShaderCheckTarget::Web.profiles(),
            vec![ShaderProfile::WebGlslEs100]
        );
        assert_eq!(
            ShaderCheckTarget::All.profiles(),
            vec![
                ShaderProfile::WebGlslEs100,
                ShaderProfile::DesktopGlsl330,
                ShaderProfile::DesktopGlsl440,
            ]
        );
    }

    #[test]
    fn collects_only_direct_generic_fragment_shaders_in_stable_order() {
        let dir = project();
        fs::write(dir.path().join("shaders/zeta.usagi.fs"), VALID_SHADER).unwrap();
        fs::write(dir.path().join("shaders/alpha.usagi.fs"), VALID_SHADER).unwrap();
        fs::write(dir.path().join("shaders/native.fs"), "#version 330\n").unwrap();
        fs::create_dir(dir.path().join("shaders/nested")).unwrap();
        fs::write(
            dir.path().join("shaders/nested/skipped.usagi.fs"),
            VALID_SHADER,
        )
        .unwrap();

        let files = collect_generic_shader_files(dir.path()).unwrap();
        let names: Vec<_> = files
            .iter()
            .map(|path| path.file_name().unwrap().to_str().unwrap())
            .collect();

        assert_eq!(names, vec!["alpha.usagi.fs", "zeta.usagi.fs"]);
    }

    #[test]
    fn valid_project_checks_selected_runtime_profile() {
        let dir = project();
        fs::write(dir.path().join("shaders/crt.usagi.fs"), VALID_SHADER).unwrap();

        let profiles = ShaderCheckTarget::Desktop.profiles();
        let report = check_project(dir.path(), &profiles).unwrap();

        assert_eq!(report.shader_count, 1);
        assert_eq!(report.compile_count, 1);
        assert!(report.failures.is_empty());
    }

    #[test]
    fn invalid_conformance_check_reports_each_failed_profile_without_stopping() {
        let dir = project();
        fs::write(
            dir.path().join("shaders/bad.usagi.fs"),
            "vec4 usagi_main(vec2 uv, vec4 color) { return texture(texture0, uv); }\n",
        )
        .unwrap();

        let profiles = ShaderCheckTarget::All.profiles();
        let report = check_project(dir.path(), &profiles).unwrap();
        let message = format_failure_report(dir.path(), &report);

        assert_eq!(report.shader_count, 1);
        assert_eq!(report.compile_count, 3);
        assert_eq!(report.failures.len(), 3);
        assert!(message.contains("shaders"));
        assert!(message.contains("bad.usagi.fs [GLSL ES 100]"));
        assert!(message.contains("bad.usagi.fs [GLSL 330]"));
        assert!(message.contains("bad.usagi.fs [GLSL 440]"));
        assert!(message.contains("generic shaders must use usagi_texture"));
    }
}
