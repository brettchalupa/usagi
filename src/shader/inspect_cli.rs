//! Offline shader metadata inspection for the native CLI.

use super::ShaderProfile;
use super::compiler::{ShaderMetadata, ShaderUniform};
use super::profile_cli::ShaderProfileTarget;
use crate::{Error, Result};
use clap::ValueEnum;
use serde_json::json;
use std::fs;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum ShaderInspectFormat {
    /// Human-readable terminal output.
    Text,
    /// Structured JSON for editor integrations and tooling.
    Json,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ShaderInspectReport {
    profile: ShaderProfile,
    metadata: ShaderMetadata,
    generated_bytes: usize,
    generated_lines: usize,
}

pub(crate) fn run(
    path_arg: &str,
    target: ShaderProfileTarget,
    format: ShaderInspectFormat,
) -> Result<()> {
    let input_path = Path::new(path_arg);
    let src = read_shader_source(input_path)?;

    match inspect_source(&src, target.profiles()) {
        Ok(reports) => {
            if format == ShaderInspectFormat::Json {
                println!("{}", format_json_report(input_path, &src, &reports)?);
            } else {
                println!("{}", format_text_report(input_path, &src, &reports));
            }
            Ok(())
        }
        Err(failure) => {
            if format == ShaderInspectFormat::Json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "ok": false,
                        "path": input_path.display().to_string(),
                        "diagnostic": diagnostic_json(failure.diagnostic.as_ref()),
                    }))
                    .map_err(|e| Error::Cli(format!("serializing shader inspect JSON: {e}")))?
                );
            }
            Err(Error::Cli(format!(
                "shader inspect failed\n{}",
                failure.render()
            )))
        }
    }
}

fn read_shader_source(path: &Path) -> Result<String> {
    let bytes =
        fs::read(path).map_err(|e| Error::Cli(format!("reading {}: {e}", path.display())))?;
    String::from_utf8(bytes)
        .map_err(|e| Error::Cli(format!("{} is not valid utf-8: {e}", path.display())))
}

fn inspect_source(
    src: &str,
    profiles: Vec<ShaderProfile>,
) -> std::result::Result<Vec<ShaderInspectReport>, super::compiler::CompileFailure> {
    let mut reports = Vec::with_capacity(profiles.len());
    for profile in profiles {
        let compiled = super::compile_generic_fragment_with_report(src, profile)?;
        reports.push(ShaderInspectReport {
            profile,
            generated_bytes: compiled.source.len(),
            generated_lines: compiled.metadata.source_map.lines.len(),
            metadata: compiled.metadata,
        });
    }
    Ok(reports)
}

fn format_text_report(input_path: &Path, src: &str, reports: &[ShaderInspectReport]) -> String {
    let mut out = format!("shader inspect: {}\n", input_path.display());

    for report in reports {
        out.push('\n');
        out.push_str(report.profile.label());
        out.push('\n');
        out.push_str(&format!(
            "  generated: {} byte(s), {} line(s)\n",
            report.generated_bytes, report.generated_lines
        ));
        if let Some((first, last)) = report.metadata.source_map.generated_source_line_range() {
            out.push_str(&format!("  generated source lines: {}-{}\n", first, last));
        }
        if let Some((first, last)) = report.metadata.source_map.original_source_line_range() {
            out.push_str(&format!("  usagi source lines: {}-{}\n", first, last));
        }
        if report.metadata.uniforms.is_empty() {
            out.push_str("  uniforms: none\n");
            continue;
        }
        out.push_str("  uniforms:\n");
        for uniform in &report.metadata.uniforms {
            let start = source_position(src, uniform.declaration_span.start);
            out.push_str(&format!(
                "    {} {} at line {}, column {}\n",
                uniform.ty, uniform.name, start.line, start.column
            ));
        }
    }

    out
}

fn format_json_report(
    input_path: &Path,
    src: &str,
    reports: &[ShaderInspectReport],
) -> Result<String> {
    let profiles = reports
        .iter()
        .map(|report| {
            let generated_range = report.metadata.source_map.generated_source_line_range();
            let source_range = report.metadata.source_map.original_source_line_range();
            json!({
                "profile": report.profile.label(),
                "generated": {
                    "bytes": report.generated_bytes,
                    "lines": report.generated_lines,
                    "source_line_range": line_range_json(generated_range),
                    "usagi_line_range": line_range_json(source_range),
                },
                "uniforms": report.metadata.uniforms.iter().map(|uniform| {
                    uniform_json(src, uniform)
                }).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();

    serde_json::to_string_pretty(&json!({
        "ok": true,
        "path": input_path.display().to_string(),
        "profiles": profiles,
    }))
    .map_err(|e| Error::Cli(format!("serializing shader inspect JSON: {e}")))
}

fn uniform_json(src: &str, uniform: &ShaderUniform) -> serde_json::Value {
    json!({
        "name": uniform.name,
        "type": uniform.ty,
        "declaration": span_json(src, uniform.declaration_span.start, uniform.declaration_span.end),
        "name_span": span_json(src, uniform.name_span.start, uniform.name_span.end),
        "type_span": span_json(src, uniform.ty_span.start, uniform.ty_span.end),
    })
}

fn diagnostic_json(diagnostic: &super::compiler::ShaderDiagnostic) -> serde_json::Value {
    json!({
        "message": &diagnostic.message,
        "line": diagnostic.line,
        "column": diagnostic.column,
        "byte_start": diagnostic.byte_start,
        "byte_end": diagnostic.byte_end,
        "source_line": &diagnostic.source_line,
        "marker_len": diagnostic.marker_len,
    })
}

fn span_json(src: &str, start: usize, end: usize) -> serde_json::Value {
    let start_pos = source_position(src, start);
    let end_pos = source_position(src, end);
    json!({
        "byte_start": start,
        "byte_end": end,
        "line": start_pos.line,
        "column": start_pos.column,
        "end_line": end_pos.line,
        "end_column": end_pos.column,
    })
}

fn line_range_json(range: Option<(usize, usize)>) -> serde_json::Value {
    match range {
        Some((first, last)) => json!({
            "first": first,
            "last": last,
        }),
        None => serde_json::Value::Null,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SourcePosition {
    line: usize,
    column: usize,
}

fn source_position(src: &str, byte_idx: usize) -> SourcePosition {
    let byte_idx = byte_idx.min(src.len());
    let prefix = &src[..byte_idx];
    let line = prefix.bytes().filter(|b| *b == b'\n').count() + 1;
    let line_start = prefix.rfind('\n').map_or(0, |idx| idx + 1);
    let column = src[line_start..byte_idx].chars().count() + 1;
    SourcePosition { line, column }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_SHADER: &str = concat!(
        "#usagi shader 1\n\n",
        "uniform float u_time;\n",
        "uniform vec2 u_resolution, u_origin;\n",
        "vec4 usagi_main(vec2 uv, vec4 color) {\n",
        "    return usagi_texture(texture0, uv) * color * u_time;\n",
        "}\n",
    );

    #[test]
    fn inspect_collects_uniform_metadata_for_selected_profile() {
        let reports =
            inspect_source(VALID_SHADER, ShaderProfileTarget::Desktop.profiles()).unwrap();

        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].profile, ShaderProfile::DesktopGlsl330);
        assert_eq!(reports[0].metadata.uniforms.len(), 3);
        assert_eq!(reports[0].metadata.uniforms[0].name, "u_time");
        assert_eq!(reports[0].metadata.uniforms[1].name, "u_resolution");
        assert_eq!(reports[0].metadata.uniforms[2].name, "u_origin");
    }

    #[test]
    fn inspect_json_includes_uniform_spans_and_generated_ranges() {
        let reports = inspect_source(VALID_SHADER, ShaderProfileTarget::Web.profiles()).unwrap();
        let json =
            format_json_report(Path::new("shaders/crt.usagi.fs"), VALID_SHADER, &reports).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["ok"], true);
        assert_eq!(value["profiles"][0]["profile"], "GLSL ES 100");
        assert_eq!(value["profiles"][0]["uniforms"][0]["name"], "u_time");
        assert_eq!(value["profiles"][0]["uniforms"][0]["type"], "float");
        assert_eq!(
            value["profiles"][0]["uniforms"][0]["declaration"]["line"],
            3
        );
        assert_eq!(
            value["profiles"][0]["generated"]["source_line_range"]["first"],
            9
        );
        assert_eq!(
            value["profiles"][0]["generated"]["usagi_line_range"]["first"],
            3
        );
    }

    #[test]
    fn inspect_text_reports_empty_uniforms() {
        let shader = concat!(
            "vec4 usagi_main(vec2 uv, vec4 color) {\n",
            "    return usagi_texture(texture0, uv) * color;\n",
            "}\n",
        );
        let reports = inspect_source(shader, ShaderProfileTarget::Desktop.profiles()).unwrap();
        let output = format_text_report(Path::new("shaders/plain.usagi.fs"), shader, &reports);

        assert!(output.contains("shader inspect: shaders/plain.usagi.fs"));
        assert!(output.contains("GLSL 330"));
        assert!(output.contains("uniforms: none"));
    }

    #[test]
    fn inspect_propagates_compiler_diagnostics() {
        let err = inspect_source(
            "vec4 usagi_main(vec2 uv, vec4 color) { return texture(texture0, uv); }\n",
            ShaderProfileTarget::Desktop.profiles(),
        )
        .unwrap_err()
        .render();

        assert!(err.contains("generic shaders must use usagi_texture"));
    }

    #[test]
    fn diagnostic_json_preserves_structured_span_fields() {
        let failure = inspect_source(
            "vec4 usagi_main(vec2 uv, vec4 color) { return texture(texture0, uv); }\n",
            ShaderProfileTarget::Desktop.profiles(),
        )
        .unwrap_err();
        let value = diagnostic_json(failure.diagnostic.as_ref());

        assert!(
            value["message"]
                .as_str()
                .unwrap()
                .contains("generic shaders must use usagi_texture")
        );
        assert!(value["line"].is_number());
        assert!(value["column"].is_number());
        assert!(value["byte_start"].is_number());
        assert!(value["byte_end"].is_number());
    }
}
