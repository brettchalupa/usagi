use super::CompileWarning;
use super::opt;
use super::syntax::{
    ExprCall, ExprCallKind, ExpressionAst, ExpressionNode, IntrinsicKind, ShaderItem, ShaderSource,
    SourceRewrite, SourceSpan, Token, TokenKind, UsagiShaderModule, is_code_token,
};
use std::collections::HashMap;

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
    expressions: Vec<IrExpression<'src>>,
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
pub(super) struct IrExpression<'src> {
    pub(super) value_type: Option<IrType>,
    pub(super) span: SourceSpan,
    nodes: Vec<IrExpressionNode<'src>>,
}

impl IrExpression<'_> {
    pub(super) fn is_well_formed(&self) -> bool {
        self.span.start <= self.span.end
            && self
                .value_type
                .is_none_or(|value_type| !value_type.as_str().is_empty())
            && self.nodes.iter().all(IrExpressionNode::is_well_formed)
    }

    #[cfg(test)]
    fn nodes(&self) -> &[IrExpressionNode<'_>] {
        &self.nodes
    }

    fn normalized_key(&self) -> String {
        let mut out = String::new();
        self.write_normalized_key(&mut out);
        out
    }

    fn write_normalized_key(&self, out: &mut String) {
        for node in &self.nodes {
            node.write_normalized_key(out);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum IrExpressionNode<'src> {
    Token(IrToken<'src>),
    Call(IrCall<'src>),
}

impl IrExpressionNode<'_> {
    fn is_well_formed(&self) -> bool {
        match self {
            Self::Token(token) => token.is_well_formed(),
            Self::Call(call) => call.is_well_formed(),
        }
    }

    fn write_normalized_key(&self, out: &mut String) {
        match self {
            Self::Token(token) => token.write_normalized_key(out),
            Self::Call(call) => call.write_normalized_key(out),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct IrToken<'src> {
    token_idx: usize,
    text: &'src str,
    span: SourceSpan,
    is_code: bool,
    is_number: bool,
}

impl IrToken<'_> {
    fn is_well_formed(&self) -> bool {
        self.span.start <= self.span.end && (!self.is_code || !self.text.trim().is_empty())
    }

    fn write_normalized_key(&self, out: &mut String) {
        if self.is_code {
            out.push_str(self.text);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct IrCall<'src> {
    kind: IrCallKind,
    name: &'src str,
    name_span: SourceSpan,
    span: SourceSpan,
    args: Vec<IrExpression<'src>>,
}

impl IrCall<'_> {
    fn is_well_formed(&self) -> bool {
        self.name_span.start <= self.name_span.end
            && self.span.start <= self.span.end
            && self.span.start <= self.name_span.start
            && self.name_span.end <= self.span.end
            && !self.name.is_empty()
            && match self.kind {
                IrCallKind::Generic => true,
                IrCallKind::TextureIntrinsic => self.name == "usagi_texture",
            }
            && self.args.iter().all(IrExpression::is_well_formed)
    }

    fn write_normalized_key(&self, out: &mut String) {
        out.push_str(self.name);
        out.push('(');
        for (idx, arg) in self.args.iter().enumerate() {
            if idx > 0 {
                out.push(',');
            }
            arg.write_normalized_key(out);
        }
        out.push(')');
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IrCallKind {
    Generic,
    TextureIntrinsic,
}

pub(super) fn lower<'module, 'src>(
    module: &'module UsagiShaderModule<'src>,
    checked: Option<&super::check::CheckedShader>,
) -> ShaderIr<'module, 'src> {
    let expressions = lower_expressions(module, checked);
    let constant_rewrites = collect_constant_fold_rewrites(&expressions, module.source_offset);
    ShaderIr {
        module,
        source: opt::optimized_source(module, constant_rewrites),
        uniforms: lower_uniforms(module),
        functions: lower_functions(module),
        expressions,
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

    pub(super) fn expressions(&self) -> &[IrExpression<'src>] {
        &self.expressions
    }

    pub(super) fn duplicate_texture_sample_warnings(&self) -> Vec<CompileWarning> {
        DuplicateTextureSampleAnalyzer::new(self).collect()
    }
}

struct DuplicateTextureSampleAnalyzer<'ir, 'module, 'src> {
    ir: &'ir ShaderIr<'module, 'src>,
    seen: HashMap<String, SourceSpan>,
    warnings: Vec<CompileWarning>,
}

impl<'ir, 'module, 'src> DuplicateTextureSampleAnalyzer<'ir, 'module, 'src> {
    fn new(ir: &'ir ShaderIr<'module, 'src>) -> Self {
        Self {
            ir,
            seen: HashMap::new(),
            warnings: Vec::new(),
        }
    }

    fn collect(mut self) -> Vec<CompileWarning> {
        for expression in self.ir.expressions() {
            self.visit_expression(expression);
        }
        self.warnings
    }

    fn visit_expression(&mut self, expression: &IrExpression<'src>) {
        for node in &expression.nodes {
            let IrExpressionNode::Call(call) = node else {
                continue;
            };
            if call.kind == IrCallKind::TextureIntrinsic {
                self.record_texture_sample(call);
            }
            for arg in &call.args {
                self.visit_expression(arg);
            }
        }
    }

    fn record_texture_sample(&mut self, call: &IrCall<'src>) {
        let Some(uv) = call.args.get(1) else {
            return;
        };
        let key = uv.normalized_key();
        if self.seen.contains_key(&key) {
            self.warnings.push(CompileWarning::at(
                format!(
                    "duplicate usagi_texture(texture0, {key}) sample; reuse the first sample when possible"
                ),
                self.relative_span(call.span),
            ));
            return;
        }
        self.seen.insert(key, self.relative_span(call.span));
    }

    fn relative_span(&self, span: SourceSpan) -> SourceSpan {
        SourceSpan {
            start: span.start.saturating_sub(self.ir.module.source_offset),
            end: span.end.saturating_sub(self.ir.module.source_offset),
        }
    }
}

fn collect_constant_fold_rewrites(
    expressions: &[IrExpression<'_>],
    source_offset: usize,
) -> Vec<SourceRewrite> {
    let mut rewrites = Vec::new();
    for expression in expressions {
        collect_expression_constant_folds(expression, source_offset, &mut rewrites);
    }
    rewrites
}

fn collect_expression_constant_folds(
    expression: &IrExpression<'_>,
    source_offset: usize,
    out: &mut Vec<SourceRewrite>,
) {
    for node in &expression.nodes {
        let IrExpressionNode::Call(call) = node else {
            continue;
        };
        for arg in &call.args {
            collect_expression_constant_folds(arg, source_offset, out);
        }
    }

    if let Some(rewrite) = fold_numeric_expression(expression, source_offset) {
        out.push(rewrite);
    }
}

fn fold_numeric_expression(
    expression: &IrExpression<'_>,
    source_offset: usize,
) -> Option<SourceRewrite> {
    let code: Vec<_> = expression
        .nodes
        .iter()
        .filter_map(|node| match node {
            IrExpressionNode::Token(token) if token.is_code => Some(token),
            IrExpressionNode::Token(_) | IrExpressionNode::Call(_) => None,
        })
        .collect();
    if code.len() < 3 {
        return None;
    }

    let code = strip_outer_parentheses(&code);
    let (lhs, next) = parse_constant_value(code, 0)?;
    let op = code.get(next)?.text;
    let (rhs, next) = parse_constant_value(code, next + 1)?;
    if next != code.len() {
        return None;
    }

    let value = fold_binary(lhs, op, rhs)?;
    let replacement = format_constant_value(value);
    let start_idx = code[0].token_idx;
    let end_idx = code[code.len() - 1].token_idx + 1;
    Some(SourceRewrite::replacement(
        start_idx,
        end_idx,
        relative_span(expression.span, source_offset),
        replacement,
    ))
}

fn strip_outer_parentheses<'a>(code: &'a [&IrToken<'_>]) -> &'a [&'a IrToken<'a>] {
    if code.len() >= 5 && code[0].text == "(" && code.last().expect("len checked").text == ")" {
        &code[1..code.len() - 1]
    } else {
        code
    }
}

fn parse_constant_value(code: &[&IrToken<'_>], start: usize) -> Option<(ConstantValue, usize)> {
    let mut sign = 1.0f64;
    let mut idx = start;
    if let Some(token) = code.get(idx)
        && matches!(token.text, "+" | "-")
    {
        if token.text == "-" {
            sign = -1.0;
        }
        idx += 1;
    }

    let token = code.get(idx)?;
    if !token.is_number {
        return None;
    }
    let value = parse_number(token.text, sign)?;
    Some((value, idx + 1))
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ConstantValue {
    Int(i32),
    Float(f64),
}

fn parse_number(text: &str, sign: f64) -> Option<ConstantValue> {
    if text.contains('.') || text.contains('e') || text.contains('E') {
        let value = text.parse::<f64>().ok()? * sign;
        if value.is_finite() {
            Some(ConstantValue::Float(value))
        } else {
            None
        }
    } else {
        let parsed = text.parse::<i64>().ok()?;
        let signed = if sign.is_sign_negative() {
            parsed.checked_neg()?
        } else {
            parsed
        };
        i32::try_from(signed).ok().map(ConstantValue::Int)
    }
}

fn fold_binary(lhs: ConstantValue, op: &str, rhs: ConstantValue) -> Option<ConstantValue> {
    match (lhs, rhs) {
        (ConstantValue::Int(a), ConstantValue::Int(b)) => fold_int_binary(a, op, b),
        _ => fold_float_binary(lhs.as_f64(), op, rhs.as_f64()).map(ConstantValue::Float),
    }
}

fn fold_int_binary(a: i32, op: &str, b: i32) -> Option<ConstantValue> {
    let value = match op {
        "+" => a.checked_add(b)?,
        "-" => a.checked_sub(b)?,
        "*" => a.checked_mul(b)?,
        "/" if b != 0 => a.checked_div(b)?,
        _ => return None,
    };
    Some(ConstantValue::Int(value))
}

fn fold_float_binary(a: f64, op: &str, b: f64) -> Option<f64> {
    let value = match op {
        "+" => a + b,
        "-" => a - b,
        "*" => a * b,
        "/" if b != 0.0 => a / b,
        _ => return None,
    };
    value.is_finite().then_some(value)
}

impl ConstantValue {
    fn as_f64(self) -> f64 {
        match self {
            Self::Int(value) => f64::from(value),
            Self::Float(value) => value,
        }
    }
}

fn format_constant_value(value: ConstantValue) -> String {
    match value {
        ConstantValue::Int(value) => value.to_string(),
        ConstantValue::Float(value) => format_float(value),
    }
}

fn format_float(value: f64) -> String {
    if value == 0.0 {
        return "0.0".to_string();
    }
    let mut text = value.to_string();
    if !text.contains('.') && !text.contains('e') && !text.contains('E') {
        text.push_str(".0");
    }
    text
}

fn relative_span(span: SourceSpan, source_offset: usize) -> SourceSpan {
    SourceSpan {
        start: span.start.saturating_sub(source_offset),
        end: span.end.saturating_sub(source_offset),
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

fn lower_expressions<'src>(
    module: &UsagiShaderModule<'src>,
    checked: Option<&super::check::CheckedShader>,
) -> Vec<IrExpression<'src>> {
    let mut expressions = Vec::new();
    for item in &module.items {
        match item {
            ShaderItem::Function(function) => collect_block_expressions(
                &function.body.statements,
                module,
                checked,
                &mut expressions,
            ),
            ShaderItem::Raw(raw) => {
                expressions.push(lower_expression(&raw.expression, module, checked));
            }
            ShaderItem::Uniform(_) => {}
        }
    }
    expressions
}

fn collect_block_expressions<'src>(
    statements: &[super::syntax::StatementAst<'src>],
    module: &UsagiShaderModule<'src>,
    checked: Option<&super::check::CheckedShader>,
    out: &mut Vec<IrExpression<'src>>,
) {
    for statement in statements {
        match statement {
            super::syntax::StatementAst::Return(stmt) => {
                if let Some(expression) = &stmt.expression {
                    out.push(lower_expression(expression, module, checked));
                }
            }
            super::syntax::StatementAst::If(stmt) => {
                out.push(lower_expression(&stmt.condition, module, checked));
                collect_branch_expressions(&stmt.then_branch, module, checked, out);
                if let Some(branch) = &stmt.else_branch {
                    collect_branch_expressions(branch, module, checked, out);
                }
            }
            super::syntax::StatementAst::Block(block) => {
                collect_block_expressions(&block.statements, module, checked, out);
            }
            super::syntax::StatementAst::Raw(stmt) => {
                out.push(lower_expression(&stmt.expression, module, checked));
            }
        }
    }
}

fn collect_branch_expressions<'src>(
    branch: &super::syntax::BranchAst<'src>,
    module: &UsagiShaderModule<'src>,
    checked: Option<&super::check::CheckedShader>,
    out: &mut Vec<IrExpression<'src>>,
) {
    match branch {
        super::syntax::BranchAst::Block(block) => {
            collect_block_expressions(&block.statements, module, checked, out);
        }
        super::syntax::BranchAst::Statement(statement) => {
            collect_block_expressions(std::slice::from_ref(statement), module, checked, out);
        }
    }
}

fn lower_expression<'src>(
    expression: &ExpressionAst<'src>,
    module: &UsagiShaderModule<'src>,
    checked: Option<&super::check::CheckedShader>,
) -> IrExpression<'src> {
    IrExpression {
        value_type: checked_type_for_span(checked, expression.span),
        span: expression.span.shifted(module.source_offset),
        nodes: expression
            .nodes
            .iter()
            .map(|node| lower_expression_node(node, module, checked))
            .collect(),
    }
}

fn lower_expression_node<'src>(
    node: &ExpressionNode<'src>,
    module: &UsagiShaderModule<'src>,
    checked: Option<&super::check::CheckedShader>,
) -> IrExpressionNode<'src> {
    match node {
        ExpressionNode::Token(idx) => IrExpressionNode::Token(lower_token(
            *idx,
            &module.tokens[*idx],
            module.source_offset,
        )),
        ExpressionNode::Call(call) => IrExpressionNode::Call(lower_call(call, module, checked)),
    }
}

fn lower_token<'src>(token_idx: usize, token: &Token<'src>, source_offset: usize) -> IrToken<'src> {
    IrToken {
        token_idx,
        text: token.text,
        span: token.span.shifted(source_offset),
        is_code: is_code_token(token),
        is_number: token.kind == TokenKind::Number,
    }
}

fn lower_call<'src>(
    call: &ExprCall<'src>,
    module: &UsagiShaderModule<'src>,
    checked: Option<&super::check::CheckedShader>,
) -> IrCall<'src> {
    IrCall {
        kind: match call.kind {
            ExprCallKind::Generic => IrCallKind::Generic,
            ExprCallKind::Intrinsic(IntrinsicKind::Texture) => IrCallKind::TextureIntrinsic,
        },
        name: call.name,
        name_span: module.tokens[call.name_idx]
            .span
            .shifted(module.source_offset),
        span: call.span.shifted(module.source_offset),
        args: call
            .args
            .iter()
            .map(|arg| lower_expression(arg, module, checked))
            .collect(),
    }
}

fn checked_type_for_span(
    checked: Option<&super::check::CheckedShader>,
    span: SourceSpan,
) -> Option<IrType> {
    checked?
        .expressions()
        .iter()
        .find(|expression| expression.span == span)
        .and_then(|expression| expression.value_type)
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
        let constructor = ir
            .expressions()
            .iter()
            .find(|expression| {
                expression.value_type == Some(IrType::Vec(4))
                    && &src[expression.span.start..expression.span.end] == "vec4(uv, 0.0, 1.0)"
            })
            .unwrap();
        let IrExpressionNode::Call(call) = &constructor.nodes()[0] else {
            panic!("expected constructor call IR node");
        };
        assert_eq!(call.kind, IrCallKind::Generic);
        assert_eq!(call.name, "vec4");
        assert_eq!(call.args.len(), 3);
        assert_eq!(call.args[0].value_type, Some(IrType::Vec(2)));
    }
}
