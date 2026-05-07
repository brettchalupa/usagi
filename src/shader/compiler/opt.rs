//! Backend-neutral shader optimization passes.
//!
//! The first pass is deliberately conservative: it folds only exact numeric
//! literal binary expressions. Anything involving symbols, calls, or uncertain
//! numeric syntax is left for the GLSL driver.

use super::syntax::{
    BlockAst, BranchAst, ExpressionAst, ExpressionNode, ShaderItem, ShaderSource, SourceRewrite,
    StatementAst, Token, TokenKind, UsagiShaderModule, is_code_token,
};

pub(super) fn optimized_source(module: &UsagiShaderModule<'_>) -> ShaderSource {
    let mut folds = Vec::new();
    ConstantFolder::new(module).collect(&mut folds);
    module.source.with_additional_rewrites(folds)
}

struct ConstantFolder<'module, 'src> {
    module: &'module UsagiShaderModule<'src>,
}

impl<'module, 'src> ConstantFolder<'module, 'src> {
    fn new(module: &'module UsagiShaderModule<'src>) -> Self {
        Self { module }
    }

    fn collect(&self, out: &mut Vec<SourceRewrite>) {
        for item in &self.module.items {
            match item {
                ShaderItem::Function(function) => self.collect_block(&function.body, out),
                ShaderItem::Raw(raw) => self.collect_expression(&raw.expression, out),
                ShaderItem::Uniform(_) => {}
            }
        }
    }

    fn collect_block(&self, block: &BlockAst<'src>, out: &mut Vec<SourceRewrite>) {
        for statement in &block.statements {
            self.collect_statement(statement, out);
        }
    }

    fn collect_statement(&self, statement: &StatementAst<'src>, out: &mut Vec<SourceRewrite>) {
        match statement {
            StatementAst::Return(stmt) => {
                if let Some(expression) = &stmt.expression {
                    self.collect_expression(expression, out);
                }
            }
            StatementAst::If(stmt) => {
                self.collect_expression(&stmt.condition, out);
                self.collect_branch(&stmt.then_branch, out);
                if let Some(branch) = &stmt.else_branch {
                    self.collect_branch(branch, out);
                }
            }
            StatementAst::Block(block) => self.collect_block(block, out),
            StatementAst::Raw(stmt) => self.collect_expression(&stmt.expression, out),
        }
    }

    fn collect_branch(&self, branch: &BranchAst<'src>, out: &mut Vec<SourceRewrite>) {
        match branch {
            BranchAst::Block(block) => self.collect_block(block, out),
            BranchAst::Statement(statement) => self.collect_statement(statement, out),
        }
    }

    fn collect_expression(&self, expression: &ExpressionAst<'src>, out: &mut Vec<SourceRewrite>) {
        for node in &expression.nodes {
            let ExpressionNode::Call(call) = node else {
                continue;
            };
            for arg in &call.args {
                self.collect_expression(arg, out);
            }
        }

        if let Some(fold) = fold_numeric_expression(&self.module.tokens, expression) {
            out.push(SourceRewrite::replacement(
                fold.start_idx,
                fold.end_idx,
                fold.span,
                fold.replacement,
            ));
        }
    }
}

struct ConstantFold {
    start_idx: usize,
    end_idx: usize,
    span: super::syntax::SourceSpan,
    replacement: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ConstantValue {
    Int(i32),
    Float(f64),
}

fn fold_numeric_expression(
    tokens: &[Token<'_>],
    expression: &ExpressionAst<'_>,
) -> Option<ConstantFold> {
    let code: Vec<_> = expression
        .nodes
        .iter()
        .filter_map(|node| match node {
            ExpressionNode::Token(idx) if is_code_token(&tokens[*idx]) => Some(*idx),
            ExpressionNode::Token(_) | ExpressionNode::Call(_) => None,
        })
        .collect();
    if code.len() < 3 {
        return None;
    }

    let code = strip_outer_parentheses(tokens, &code);
    let (lhs, next) = parse_constant_value(tokens, code, 0)?;
    let op_idx = *code.get(next)?;
    let op = tokens[op_idx].text;
    let (rhs, next) = parse_constant_value(tokens, code, next + 1)?;
    if next != code.len() {
        return None;
    }

    let value = fold_binary(lhs, op, rhs)?;
    let replacement = format_constant_value(value);
    let start_idx = code[0];
    let end_idx = code[code.len() - 1] + 1;
    Some(ConstantFold {
        start_idx,
        end_idx,
        span: expression.span,
        replacement,
    })
}

fn strip_outer_parentheses<'a>(tokens: &[Token<'_>], code: &'a [usize]) -> &'a [usize] {
    if code.len() >= 5
        && tokens[code[0]].text == "("
        && tokens[*code.last().expect("len checked")].text == ")"
    {
        &code[1..code.len() - 1]
    } else {
        code
    }
}

fn parse_constant_value(
    tokens: &[Token<'_>],
    code: &[usize],
    start: usize,
) -> Option<(ConstantValue, usize)> {
    let mut sign = 1.0f64;
    let mut idx = start;
    if let Some(token_idx) = code.get(idx)
        && matches!(tokens[*token_idx].text, "+" | "-")
    {
        if tokens[*token_idx].text == "-" {
            sign = -1.0;
        }
        idx += 1;
    }

    let token_idx = *code.get(idx)?;
    let token = &tokens[token_idx];
    if token.kind != TokenKind::Number {
        return None;
    }
    let value = parse_number(token.text, sign)?;
    Some((value, idx + 1))
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

#[cfg(test)]
mod tests {
    use super::super::ir;
    use super::super::syntax::UsagiShaderModule;
    use crate::shader::ShaderProfile;

    fn optimized_body(src: &str) -> String {
        let module = UsagiShaderModule::parse(src).unwrap();
        let ir = ir::lower(&module);
        let mut out = String::new();
        super::super::emit_glsl::emit(&ir, ShaderProfile::DesktopGlsl330)
            .unwrap()
            .source
            .lines()
            .filter(|line| {
                !line.starts_with("#version")
                    && !line.starts_with("in ")
                    && !line.starts_with("uniform sampler2D")
                    && !line.starts_with("out ")
            })
            .for_each(|line| {
                out.push_str(line);
                out.push('\n');
            });
        out
    }

    #[test]
    fn folds_numeric_literal_constructor_arguments() {
        let out = optimized_body(
            "vec4 usagi_main(vec2 uv, vec4 color) {\n    return vec4(0.2 + 0.3, 1.0 * 0.5, 4.0 / 2.0, 2 - 1);\n}\n",
        );

        assert!(out.contains("return vec4(0.5, 0.5, 2.0, 1);"));
    }

    #[test]
    fn leaves_symbols_calls_and_zero_division_unfolded() {
        let out = optimized_body(
            "vec4 helper(vec2 uv) { return vec4(uv, 0.0, 1.0); }\nvec4 usagi_main(vec2 uv, vec4 color) {\n    return helper(uv) + vec4(1.0 / 0.0, color.g, 2.0 + color.b, 1.0);\n}\n",
        );

        assert!(out.contains("1.0 / 0.0"));
        assert!(out.contains("2.0 + color.b"));
    }
}
