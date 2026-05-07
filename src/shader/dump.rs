//! Runtime generated GLSL dump support for generic shaders.
//!
//! Dumps are deliberately opt-in and best-effort. Shader load must never fail
//! because a debug artifact cannot be written.

use super::FragmentSource;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

const SHADER_DUMP_ENV: &str = "USAGI_SHADER_DUMP_DIR";

#[derive(Debug, PartialEq, Eq)]
struct DumpedShaderPaths {
    source_path: PathBuf,
    metadata_path: PathBuf,
}

pub(super) fn dump_generated_fragment(shader_name: &str, fragment: &FragmentSource) {
    if fragment.metadata.is_none() {
        return;
    }
    let Some(dir) = std::env::var_os(SHADER_DUMP_ENV) else {
        return;
    };
    let dir = PathBuf::from(dir);
    if dir.as_os_str().is_empty() {
        return;
    }

    match dump_generated_fragment_to_dir(&dir, shader_name, fragment) {
        Ok(Some(paths)) => crate::msg::info!(
            "shader '{shader_name}': dumped generated GLSL to {}",
            paths.source_path.display()
        ),
        Ok(None) => {}
        Err(e) => crate::msg::warn!(
            "shader '{shader_name}': failed to dump generated GLSL to {}: {e}",
            dir.display()
        ),
    }
}

fn dump_generated_fragment_to_dir(
    dir: &Path,
    shader_name: &str,
    fragment: &FragmentSource,
) -> std::io::Result<Option<DumpedShaderPaths>> {
    let Some(metadata) = fragment.metadata.as_ref() else {
        return Ok(None);
    };

    fs::create_dir_all(dir)?;
    let stem = dump_file_stem(shader_name, metadata.profile);
    let source_path = dir.join(format!("{stem}.fs"));
    let metadata_path = dir.join(format!("{stem}.json"));

    fs::write(&source_path, &fragment.source)?;
    let metadata_json = serde_json::to_string_pretty(&json!({
        "shader": shader_name,
        "source_key": fragment.key,
        "profile": metadata.profile.label(),
        "profile_suffix": metadata.profile.file_suffix(),
        "generated_source": fragment.source,
        "generated_source_path": source_path.file_name().and_then(|name| name.to_str()),
        "uniforms": metadata.uniforms.iter().map(|uniform| {
            json!({
                "name": uniform.name,
                "type": uniform.ty,
                "declaration_span": span_json(uniform.declaration_span.start, uniform.declaration_span.end),
                "name_span": span_json(uniform.name_span.start, uniform.name_span.end),
                "type_span": span_json(uniform.ty_span.start, uniform.ty_span.end),
            })
        }).collect::<Vec<_>>(),
        "warnings": metadata.warnings.iter().map(|diagnostic| {
            json!({
                "message": diagnostic.message,
                "line": diagnostic.line,
                "column": diagnostic.column,
                "byte_start": diagnostic.byte_start,
                "byte_end": diagnostic.byte_end,
                "source_line": diagnostic.source_line,
                "marker_len": diagnostic.marker_len,
            })
        }).collect::<Vec<_>>(),
        "source_map": metadata.source_map.lines.iter().map(|line| {
            json!({
                "generated_line": line.generated_line,
                "source_line": line.source_line,
                "kind": line.kind.as_str(),
            })
        }).collect::<Vec<_>>(),
    }))?;
    fs::write(&metadata_path, metadata_json)?;

    Ok(Some(DumpedShaderPaths {
        source_path,
        metadata_path,
    }))
}

fn dump_file_stem(shader_name: &str, profile: super::ShaderProfile) -> String {
    format!(
        "{}.{}",
        sanitize_shader_name(shader_name),
        profile.file_suffix()
    )
}

fn sanitize_shader_name(shader_name: &str) -> String {
    let mut out = String::with_capacity(shader_name.len().max(6));
    for ch in shader_name.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let out = out.trim_matches('.').to_string();
    let out = if out.is_empty() {
        "shader".to_string()
    } else {
        out
    };
    if is_windows_reserved_file_stem(&out) {
        format!("_{out}")
    } else {
        out
    }
}

fn is_windows_reserved_file_stem(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name);
    let upper = stem.to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

fn span_json(start: usize, end: usize) -> serde_json::Value {
    json!({
        "byte_start": start,
        "byte_end": end,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shader::{FragmentSource, ShaderProfile, generate_generic_fragment_with_metadata};
    use tempfile::TempDir;

    const VALID_SHADER: &str = concat!(
        "#usagi shader 1\n\n",
        "uniform float u_time;\n",
        "vec4 usagi_main(vec2 uv, vec4 color) {\n",
        "    vec4 a = usagi_texture(texture0, uv);\n",
        "    vec4 b = usagi_texture(texture0, uv);\n",
        "    return (a + b) * color * u_time;\n",
        "}\n",
    );

    #[test]
    fn runtime_dump_writes_generated_source_and_metadata_json() {
        let dir = TempDir::new().unwrap();
        let compiled =
            generate_generic_fragment_with_metadata(VALID_SHADER, ShaderProfile::DesktopGlsl330)
                .unwrap();
        let fragment = FragmentSource {
            key: "shaders/crt.usagi.fs".to_string(),
            source: compiled.source,
            metadata: Some(compiled.metadata),
        };

        let paths = dump_generated_fragment_to_dir(dir.path(), "crt", &fragment)
            .unwrap()
            .expect("generic shader should dump");

        assert_eq!(
            paths.source_path.file_name().and_then(|name| name.to_str()),
            Some("crt.glsl330.fs")
        );
        assert_eq!(
            paths
                .metadata_path
                .file_name()
                .and_then(|name| name.to_str()),
            Some("crt.glsl330.json")
        );
        assert!(
            fs::read_to_string(&paths.source_path)
                .unwrap()
                .contains("#version 330")
        );

        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&paths.metadata_path).unwrap()).unwrap();
        assert_eq!(value["shader"], "crt");
        assert_eq!(value["source_key"], "shaders/crt.usagi.fs");
        assert_eq!(value["profile"], "GLSL 330");
        assert_eq!(value["profile_suffix"], "glsl330");
        assert!(
            value["generated_source"]
                .as_str()
                .unwrap()
                .contains("texture(texture0, uv)")
        );
        assert_eq!(value["uniforms"][0]["name"], "u_time");
        assert!(
            value["warnings"][0]["message"]
                .as_str()
                .unwrap()
                .contains("duplicate usagi_texture")
        );
        assert!(
            value["source_map"]
                .as_array()
                .unwrap()
                .iter()
                .any(|line| line["source_line"] == 4)
        );
    }

    #[test]
    fn runtime_dump_ignores_native_fallback_fragments() {
        let dir = TempDir::new().unwrap();
        let fragment = FragmentSource {
            key: "shaders/native.fs".to_string(),
            source: "#version 330\nvoid main() {}\n".to_string(),
            metadata: None,
        };

        let paths = dump_generated_fragment_to_dir(dir.path(), "native", &fragment).unwrap();

        assert_eq!(paths, None);
        assert!(fs::read_dir(dir.path()).unwrap().next().is_none());
    }

    #[test]
    fn dump_file_names_are_safe_for_windows_paths() {
        assert_eq!(
            dump_file_stem("world/crt:final", ShaderProfile::WebGlslEs100),
            "world_crt_final.es100"
        );
        assert_eq!(
            dump_file_stem("CON", ShaderProfile::DesktopGlsl440),
            "_CON.glsl440"
        );
        assert_eq!(
            dump_file_stem("...", ShaderProfile::DesktopGlsl330),
            "shader.glsl330"
        );
    }
}
