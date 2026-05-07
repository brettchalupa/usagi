//! Backend-neutral shader optimization passes.
//!
//! The passes are deliberately conservative. They fold only exact numeric
//! literal binary expressions and prune only statements after a syntactically
//! guaranteed return. Anything involving symbols, calls, or uncertain numeric
//! syntax is left for the GLSL driver.

use super::ir::{IrBlock, IrBranch, IrFunction, IrStatement, IrStatementKind};
use super::syntax::{ShaderSource, SourceRewrite, SourceSpan, Token, UsagiShaderModule};

pub(super) fn optimized_source(
    module: &UsagiShaderModule<'_>,
    functions: &[IrFunction<'_>],
    mut rewrites: Vec<SourceRewrite>,
) -> ShaderSource {
    DeadCodePruner::new(module, functions).collect(&mut rewrites);
    module.source.with_additional_rewrites(rewrites)
}

struct DeadCodePruner<'module, 'ir, 'src> {
    module: &'module UsagiShaderModule<'src>,
    functions: &'ir [IrFunction<'src>],
}

impl<'module, 'ir, 'src> DeadCodePruner<'module, 'ir, 'src> {
    fn new(module: &'module UsagiShaderModule<'src>, functions: &'ir [IrFunction<'src>]) -> Self {
        Self { module, functions }
    }

    fn collect(&self, out: &mut Vec<SourceRewrite>) {
        for function in self.functions {
            self.collect_block(&function.body, out);
        }
    }

    fn collect_block(&self, block: &IrBlock, out: &mut Vec<SourceRewrite>) {
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

    fn collect_statement(&self, statement: &IrStatement, out: &mut Vec<SourceRewrite>) {
        match &statement.kind {
            IrStatementKind::If {
                then_branch,
                else_branch,
            } => {
                self.collect_branch(then_branch, out);
                if let Some(branch) = else_branch {
                    self.collect_branch(branch, out);
                }
            }
            IrStatementKind::Block(block) => self.collect_block(block, out),
            IrStatementKind::Return | IrStatementKind::Raw => {}
        }
    }

    fn collect_branch(&self, branch: &IrBranch, out: &mut Vec<SourceRewrite>) {
        match branch {
            IrBranch::Block(block) => self.collect_block(block, out),
            IrBranch::Statement(statement) => self.collect_statement(statement, out),
        }
    }

    fn prune_statement(&self, statement: &IrStatement) -> Option<SourceRewrite> {
        let replacement =
            line_preserving_blank(&self.module.tokens[statement.start_idx..statement.end_idx]);
        Some(SourceRewrite::replacement(
            statement.start_idx,
            statement.end_idx,
            relative_span(statement.span, self.module.source_offset),
            replacement,
        ))
    }
}

fn statement_always_returns(statement: &IrStatement) -> bool {
    match &statement.kind {
        IrStatementKind::Return => true,
        IrStatementKind::If {
            then_branch,
            else_branch,
        } => {
            let Some(else_branch) = else_branch else {
                return false;
            };
            branch_always_returns(then_branch) && branch_always_returns(else_branch)
        }
        IrStatementKind::Block(block) => block_always_returns(block),
        IrStatementKind::Raw => false,
    }
}

fn block_always_returns(block: &IrBlock) -> bool {
    block.statements.iter().any(statement_always_returns)
}

fn branch_always_returns(branch: &IrBranch) -> bool {
    match branch {
        IrBranch::Block(block) => block_always_returns(block),
        IrBranch::Statement(statement) => statement_always_returns(statement),
    }
}

fn relative_span(span: SourceSpan, source_offset: usize) -> SourceSpan {
    SourceSpan {
        start: span.start.saturating_sub(source_offset),
        end: span.end.saturating_sub(source_offset),
    }
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
