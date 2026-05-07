//! Offline generated GLSL emission for the native CLI.

use super::ShaderProfile;
use super::profile_cli::ShaderProfileTarget;
use crate::{Error, Result};
use clap::ValueEnum;
use serde_json::json;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum ShaderEmitFormat {
    /// Emit raw GLSL source.
    Source,
    /// Emit generated GLSL plus source-map metadata as JSON.
    Json,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EmittedShader {
    profile: ShaderProfile,
    source: String,
    source_map: super::compiler::ShaderSourceMap,
}

pub(crate) fn run(
    path_arg: &str,
    target: ShaderProfileTarget,
    format: ShaderEmitFormat,
    output: Option<&str>,
) -> Result<()> {
    let input_path = Path::new(path_arg);
    let src = read_shader_source(input_path)?;
    let emitted = emit_source(&src, target.profiles())?;

    if format == ShaderEmitFormat::Json {
        if output.is_some() {
            return Err(Error::Cli(
                "shader emit --format json writes to stdout; omit --output".into(),
            ));
        }
        println!("{}", format_json_stdout(input_path, &emitted)?);
        return Ok(());
    }

    if let Some(output) = output {
        write_emitted(input_path, Path::new(output), target, &emitted)?;
        return Ok(());
    }

    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    stdout
        .write_all(format_stdout(&emitted).as_bytes())
        .map_err(|e| Error::Cli(format!("writing generated GLSL to stdout: {e}")))?;
    Ok(())
}

fn read_shader_source(path: &Path) -> Result<String> {
    let bytes =
        fs::read(path).map_err(|e| Error::Cli(format!("reading {}: {e}", path.display())))?;
    String::from_utf8(bytes)
        .map_err(|e| Error::Cli(format!("{} is not valid utf-8: {e}", path.display())))
}

fn emit_source(src: &str, profiles: Vec<ShaderProfile>) -> Result<Vec<EmittedShader>> {
    let mut emitted = Vec::with_capacity(profiles.len());
    for profile in profiles {
        match super::compile_generic_fragment_with_report(src, profile) {
            Ok(compiled) => emitted.push(EmittedShader {
                profile,
                source: compiled.source,
                source_map: compiled.metadata.source_map,
            }),
            Err(failure) => {
                return Err(Error::Cli(format!(
                    "shader emit failed [{}]\n{}",
                    profile.label(),
                    failure.render()
                )));
            }
        }
    }
    Ok(emitted)
}

fn format_json_stdout(input_path: &Path, emitted: &[EmittedShader]) -> Result<String> {
    let outputs: Vec<_> = emitted
        .iter()
        .map(|shader| {
            json!({
                "profile": shader.profile.label(),
                "source": shader.source,
                "source_map": shader.source_map.lines.iter().map(|line| {
                    json!({
                        "generated_line": line.generated_line,
                        "source_line": line.source_line,
                        "kind": line.kind.as_str(),
                    })
                }).collect::<Vec<_>>(),
            })
        })
        .collect();

    serde_json::to_string_pretty(&json!({
        "ok": true,
        "path": input_path.display().to_string(),
        "outputs": outputs,
    }))
    .map_err(|e| Error::Cli(format!("serializing shader emit JSON: {e}")))
}

fn format_stdout(emitted: &[EmittedShader]) -> String {
    if emitted.len() == 1 {
        return emitted[0].source.clone();
    }

    let mut out = String::new();
    for shader in emitted {
        out.push_str("// ===== ");
        out.push_str(shader.profile.label());
        out.push_str(" =====\n");
        out.push_str(&shader.source);
        if !shader.source.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
    }
    out
}

fn write_emitted(
    input_path: &Path,
    output_path: &Path,
    target: ShaderProfileTarget,
    emitted: &[EmittedShader],
) -> Result<()> {
    if target.is_all() {
        fs::create_dir_all(output_path).map_err(|e| {
            Error::Cli(format!(
                "creating output directory {}: {e}",
                output_path.display()
            ))
        })?;
        for shader in emitted {
            let path = output_path.join(output_file_name(input_path, shader.profile));
            fs::write(&path, &shader.source)
                .map_err(|e| Error::Cli(format!("writing {}: {e}", path.display())))?;
            crate::msg::info!("wrote {} ({})", path.display(), shader.profile.label());
        }
        return Ok(());
    }

    let Some(shader) = emitted.first() else {
        return Err(Error::Cli("no generated GLSL output produced".into()));
    };
    if let Some(parent) = output_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|e| {
            Error::Cli(format!(
                "creating output directory {}: {e}",
                parent.display()
            ))
        })?;
    }
    fs::write(output_path, &shader.source)
        .map_err(|e| Error::Cli(format!("writing {}: {e}", output_path.display())))?;
    crate::msg::info!(
        "wrote {} ({})",
        output_path.display(),
        shader.profile.label()
    );
    Ok(())
}

fn output_file_name(input_path: &Path, profile: ShaderProfile) -> String {
    format!(
        "{}.{}.fs",
        shader_base_name(input_path),
        profile_file_suffix(profile)
    )
}

fn shader_base_name(input_path: &Path) -> String {
    let file_name = input_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("shader");
    file_name
        .strip_suffix(".usagi.fs")
        .or_else(|| file_name.strip_suffix(".fs"))
        .unwrap_or(file_name)
        .to_string()
}

fn profile_file_suffix(profile: ShaderProfile) -> &'static str {
    match profile {
        ShaderProfile::WebGlslEs100 => "es100",
        ShaderProfile::DesktopGlsl330 => "glsl330",
        ShaderProfile::DesktopGlsl440 => "glsl440",
    }
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

    #[test]
    fn emit_targets_map_to_profiles() {
        assert_eq!(
            ShaderProfileTarget::Desktop.profiles(),
            vec![ShaderProfile::DesktopGlsl330]
        );
        assert_eq!(
            ShaderProfileTarget::Web.profiles(),
            vec![ShaderProfile::WebGlslEs100]
        );
        assert_eq!(
            ShaderProfileTarget::Glsl440.profiles(),
            vec![ShaderProfile::DesktopGlsl440]
        );
        assert_eq!(
            ShaderProfileTarget::All.profiles(),
            vec![
                ShaderProfile::WebGlslEs100,
                ShaderProfile::DesktopGlsl330,
                ShaderProfile::DesktopGlsl440,
            ]
        );
    }

    #[test]
    fn emits_selected_profile_source() {
        let emitted = emit_source(VALID_SHADER, ShaderProfileTarget::Web.profiles()).unwrap();

        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].profile, ShaderProfile::WebGlslEs100);
        assert!(emitted[0].source.contains("#version 100"));
        assert!(emitted[0].source.contains("texture2D(texture0, uv)"));
        assert_eq!(
            emitted[0].source_map.original_line_for_generated_line(9),
            Some(3)
        );
    }

    #[test]
    fn formats_all_profiles_with_headers() {
        let emitted = emit_source(VALID_SHADER, ShaderProfileTarget::All.profiles()).unwrap();
        let output = format_stdout(&emitted);

        assert!(output.contains("// ===== GLSL ES 100 ====="));
        assert!(output.contains("// ===== GLSL 330 ====="));
        assert!(output.contains("// ===== GLSL 440 ====="));
    }

    #[test]
    fn output_file_names_strip_usagi_suffix_and_add_profile_suffix() {
        let input = Path::new("shaders/crt.usagi.fs");

        assert_eq!(
            output_file_name(input, ShaderProfile::WebGlslEs100),
            "crt.es100.fs"
        );
        assert_eq!(
            output_file_name(input, ShaderProfile::DesktopGlsl330),
            "crt.glsl330.fs"
        );
        assert_eq!(
            output_file_name(input, ShaderProfile::DesktopGlsl440),
            "crt.glsl440.fs"
        );
    }

    #[test]
    fn writes_all_outputs_to_directory() {
        let dir = TempDir::new().unwrap();
        let input = dir.path().join("crt.usagi.fs");
        fs::write(&input, VALID_SHADER).unwrap();
        let emitted = emit_source(VALID_SHADER, ShaderProfileTarget::All.profiles()).unwrap();
        let output_dir = dir.path().join("generated");

        write_emitted(&input, &output_dir, ShaderProfileTarget::All, &emitted).unwrap();

        assert!(
            fs::read_to_string(output_dir.join("crt.es100.fs"))
                .unwrap()
                .contains("#version 100")
        );
        assert!(
            fs::read_to_string(output_dir.join("crt.glsl330.fs"))
                .unwrap()
                .contains("#version 330")
        );
        assert!(
            fs::read_to_string(output_dir.join("crt.glsl440.fs"))
                .unwrap()
                .contains("#version 440 core")
        );
    }

    #[test]
    fn invalid_shader_reports_profile_and_diagnostic() {
        let err = emit_source(
            "vec4 usagi_main(vec2 uv, vec4 color) { return texture(texture0, uv); }\n",
            ShaderProfileTarget::Desktop.profiles(),
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("shader emit failed [GLSL 330]"));
        assert!(err.contains("generic shaders must use usagi_texture"));
    }

    #[test]
    fn json_output_includes_generated_source_map() {
        let emitted = emit_source(VALID_SHADER, ShaderProfileTarget::Desktop.profiles()).unwrap();
        let json = format_json_stdout(Path::new("shaders/crt.usagi.fs"), &emitted).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["ok"], true);
        assert_eq!(value["outputs"][0]["profile"], "GLSL 330");
        assert!(
            value["outputs"][0]["source"]
                .as_str()
                .unwrap()
                .contains("#version 330")
        );
        assert_eq!(value["outputs"][0]["source_map"][0]["kind"], "generated");
        assert!(
            value["outputs"][0]["source_map"]
                .as_array()
                .unwrap()
                .iter()
                .any(|line| line["source_line"] == 4)
        );
    }
}
