//! Offline generic shader compiler check for the native CLI.

use super::ShaderProfile;
use crate::{Error, Result};
use clap::ValueEnum;
use serde_json::json;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum ShaderCheckFormat {
    /// Human-readable terminal output.
    Text,
    /// Structured JSON for editor integrations and tooling.
    Json,
}

#[derive(Debug, PartialEq, Eq)]
struct ShaderCheckReport {
    shader_count: usize,
    compile_count: usize,
    failures: Vec<ShaderCheckFailure>,
    warnings: Vec<ShaderCheckWarning>,
}

#[derive(Debug, PartialEq, Eq)]
struct ShaderCheckFailure {
    path: PathBuf,
    profile: Option<ShaderProfile>,
    diagnostic: ShaderCheckDiagnostic,
}

#[derive(Debug, PartialEq, Eq)]
struct ShaderCheckWarning {
    path: PathBuf,
    profile: ShaderProfile,
    diagnostic: ShaderCheckDiagnostic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShaderCheckDiagnosticKind {
    Source,
    Compiler,
    CompilerWarning,
}

impl ShaderCheckDiagnosticKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Compiler => "compiler",
            Self::CompilerWarning => "compiler-warning",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ShaderCheckDiagnostic {
    kind: ShaderCheckDiagnosticKind,
    message: String,
    line: Option<usize>,
    column: Option<usize>,
    byte_start: Option<usize>,
    byte_end: Option<usize>,
    source_line: Option<String>,
    marker_len: Option<usize>,
}

impl ShaderCheckDiagnostic {
    fn source(message: impl Into<String>) -> Self {
        Self {
            kind: ShaderCheckDiagnosticKind::Source,
            message: message.into(),
            line: None,
            column: None,
            byte_start: None,
            byte_end: None,
            source_line: None,
            marker_len: None,
        }
    }

    fn compiler(diagnostic: &super::compiler::ShaderDiagnostic) -> Self {
        Self {
            kind: ShaderCheckDiagnosticKind::Compiler,
            message: diagnostic.message.clone(),
            line: diagnostic.line,
            column: diagnostic.column,
            byte_start: diagnostic.byte_start,
            byte_end: diagnostic.byte_end,
            source_line: diagnostic.source_line.clone(),
            marker_len: diagnostic.marker_len,
        }
    }

    fn compiler_warning(diagnostic: &super::compiler::ShaderDiagnostic) -> Self {
        Self {
            kind: ShaderCheckDiagnosticKind::CompilerWarning,
            message: diagnostic.message.clone(),
            line: diagnostic.line,
            column: diagnostic.column,
            byte_start: diagnostic.byte_start,
            byte_end: diagnostic.byte_end,
            source_line: diagnostic.source_line.clone(),
            marker_len: diagnostic.marker_len,
        }
    }

    fn render_text(&self) -> String {
        let (Some(line), Some(column), Some(source_line), Some(marker_len)) = (
            self.line,
            self.column,
            self.source_line.as_deref(),
            self.marker_len,
        ) else {
            return format!("[{}] {}", self.kind.as_str(), self.message);
        };

        format!(
            "[{}] {} at line {}, column {}\n{}\n{}{}",
            self.kind.as_str(),
            self.message,
            line,
            column,
            source_line,
            " ".repeat(column.saturating_sub(1)),
            "^".repeat(marker_len)
        )
    }

    fn as_tool_diagnostic(&self) -> super::tool_json::ToolDiagnostic<'_> {
        super::tool_json::ToolDiagnostic {
            kind: Some(self.kind.as_str()),
            message: &self.message,
            line: self.line,
            column: self.column,
            byte_start: self.byte_start,
            byte_end: self.byte_end,
            source_line: self.source_line.as_deref(),
            marker_len: self.marker_len,
        }
    }
}

pub(crate) fn run(
    path_arg: &str,
    target: ShaderCheckTarget,
    format: ShaderCheckFormat,
) -> Result<()> {
    let script_path = crate::cli::resolve_script_path(path_arg)?;
    let project_root = Path::new(&script_path)
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let profiles = target.profiles();
    let report = check_project(project_root, &profiles)?;

    if format == ShaderCheckFormat::Json {
        println!("{}", format_json_report(project_root, &profiles, &report)?);
        if !report.failures.is_empty() {
            return Err(Error::Cli(format!(
                "shader check failed: {} error(s) across {} generic shader(s)",
                report.failures.len(),
                report.shader_count
            )));
        }
        return Ok(());
    }

    if !report.failures.is_empty() {
        return Err(Error::Cli(format_failure_report(project_root, &report)));
    }

    if !report.warnings.is_empty() {
        crate::msg::warn!("{}", format_warning_report(project_root, &report));
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
    let mut warnings = Vec::new();
    let mut compile_count = 0usize;

    for path in &shaders {
        let src = match read_shader_source(path) {
            Ok(src) => src,
            Err(diagnostic) => {
                failures.push(ShaderCheckFailure {
                    path: path.clone(),
                    profile: None,
                    diagnostic: ShaderCheckDiagnostic::source(diagnostic),
                });
                continue;
            }
        };

        for profile in profiles {
            compile_count += 1;
            match super::compile_generic_fragment_with_report(&src, *profile) {
                Ok(compiled) => {
                    warnings.extend(compiled.metadata.warnings.iter().map(|warning| {
                        ShaderCheckWarning {
                            path: path.clone(),
                            profile: *profile,
                            diagnostic: ShaderCheckDiagnostic::compiler_warning(warning),
                        }
                    }));
                }
                Err(failure) => {
                    failures.push(ShaderCheckFailure {
                        path: path.clone(),
                        profile: Some(*profile),
                        diagnostic: ShaderCheckDiagnostic::compiler(failure.diagnostic.as_ref()),
                    });
                }
            }
        }
    }

    Ok(ShaderCheckReport {
        shader_count: shaders.len(),
        compile_count,
        failures,
        warnings,
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
    let bytes = fs::read(path).map_err(|e| format!("reading file: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("file is not valid utf-8: {e}"))
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
        out.push_str(failure.diagnostic.render_text().trim_end());
    }
    if !report.warnings.is_empty() {
        out.push_str("\n\nWarnings:\n");
        out.push_str(format_warning_lines(project_root, report).trim_end());
    }
    out
}

fn format_warning_report(project_root: &Path, report: &ShaderCheckReport) -> String {
    format!(
        "shader check warning: {} warning(s) across {} generic shader(s)\n{}",
        report.warnings.len(),
        report.shader_count,
        format_warning_lines(project_root, report).trim_end()
    )
}

fn format_warning_lines(project_root: &Path, report: &ShaderCheckReport) -> String {
    let mut out = String::new();
    for warning in &report.warnings {
        out.push_str(&display_shader_path(project_root, &warning.path));
        out.push_str(" [");
        out.push_str(warning.profile.label());
        out.push_str("]\n");
        out.push_str(warning.diagnostic.render_text().trim_end());
        out.push_str("\n\n");
    }
    out
}

fn format_json_report(
    project_root: &Path,
    profiles: &[ShaderProfile],
    report: &ShaderCheckReport,
) -> Result<String> {
    let failures: Vec<_> = report
        .failures
        .iter()
        .map(|failure| {
            let mut fields =
                super::tool_json::diagnostic_fields(failure.diagnostic.as_tool_diagnostic());
            fields.insert(
                "path".to_string(),
                json!(json_shader_path(project_root, &failure.path)),
            );
            fields.insert(
                "profile".to_string(),
                json!(failure.profile.map(|profile| profile.label())),
            );
            serde_json::Value::Object(fields)
        })
        .collect();
    let warnings: Vec<_> = report
        .warnings
        .iter()
        .map(|warning| {
            let mut fields =
                super::tool_json::diagnostic_fields(warning.diagnostic.as_tool_diagnostic());
            fields.insert(
                "path".to_string(),
                json!(json_shader_path(project_root, &warning.path)),
            );
            fields.insert("profile".to_string(), json!(warning.profile.label()));
            serde_json::Value::Object(fields)
        })
        .collect();

    serde_json::to_string_pretty(&json!({
        "ok": report.failures.is_empty(),
        "target_profiles": profiles
            .iter()
            .map(|profile| profile.label())
            .collect::<Vec<_>>(),
        "shader_count": report.shader_count,
        "compile_count": report.compile_count,
        "warning_count": report.warnings.len(),
        "warnings": warnings,
        "failure_count": report.failures.len(),
        "failures": failures,
    }))
    .map_err(|e| Error::Cli(format!("serializing shader check JSON: {e}")))
}

fn display_shader_path(project_root: &Path, shader_path: &Path) -> String {
    shader_path
        .strip_prefix(project_root)
        .unwrap_or(shader_path)
        .display()
        .to_string()
}

fn json_shader_path(project_root: &Path, shader_path: &Path) -> String {
    shader_path
        .strip_prefix(project_root)
        .unwrap_or(shader_path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
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
        assert!(report.warnings.is_empty());
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

    #[test]
    fn json_report_exposes_structured_compiler_diagnostics() {
        let dir = project();
        fs::write(
            dir.path().join("shaders/bad.usagi.fs"),
            "vec4 usagi_main(vec2 uv, vec4 color) { return texture(texture0, uv); }\n",
        )
        .unwrap();

        let profiles = ShaderCheckTarget::Desktop.profiles();
        let report = check_project(dir.path(), &profiles).unwrap();
        let json = format_json_report(dir.path(), &profiles, &report).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["ok"], false);
        assert_eq!(value["shader_count"], 1);
        assert_eq!(value["compile_count"], 1);
        assert_eq!(value["failure_count"], 1);
        assert_eq!(value["warning_count"], 0);
        assert_eq!(value["target_profiles"][0], "GLSL 330");
        assert_eq!(value["failures"][0]["path"], "shaders/bad.usagi.fs");
        assert_eq!(value["failures"][0]["profile"], "GLSL 330");
        assert_eq!(value["failures"][0]["kind"], "compiler");
        assert!(
            value["failures"][0]["message"]
                .as_str()
                .unwrap()
                .contains("usagi_texture")
        );
        assert_eq!(value["failures"][0]["line"], 1);
        assert_eq!(value["failures"][0]["column"], 47);
        assert!(
            value["failures"][0]["source_line"]
                .as_str()
                .unwrap()
                .contains("return texture")
        );
        assert_eq!(value["failures"][0]["marker_len"], 7);
    }

    #[test]
    fn json_report_exposes_unsupported_structured_statement_diagnostics() {
        let dir = project();
        fs::write(
            dir.path().join("shaders/loop.usagi.fs"),
            concat!(
                "vec4 usagi_main(vec2 uv, vec4 color) {\n",
                "    for (int i = 0; i < 2; i = i + 1) color *= 0.5;\n",
                "    return color;\n",
                "}\n",
            ),
        )
        .unwrap();

        let profiles = ShaderCheckTarget::Desktop.profiles();
        let report = check_project(dir.path(), &profiles).unwrap();
        let text = format_failure_report(dir.path(), &report);
        let json = format_json_report(dir.path(), &profiles, &report).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(report.shader_count, 1);
        assert_eq!(report.compile_count, 1);
        assert_eq!(report.failures.len(), 1);
        assert!(text.contains("loop.usagi.fs [GLSL 330]"));
        assert!(text.contains("generic shaders do not support 'for' statements"));
        assert_eq!(value["ok"], false);
        assert_eq!(value["failure_count"], 1);
        assert_eq!(value["failures"][0]["path"], "shaders/loop.usagi.fs");
        assert_eq!(value["failures"][0]["profile"], "GLSL 330");
        assert_eq!(value["failures"][0]["kind"], "compiler");
        assert!(
            value["failures"][0]["message"]
                .as_str()
                .unwrap()
                .contains("'for' statements")
        );
        assert_eq!(value["failures"][0]["line"], 2);
        assert_eq!(value["failures"][0]["column"], 5);
        assert!(
            value["failures"][0]["source_line"]
                .as_str()
                .unwrap()
                .contains("for (int i")
        );
        assert_eq!(value["failures"][0]["marker_len"], 3);
    }

    #[test]
    fn json_report_exposes_structured_source_diagnostics() {
        let dir = project();
        fs::write(dir.path().join("shaders/bad.usagi.fs"), [0xff]).unwrap();

        let profiles = ShaderCheckTarget::Desktop.profiles();
        let report = check_project(dir.path(), &profiles).unwrap();
        let json = format_json_report(dir.path(), &profiles, &report).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["ok"], false);
        assert_eq!(value["shader_count"], 1);
        assert_eq!(value["compile_count"], 0);
        assert_eq!(value["failure_count"], 1);
        assert_eq!(value["warning_count"], 0);
        assert_eq!(value["failures"][0]["path"], "shaders/bad.usagi.fs");
        assert_eq!(value["failures"][0]["profile"], serde_json::Value::Null);
        assert_eq!(value["failures"][0]["kind"], "source");
        assert!(
            value["failures"][0]["message"]
                .as_str()
                .unwrap()
                .contains("not valid utf-8")
        );
        assert_eq!(value["failures"][0]["line"], serde_json::Value::Null);
        assert_eq!(value["failures"][0]["source_line"], serde_json::Value::Null);
    }

    #[test]
    fn json_report_exposes_structured_compiler_warnings() {
        let dir = project();
        fs::write(
            dir.path().join("shaders/warn.usagi.fs"),
            concat!(
                "vec4 usagi_main(vec2 uv, vec4 color) {\n",
                "    vec4 a = usagi_texture(texture0, uv);\n",
                "    vec4 b = usagi_texture(texture0, uv);\n",
                "    return a + b;\n",
                "}\n",
            ),
        )
        .unwrap();

        let profiles = ShaderCheckTarget::Desktop.profiles();
        let report = check_project(dir.path(), &profiles).unwrap();
        let text = format_warning_report(dir.path(), &report);
        let json = format_json_report(dir.path(), &profiles, &report).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(report.failures.is_empty());
        assert_eq!(report.warnings.len(), 1);
        assert!(text.contains("warn.usagi.fs [GLSL 330]"));
        assert!(text.contains("duplicate usagi_texture"));
        assert_eq!(value["ok"], true);
        assert_eq!(value["warning_count"], 1);
        assert_eq!(value["warnings"][0]["path"], "shaders/warn.usagi.fs");
        assert_eq!(value["warnings"][0]["profile"], "GLSL 330");
        assert_eq!(value["warnings"][0]["kind"], "compiler-warning");
        assert_eq!(value["warnings"][0]["line"], 3);
        assert_eq!(value["warnings"][0]["column"], 14);
    }
}
