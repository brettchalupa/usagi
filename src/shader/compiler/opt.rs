//! Backend-neutral shader optimization passes.
//!
//! The passes are deliberately conservative. They fold only exact numeric
//! literal binary expressions and prune only statements after a syntactically
//! guaranteed return. Anything involving symbols, calls, or uncertain numeric
//! syntax is left for the GLSL driver.

use super::syntax::{
    BlockAst, BranchAst, ShaderItem, ShaderSource, SourceRewrite, SourceSpan, StatementAst, Token,
    UsagiShaderModule,
};

pub(super) fn optimized_source(
    module: &UsagiShaderModule<'_>,
    mut rewrites: Vec<SourceRewrite>,
) -> ShaderSource {
    DeadCodePruner::new(module).collect(&mut rewrites);
    module.source.with_additional_rewrites(rewrites)
}

struct DeadCodePruner<'module, 'src> {
    module: &'module UsagiShaderModule<'src>,
}

impl<'module, 'src> DeadCodePruner<'module, 'src> {
    fn new(module: &'module UsagiShaderModule<'src>) -> Self {
        Self { module }
    }

    fn collect(&self, out: &mut Vec<SourceRewrite>) {
        for item in &self.module.items {
            match item {
                ShaderItem::Function(function) => self.collect_block(&function.body, out),
                ShaderItem::Raw(_) | ShaderItem::Uniform(_) => {}
            }
        }
    }

    fn collect_block(&self, block: &BlockAst<'src>, out: &mut Vec<SourceRewrite>) {
        let mut reached_terminal = false;
        for statement in &block.statements {
            if reached_terminal {
                if let Some(rewrite) = self.prune_statement(statement) {
                    out.push(rewrite);
                }
                continue;
            }

            self.collect_statement(statement, out);
            if statement_always_returns(statement) {
                reached_terminal = true;
            }
        }
    }

    fn collect_statement(&self, statement: &StatementAst<'src>, out: &mut Vec<SourceRewrite>) {
        match statement {
            StatementAst::If(stmt) => {
                self.collect_branch(&stmt.then_branch, out);
                if let Some(branch) = &stmt.else_branch {
                    self.collect_branch(branch, out);
                }
            }
            StatementAst::Block(block) => self.collect_block(block, out),
            StatementAst::Return(_) | StatementAst::Raw(_) => {}
        }
    }

    fn collect_branch(&self, branch: &BranchAst<'src>, out: &mut Vec<SourceRewrite>) {
        match branch {
            BranchAst::Block(block) => self.collect_block(block, out),
            BranchAst::Statement(statement) => self.collect_statement(statement, out),
        }
    }

    fn prune_statement(&self, statement: &StatementAst<'src>) -> Option<SourceRewrite> {
        let span = statement_span(statement);
        let (start_idx, end_idx) = token_range_for_span(&self.module.tokens, span)?;
        let replacement = line_preserving_blank(&self.module.tokens[start_idx..end_idx]);
        Some(SourceRewrite::replacement(
            start_idx,
            end_idx,
            span,
            replacement,
        ))
    }
}

fn statement_always_returns(statement: &StatementAst<'_>) -> bool {
    match statement {
        StatementAst::Return(_) => true,
        StatementAst::If(stmt) => {
            let Some(else_branch) = &stmt.else_branch else {
                return false;
            };
            branch_always_returns(&stmt.then_branch) && branch_always_returns(else_branch)
        }
        StatementAst::Block(block) => block_always_returns(block),
        StatementAst::Raw(_) => false,
    }
}

fn block_always_returns(block: &BlockAst<'_>) -> bool {
    block.statements.iter().any(statement_always_returns)
}

fn branch_always_returns(branch: &BranchAst<'_>) -> bool {
    match branch {
        BranchAst::Block(block) => block_always_returns(block),
        BranchAst::Statement(statement) => statement_always_returns(statement),
    }
}

fn statement_span(statement: &StatementAst<'_>) -> SourceSpan {
    match statement {
        StatementAst::Return(stmt) => stmt.span,
        StatementAst::If(stmt) => stmt.span,
        StatementAst::Block(block) => block.span,
        StatementAst::Raw(stmt) => stmt.span,
    }
}

fn token_range_for_span(tokens: &[Token<'_>], span: SourceSpan) -> Option<(usize, usize)> {
    let start_idx = tokens
        .iter()
        .position(|token| token.span.end > span.start)?;
    let end_idx = tokens
        .iter()
        .rposition(|token| token.span.start < span.end)?
        + 1;
    (start_idx < end_idx).then_some((start_idx, end_idx))
}

fn line_preserving_blank(tokens: &[Token<'_>]) -> String {
    let newline_count = tokens
        .iter()
        .map(|token| token.text.bytes().filter(|byte| *byte == b'\n').count())
        .sum();
    "\n".repeat(newline_count)
}

#[cfg(test)]
mod tests {
    use super::super::ir;
    use super::super::syntax::UsagiShaderModule;
    use crate::shader::ShaderProfile;

    fn optimized_body(src: &str) -> String {
        let module = UsagiShaderModule::parse(src).unwrap();
        let ir = ir::lower(&module, None);
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

    #[test]
    fn prunes_statements_after_guaranteed_return() {
        let src = "vec4 usagi_main(vec2 uv, vec4 color) {\n    return color;\n    vec4 dead = vec4(1.0);\n    return dead;\n}\n";
        let out = optimized_body(src);

        assert!(out.contains("return color;"));
        assert!(!out.contains("vec4 dead"));
        assert!(!out.contains("return dead"));
        let module = UsagiShaderModule::parse(src).unwrap();
        let ir = ir::lower(&module, None);
        let emission = super::super::emit_glsl::emit(&ir, ShaderProfile::DesktopGlsl330).unwrap();
        assert_eq!(
            emission.source_map.original_source_line_range(),
            Some((1, src.lines().count()))
        );
    }

    #[test]
    fn prunes_after_if_else_when_both_branches_return() {
        let out = optimized_body(
            "vec4 usagi_main(vec2 uv, vec4 color) {\n    if (uv.x > 0.5) return color;\n    else return vec4(0.0, 0.0, 0.0, 1.0);\n    vec4 dead = vec4(1.0);\n    return dead;\n}\n",
        );

        assert!(out.contains("if (uv.x > 0.5) return color;"));
        assert!(!out.contains("vec4 dead"));
        assert!(!out.contains("return dead"));
    }

    #[test]
    fn does_not_prune_after_non_terminal_if() {
        let out = optimized_body(
            "vec4 usagi_main(vec2 uv, vec4 color) {\n    if (uv.x > 0.5) return color;\n    vec4 live = vec4(1.0);\n    return live;\n}\n",
        );

        assert!(out.contains("vec4 live"));
        assert!(out.contains("return live"));
    }
}
