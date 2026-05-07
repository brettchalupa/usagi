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
mod opt;
mod syntax;

use self::syntax::{ShaderItem, SourceSpan, UsagiShaderModule};
use super::ShaderProfile;

pub(crate) fn compile_fragment_with_metadata(
    src: &str,
    profile: ShaderProfile,
) -> Result<CompiledFragment, String> {
    compile_fragment_with_report(src, profile).map_err(|err| err.render())
}

pub(crate) fn compile_fragment_with_report(
    src: &str,
    profile: ShaderProfile,
) -> Result<CompiledFragment, CompileFailure> {
    let module = UsagiShaderModule::parse_with_diagnostic(src).map_err(|err| {
        CompileFailure::from_diagnostic(err.error.to_diagnostic(src, err.source_offset))
    })?;
    check::validate(&module, profile).map_err(|err| {
        CompileFailure::from_diagnostic(err.to_diagnostic(src, module.source_offset))
    })?;
    let warnings = check::warnings(&module)
        .into_iter()
        .map(|warning| warning.to_diagnostic(src, module.source_offset))
        .collect();
    let ir = ir::lower(&module);
    let emission = emit_glsl::emit(&ir, profile)
        .map_err(|message| CompileFailure::from_diagnostic(ShaderDiagnostic::new(message)))?;
    let metadata = ShaderMetadata::from_module(profile, &module, emission.source_map, warnings);
    Ok(CompiledFragment {
        source: emission.source,
        metadata,
    })
}

#[cfg(not(target_os = "emscripten"))]
pub(crate) fn inspect_fragment(src: &str) -> Result<ShaderInspection, CompileFailure> {
    let module = UsagiShaderModule::parse_with_diagnostic(src).map_err(|err| {
        CompileFailure::from_diagnostic(err.error.to_diagnostic(src, err.source_offset))
    })?;
    Ok(ShaderInspection::from_module(&module))
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
    pub(crate) warnings: Vec<ShaderDiagnostic>,
    pub(crate) source_map: ShaderSourceMap,
}

impl ShaderMetadata {
    fn from_module(
        profile: ShaderProfile,
        module: &UsagiShaderModule<'_>,
        source_map: ShaderSourceMap,
        warnings: Vec<ShaderDiagnostic>,
    ) -> Self {
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

        Self {
            profile,
            uniforms,
            warnings,
            source_map,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ShaderSourceMap {
    pub(crate) lines: Vec<ShaderSourceMapLine>,
}

impl ShaderSourceMap {
    pub(super) fn new() -> Self {
        Self { lines: Vec::new() }
    }

    #[allow(
        dead_code,
        reason = "line-exact lookup is used by tests now and by GL-log remapping once raylib log capture is wired"
    )]
    pub(crate) fn original_line_for_generated_line(&self, generated_line: usize) -> Option<usize> {
        self.lines
            .iter()
            .find(|line| line.generated_line == generated_line)
            .and_then(|line| line.source_line)
    }

    pub(crate) fn generated_source_line_range(&self) -> Option<(usize, usize)> {
        let mut source_lines = self
            .lines
            .iter()
            .filter(|line| line.source_line.is_some())
            .map(|line| line.generated_line);
        let first = source_lines.next()?;
        let last = source_lines.next_back().unwrap_or(first);
        Some((first, last))
    }

    pub(crate) fn original_source_line_range(&self) -> Option<(usize, usize)> {
        let mut source_lines = self.lines.iter().filter_map(|line| line.source_line);
        let first = source_lines.next()?;
        let last = source_lines.next_back().unwrap_or(first);
        Some((first, last))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ShaderSourceMapLine {
    pub(crate) generated_line: usize,
    pub(crate) source_line: Option<usize>,
    pub(crate) kind: ShaderSourceMapLineKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShaderSourceMapLineKind {
    Generated,
    Source,
}

impl ShaderSourceMapLineKind {
    #[allow(
        dead_code,
        reason = "native JSON emit and LSP use this; the web runtime keeps source maps but has no CLI/LSP"
    )]
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Generated => "generated",
            Self::Source => "source",
        }
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

#[cfg(not(target_os = "emscripten"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ShaderInspection {
    pub(crate) symbols: Vec<ShaderSymbol>,
}

#[cfg(not(target_os = "emscripten"))]
impl ShaderInspection {
    fn from_module(module: &UsagiShaderModule<'_>) -> Self {
        let mut symbols = Vec::new();
        for item in &module.items {
            match item {
                ShaderItem::Uniform(uniform) => {
                    symbols.extend(uniform.names.iter().map(|name| ShaderSymbol {
                        kind: ShaderSymbolKind::Uniform,
                        name: name.name.to_string(),
                        ty: uniform.ty.to_string(),
                        name_span: name.span.shifted(module.source_offset),
                        declaration_span: uniform.span.shifted(module.source_offset),
                    }));
                }
                ShaderItem::Function(function) => {
                    symbols.push(ShaderSymbol {
                        kind: ShaderSymbolKind::Function,
                        name: function.name.to_string(),
                        ty: function.return_type.to_string(),
                        name_span: function.name_span.shifted(module.source_offset),
                        declaration_span: function.span.shifted(module.source_offset),
                    });
                }
                ShaderItem::Raw(_) => {}
            }
        }
        Self { symbols }
    }
}

#[cfg(not(target_os = "emscripten"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ShaderSymbol {
    pub(crate) kind: ShaderSymbolKind,
    pub(crate) name: String,
    pub(crate) ty: String,
    pub(crate) name_span: SourceSpan,
    pub(crate) declaration_span: SourceSpan,
}

#[cfg(not(target_os = "emscripten"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShaderSymbolKind {
    Function,
    Uniform,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CompileFailure {
    pub(crate) diagnostic: Box<ShaderDiagnostic>,
}

impl CompileFailure {
    fn from_diagnostic(diagnostic: ShaderDiagnostic) -> Self {
        Self {
            diagnostic: Box::new(diagnostic),
        }
    }

    pub(crate) fn render(&self) -> String {
        self.diagnostic.render()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ShaderDiagnostic {
    pub(crate) message: String,
    pub(crate) line: Option<usize>,
    pub(crate) column: Option<usize>,
    pub(crate) byte_start: Option<usize>,
    pub(crate) byte_end: Option<usize>,
    pub(crate) source_line: Option<String>,
    pub(crate) marker_len: Option<usize>,
}

impl ShaderDiagnostic {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            line: None,
            column: None,
            byte_start: None,
            byte_end: None,
            source_line: None,
            marker_len: None,
        }
    }

    pub(crate) fn render(&self) -> String {
        let (Some(line), Some(column), Some(source_line), Some(marker_len)) = (
            self.line,
            self.column,
            self.source_line.as_deref(),
            self.marker_len,
        ) else {
            return self.message.clone();
        };

        format!(
            "{} at line {}, column {}\n{}\n{}{}",
            self.message,
            line,
            column,
            source_line,
            " ".repeat(column.saturating_sub(1)),
            "^".repeat(marker_len)
        )
    }
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

    fn to_diagnostic(&self, src: &str, source_offset: usize) -> ShaderDiagnostic {
        diagnostic_from_parts(&self.message, self.span, src, source_offset)
    }

    #[cfg(test)]
    fn render(&self, src: &str, source_offset: usize) -> String {
        self.to_diagnostic(src, source_offset).render()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CompileWarning {
    message: String,
    span: Option<SourceSpan>,
}

impl CompileWarning {
    pub(super) fn at(message: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            message: message.into(),
            span: Some(span),
        }
    }

    fn to_diagnostic(&self, src: &str, source_offset: usize) -> ShaderDiagnostic {
        diagnostic_from_parts(&self.message, self.span, src, source_offset)
    }
}

fn diagnostic_from_parts(
    message: &str,
    span: Option<SourceSpan>,
    src: &str,
    source_offset: usize,
) -> ShaderDiagnostic {
    let Some(span) = span else {
        return ShaderDiagnostic::new(message.to_string());
    };

    let absolute_start = source_offset.saturating_add(span.start).min(src.len());
    let absolute_end = source_offset.saturating_add(span.end).min(src.len());
    let (line, column, line_start, line_end) = syntax::source_location(src, absolute_start);
    let source_line = src[line_start..line_end].trim_end_matches('\r').to_string();
    let marker_len = src[absolute_start..absolute_end]
        .chars()
        .take_while(|ch| *ch != '\n' && *ch != '\r')
        .count()
        .max(1);

    ShaderDiagnostic {
        message: message.to_string(),
        line: Some(line),
        column: Some(column),
        byte_start: Some(absolute_start),
        byte_end: Some(absolute_end),
        source_line: Some(source_line),
        marker_len: Some(marker_len),
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
        assert!(compiled.metadata.warnings.is_empty());
        assert_eq!(
            compiled
                .metadata
                .source_map
                .original_line_for_generated_line(8),
            Some(3)
        );
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

    #[test]
    fn compiler_metadata_records_duplicate_texture_sample_warnings() {
        let src = concat!(
            "#usagi shader 1\n\n",
            "vec4 usagi_main(vec2 uv, vec4 color) {\n",
            "    vec4 a = usagi_texture(texture0, uv);\n",
            "    vec4 b = usagi_texture(texture0, uv);\n",
            "    return a + b;\n",
            "}\n",
        );

        let compiled = compile_fragment_with_metadata(src, ShaderProfile::DesktopGlsl330).unwrap();

        assert_eq!(compiled.metadata.warnings.len(), 1);
        let warning = &compiled.metadata.warnings[0];
        assert!(warning.message.contains("duplicate usagi_texture"));
        assert!(warning.message.contains("reuse the first sample"));
        assert_eq!(warning.line, Some(5));
        assert_eq!(warning.column, Some(14));
        assert_eq!(
            warning.source_line.as_deref(),
            Some("    vec4 b = usagi_texture(texture0, uv);")
        );
    }
}
