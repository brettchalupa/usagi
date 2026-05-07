//! Compiler for the `.usagi.fs` shader dialect.
//!
//! The dialect is intentionally close to fragment GLSL, but the source
//! is not passed through verbatim. The current compiler builds the
//! initial module representation needed for target-aware emission; the
//! production target is a complete AST/ABT so every accepted Usagi shader
//! construct lowers or fails deterministically before GL driver compile.
//!
//! Current contract:
//! - an optional `#usagi shader 1` marker may appear as the first non-blank line;
//! - source must define exactly one `vec4 usagi_main(vec2 uv, vec4 color)`;
//! - `texture0`, `fragTexCoord`, `fragColor`, `finalColor`, `gl_FragColor`,
//!   and `main` are emitter-owned names;
//! - `usagi_texture(texture0, uv)` is the target-neutral texture intrinsic;
//! - direct `texture(...)` / `texture2D(...)` calls are rejected so generic
//!   sources remain portable across GLSL ES 100, GLSL 330, and staged GLSL 440.

mod check;
mod emit_glsl;
mod ir;
mod syntax;

use self::syntax::{ShaderItem, SourceSpan, UsagiShaderModule};
use super::ShaderProfile;

pub(crate) fn compile_fragment_with_metadata(
    src: &str,
    profile: ShaderProfile,
) -> Result<CompiledFragment, String> {
    let module = UsagiShaderModule::parse(src)?;
    check::validate(&module, profile).map_err(|err| err.render(src, module.source_offset))?;
    let ir = ir::lower(&module);
    let source = emit_glsl::emit(&ir, profile)?;
    let metadata = ShaderMetadata::from_module(profile, &module);
    Ok(CompiledFragment { source, metadata })
}

pub(super) type CompileResult<T> = Result<T, CompileError>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CompiledFragment {
    pub(crate) source: String,
    pub(crate) metadata: ShaderMetadata,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ShaderMetadata {
    pub(crate) profile: ShaderProfile,
    pub(crate) uniforms: Vec<ShaderUniform>,
}

impl ShaderMetadata {
    fn from_module(profile: ShaderProfile, module: &UsagiShaderModule<'_>) -> Self {
        let uniform_count = module
            .items
            .iter()
            .filter_map(|item| match item {
                ShaderItem::Uniform(uniform) => Some(uniform.names.len()),
                ShaderItem::Function(_) | ShaderItem::Raw(_) => None,
            })
            .sum();
        let mut uniforms = Vec::with_capacity(uniform_count);

        for item in &module.items {
            let ShaderItem::Uniform(uniform) = item else {
                continue;
            };
            uniforms.extend(uniform.names.iter().map(|name| ShaderUniform {
                ty: uniform.ty.to_string(),
                name: name.name.to_string(),
                ty_span: uniform.ty_span.shifted(module.source_offset),
                name_span: name.span.shifted(module.source_offset),
                declaration_span: uniform.span.shifted(module.source_offset),
            }));
        }

        Self { profile, uniforms }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ShaderUniform {
    pub(crate) ty: String,
    pub(crate) name: String,
    pub(crate) ty_span: SourceSpan,
    pub(crate) name_span: SourceSpan,
    pub(crate) declaration_span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CompileError {
    message: String,
    span: Option<SourceSpan>,
}

impl CompileError {
    pub(super) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
        }
    }

    pub(super) fn at(message: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            message: message.into(),
            span: Some(span),
        }
    }

    fn render(&self, src: &str, source_offset: usize) -> String {
        let Some(span) = self.span else {
            return self.message.clone();
        };

        let absolute_start = source_offset.saturating_add(span.start).min(src.len());
        let absolute_end = source_offset.saturating_add(span.end).min(src.len());
        let (line, column, line_start, line_end) = syntax::source_location(src, absolute_start);
        let line_text = src[line_start..line_end].trim_end_matches('\r');
        let marker_len = src[absolute_start..absolute_end]
            .chars()
            .take_while(|ch| *ch != '\n' && *ch != '\r')
            .count()
            .max(1);

        format!(
            "{} at line {}, column {}\n{}\n{}{}",
            self.message,
            line,
            column,
            line_text,
            " ".repeat(column.saturating_sub(1)),
            "^".repeat(marker_len)
        )
    }
}

impl From<String> for CompileError {
    fn from(message: String) -> Self {
        Self::new(message)
    }
}

impl From<&str> for CompileError {
    fn from(message: &str) -> Self {
        Self::new(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiler_errors_include_line_column_and_source_snippet() {
        let src = "#usagi shader 1\n\nvec4 usagi_main(vec2 uv, vec4 color) {\n    return texture(texture0, uv);\n}\n";
        let err = compile_fragment_with_metadata(src, ShaderProfile::DesktopGlsl330).unwrap_err();

        assert!(err.contains("line 4, column 12"));
        assert!(err.contains("return texture(texture0, uv);"));
        assert!(err.contains("           ^^^^^^^"));
    }

    #[test]
    fn compiler_metadata_records_profile_uniforms_and_source_spans() {
        let src = concat!(
            "#usagi shader 1\n\n",
            "uniform float u_time;\n",
            "uniform vec2 u_resolution, u_origin;\n\n",
            "vec4 usagi_main(vec2 uv, vec4 color) {\n",
            "    return color * u_time + ",
            "vec4(u_resolution / max(u_origin, vec2(1.0)), 0.0, 0.0);\n",
            "}\n",
        );
        let compiled = compile_fragment_with_metadata(src, ShaderProfile::DesktopGlsl330).unwrap();

        assert_eq!(compiled.metadata.profile, ShaderProfile::DesktopGlsl330);
        assert_eq!(compiled.metadata.uniforms.len(), 3);
        assert_eq!(compiled.metadata.uniforms[0].ty, "float");
        assert_eq!(compiled.metadata.uniforms[0].name, "u_time");
        assert_eq!(compiled.metadata.uniforms[1].ty, "vec2");
        assert_eq!(compiled.metadata.uniforms[1].name, "u_resolution");
        assert_eq!(compiled.metadata.uniforms[2].ty, "vec2");
        assert_eq!(compiled.metadata.uniforms[2].name, "u_origin");
        for uniform in &compiled.metadata.uniforms {
            assert_eq!(
                &src[uniform.name_span.start..uniform.name_span.end],
                uniform.name
            );
            assert_eq!(&src[uniform.ty_span.start..uniform.ty_span.end], uniform.ty);
            assert!(
                src[uniform.declaration_span.start..uniform.declaration_span.end]
                    .starts_with("uniform ")
            );
        }
    }
}
