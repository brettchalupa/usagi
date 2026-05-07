use super::opt;
use super::syntax::{ShaderItem, ShaderSource, SourceSpan, UsagiShaderModule};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum IrType {
    Void,
    Bool,
    Int,
    Float,
    Vec(u8),
    BVec(u8),
    IVec(u8),
    Mat(u8),
    Sampler2D,
}

impl IrType {
    pub(super) fn parse(src: &str) -> Option<Self> {
        match src {
            "void" => Some(Self::Void),
            "bool" => Some(Self::Bool),
            "int" => Some(Self::Int),
            "float" => Some(Self::Float),
            "vec2" => Some(Self::Vec(2)),
            "vec3" => Some(Self::Vec(3)),
            "vec4" => Some(Self::Vec(4)),
            "bvec2" => Some(Self::BVec(2)),
            "bvec3" => Some(Self::BVec(3)),
            "bvec4" => Some(Self::BVec(4)),
            "ivec2" => Some(Self::IVec(2)),
            "ivec3" => Some(Self::IVec(3)),
            "ivec4" => Some(Self::IVec(4)),
            "mat2" => Some(Self::Mat(2)),
            "mat3" => Some(Self::Mat(3)),
            "mat4" => Some(Self::Mat(4)),
            "sampler2D" => Some(Self::Sampler2D),
            _ => None,
        }
    }

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Void => "void",
            Self::Bool => "bool",
            Self::Int => "int",
            Self::Float => "float",
            Self::Vec(2) => "vec2",
            Self::Vec(3) => "vec3",
            Self::Vec(4) => "vec4",
            Self::BVec(2) => "bvec2",
            Self::BVec(3) => "bvec3",
            Self::BVec(4) => "bvec4",
            Self::IVec(2) => "ivec2",
            Self::IVec(3) => "ivec3",
            Self::IVec(4) => "ivec4",
            Self::Mat(2) => "mat2",
            Self::Mat(3) => "mat3",
            Self::Mat(4) => "mat4",
            Self::Sampler2D => "sampler2D",
            _ => "unknown",
        }
    }

    pub(super) fn is_runtime_uniform(self) -> bool {
        matches!(
            self,
            Self::Float | Self::Vec(2) | Self::Vec(3) | Self::Vec(4)
        )
    }

    pub(super) fn is_function_return(self) -> bool {
        !matches!(self, Self::Sampler2D)
    }

    pub(super) fn is_function_parameter(self) -> bool {
        !matches!(self, Self::Void | Self::Sampler2D)
    }

    pub(super) fn is_local_value(self) -> bool {
        !matches!(self, Self::Void | Self::Sampler2D)
    }

    pub(super) fn is_scalar_numeric(self) -> bool {
        matches!(self, Self::Int | Self::Float)
    }

    pub(super) fn is_float_vector(self) -> bool {
        matches!(self, Self::Vec(_))
    }

    pub(super) fn is_numeric(self) -> bool {
        matches!(self, Self::Int | Self::Float | Self::Vec(_) | Self::Mat(_))
    }
}

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
    expressions: Vec<IrExpression>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct IrUniform<'src> {
    pub(super) ty: &'src str,
    pub(super) value_type: Option<IrType>,
    pub(super) name: &'src str,
    pub(super) ty_span: SourceSpan,
    pub(super) name_span: SourceSpan,
    pub(super) declaration_span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct IrFunction<'src> {
    pub(super) name: &'src str,
    pub(super) return_type: &'src str,
    pub(super) return_value_type: Option<IrType>,
    pub(super) name_span: SourceSpan,
    pub(super) declaration_span: SourceSpan,
    pub(super) params: Vec<IrFunctionParam<'src>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct IrFunctionParam<'src> {
    pub(super) ty: &'src str,
    pub(super) value_type: Option<IrType>,
    pub(super) name: &'src str,
    pub(super) ty_span: SourceSpan,
    pub(super) name_span: SourceSpan,
    pub(super) declaration_span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct IrExpression {
    pub(super) value_type: Option<IrType>,
    pub(super) span: SourceSpan,
}

pub(super) fn lower<'module, 'src>(
    module: &'module UsagiShaderModule<'src>,
    checked: Option<&super::check::CheckedShader>,
) -> ShaderIr<'module, 'src> {
    ShaderIr {
        module,
        source: opt::optimized_source(module),
        uniforms: lower_uniforms(module),
        functions: lower_functions(module),
        expressions: lower_expressions(module, checked),
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

    pub(super) fn expressions(&self) -> &[IrExpression] {
        &self.expressions
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
            value_type: IrType::parse(uniform.ty),
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
                return_value_type: IrType::parse(function.return_type),
                name_span: function.name_span.shifted(module.source_offset),
                declaration_span: function.span.shifted(module.source_offset),
                params: function
                    .params
                    .iter()
                    .map(|param| IrFunctionParam {
                        ty: param.ty,
                        value_type: IrType::parse(param.ty),
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

fn lower_expressions(
    module: &UsagiShaderModule<'_>,
    checked: Option<&super::check::CheckedShader>,
) -> Vec<IrExpression> {
    let Some(checked) = checked else {
        return Vec::new();
    };
    checked
        .expressions()
        .iter()
        .map(|expression| IrExpression {
            value_type: expression.value_type,
            span: expression.span.shifted(module.source_offset),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::check;
    use super::super::syntax::UsagiShaderModule;
    use super::*;
    use crate::shader::ShaderProfile;

    #[test]
    fn lower_preserves_uniform_summaries_with_absolute_spans() {
        let src = "#usagi shader 1\n\nuniform float u_time;\nuniform vec2 u_resolution, u_origin;\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n";
        let module = UsagiShaderModule::parse_with_diagnostic(src).unwrap();
        let ir = lower(&module, None);

        assert_eq!(ir.uniforms().len(), 3);
        assert_eq!(ir.uniforms()[0].ty, "float");
        assert_eq!(ir.uniforms()[0].value_type, Some(IrType::Float));
        assert_eq!(ir.uniforms()[0].name, "u_time");
        assert_eq!(ir.uniforms()[1].ty, "vec2");
        assert_eq!(ir.uniforms()[1].value_type, Some(IrType::Vec(2)));
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
        let ir = lower(&module, None);

        assert_eq!(ir.functions().len(), 2);
        assert_eq!(ir.functions()[0].name, "helper");
        assert_eq!(ir.functions()[0].return_type, "vec4");
        assert_eq!(ir.functions()[0].return_value_type, Some(IrType::Vec(4)));
        assert_eq!(ir.functions()[0].params.len(), 1);
        assert_eq!(ir.functions()[0].params[0].ty, "vec2");
        assert_eq!(ir.functions()[0].params[0].value_type, Some(IrType::Vec(2)));
        assert_eq!(ir.functions()[0].params[0].name, "uv");
        assert_eq!(ir.functions()[1].name, "usagi_main");
        assert!(ir.source().rewrites.iter().any(|rewrite| matches!(
            rewrite.kind,
            super::super::syntax::SourceRewriteKind::Replacement(_)
        )));
    }

    #[test]
    fn lower_preserves_checked_expression_types_with_absolute_spans() {
        let src = concat!(
            "#usagi shader 1\n\n",
            "vec4 usagi_main(vec2 uv, vec4 color) {\n",
            "    if (uv.x > 0.5) return color;\n",
            "    return vec4(uv, 0.0, 1.0);\n",
            "}\n",
        );
        let module = UsagiShaderModule::parse_with_diagnostic(src).unwrap();
        let checked = check::analyze(&module, ShaderProfile::DesktopGlsl330).unwrap();
        let ir = lower(&module, Some(&checked));

        assert!(ir.expressions().iter().any(|expression| {
            expression.value_type == Some(IrType::Bool)
                && &src[expression.span.start..expression.span.end] == "uv.x > 0.5"
        }));
        assert!(ir.expressions().iter().any(|expression| {
            expression.value_type == Some(IrType::Vec(4))
                && &src[expression.span.start..expression.span.end] == "vec4(uv, 0.0, 1.0)"
        }));
    }
}
