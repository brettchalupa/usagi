use super::opt;
use super::syntax::{ShaderItem, ShaderSource, SourceSpan, UsagiShaderModule};

/// Initial backend-neutral compiler boundary.
///
/// Today it keeps the source-preserving syntax module for GLSL emission while
/// owning checked summaries that metadata, tooling, and future backends can use
/// without re-walking syntax directly.
pub(super) struct ShaderIr<'module, 'src> {
    module: &'module UsagiShaderModule<'src>,
    source: ShaderSource,
    uniforms: Vec<IrUniform<'src>>,
    functions: Vec<IrFunction<'src>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct IrUniform<'src> {
    pub(super) ty: &'src str,
    pub(super) name: &'src str,
    pub(super) ty_span: SourceSpan,
    pub(super) name_span: SourceSpan,
    pub(super) declaration_span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct IrFunction<'src> {
    pub(super) name: &'src str,
    pub(super) return_type: &'src str,
    pub(super) name_span: SourceSpan,
    pub(super) declaration_span: SourceSpan,
    pub(super) params: Vec<IrFunctionParam<'src>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct IrFunctionParam<'src> {
    pub(super) ty: &'src str,
    pub(super) name: &'src str,
    pub(super) ty_span: SourceSpan,
    pub(super) name_span: SourceSpan,
    pub(super) declaration_span: SourceSpan,
}

pub(super) fn lower<'module, 'src>(
    module: &'module UsagiShaderModule<'src>,
) -> ShaderIr<'module, 'src> {
    ShaderIr {
        module,
        source: opt::optimized_source(module),
        uniforms: lower_uniforms(module),
        functions: lower_functions(module),
    }
}

impl<'module, 'src> ShaderIr<'module, 'src> {
    pub(super) fn module(&self) -> &'module UsagiShaderModule<'src> {
        self.module
    }

    pub(super) fn source(&self) -> &ShaderSource {
        &self.source
    }

    pub(super) fn uniforms(&self) -> &[IrUniform<'src>] {
        &self.uniforms
    }

    pub(super) fn functions(&self) -> &[IrFunction<'src>] {
        &self.functions
    }
}

fn lower_uniforms<'src>(module: &UsagiShaderModule<'src>) -> Vec<IrUniform<'src>> {
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
        uniforms.extend(uniform.names.iter().map(|name| IrUniform {
            ty: uniform.ty,
            name: name.name,
            ty_span: uniform.ty_span.shifted(module.source_offset),
            name_span: name.span.shifted(module.source_offset),
            declaration_span: uniform.span.shifted(module.source_offset),
        }));
    }

    uniforms
}

fn lower_functions<'src>(module: &UsagiShaderModule<'src>) -> Vec<IrFunction<'src>> {
    module
        .items
        .iter()
        .filter_map(|item| {
            let ShaderItem::Function(function) = item else {
                return None;
            };
            Some(IrFunction {
                name: function.name,
                return_type: function.return_type,
                name_span: function.name_span.shifted(module.source_offset),
                declaration_span: function.span.shifted(module.source_offset),
                params: function
                    .params
                    .iter()
                    .map(|param| IrFunctionParam {
                        ty: param.ty,
                        name: param.name,
                        ty_span: param.ty_span.shifted(module.source_offset),
                        name_span: param.name_span.shifted(module.source_offset),
                        declaration_span: param.span.shifted(module.source_offset),
                    })
                    .collect(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::syntax::UsagiShaderModule;
    use super::*;

    #[test]
    fn lower_preserves_uniform_summaries_with_absolute_spans() {
        let src = "#usagi shader 1\n\nuniform float u_time;\nuniform vec2 u_resolution, u_origin;\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n";
        let module = UsagiShaderModule::parse_with_diagnostic(src).unwrap();
        let ir = lower(&module);

        assert_eq!(ir.uniforms().len(), 3);
        assert_eq!(ir.uniforms()[0].ty, "float");
        assert_eq!(ir.uniforms()[0].name, "u_time");
        assert_eq!(ir.uniforms()[1].ty, "vec2");
        assert_eq!(ir.uniforms()[1].name, "u_resolution");
        assert_eq!(ir.uniforms()[2].name, "u_origin");
        assert_eq!(
            &src[ir.uniforms()[0].name_span.start..ir.uniforms()[0].name_span.end],
            "u_time"
        );
        assert_eq!(
            &src[ir.uniforms()[1].ty_span.start..ir.uniforms()[1].ty_span.end],
            "vec2"
        );
    }

    #[test]
    fn lower_preserves_function_signatures_and_optimized_source() {
        let src = concat!(
            "#usagi shader 1\n\n",
            "vec4 helper(vec2 uv) { return vec4(uv, 0.0, 1.0); }\n",
            "vec4 usagi_main(vec2 uv, vec4 color) {\n",
            "    return vec4(0.2 + 0.3, color.g, color.b, color.a);\n",
            "}\n",
        );
        let module = UsagiShaderModule::parse_with_diagnostic(src).unwrap();
        let ir = lower(&module);

        assert_eq!(ir.functions().len(), 2);
        assert_eq!(ir.functions()[0].name, "helper");
        assert_eq!(ir.functions()[0].return_type, "vec4");
        assert_eq!(ir.functions()[0].params.len(), 1);
        assert_eq!(ir.functions()[0].params[0].ty, "vec2");
        assert_eq!(ir.functions()[0].params[0].name, "uv");
        assert_eq!(ir.functions()[1].name, "usagi_main");
        assert!(ir.source().rewrites.iter().any(|rewrite| matches!(
            rewrite.kind,
            super::super::syntax::SourceRewriteKind::Replacement(_)
        )));
    }
}
