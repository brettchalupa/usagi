use std::collections::{HashMap, HashSet};

use super::ir::IrType as ShaderType;
use super::syntax::{
    BlockAst, BranchAst, ExprCall, ExprCallKind, ExpressionAst, ExpressionNode, FunctionDecl,
    IntrinsicKind, RawItem, RawStmt, ReturnStmt, ShaderItem, StatementAst, Token, TokenKind,
    UniformDecl, UsagiShaderModule, is_code_token,
};
use super::{CompileError, CompileResult, ShaderProfile};

#[cfg(test)]
pub(super) fn validate(
    module: &UsagiShaderModule<'_>,
    profile: ShaderProfile,
) -> CompileResult<()> {
    analyze(module, profile).map(|_| ())
}

pub(super) fn analyze(
    module: &UsagiShaderModule<'_>,
    _profile: ShaderProfile,
) -> CompileResult<CheckedShader> {
    validate_portable_tokens(&module.tokens)?;
    SemanticValidator::new(module)?.analyze()
}

fn validate_portable_tokens(tokens: &[Token<'_>]) -> CompileResult<()> {
    for token in tokens {
        if !is_code_token(token) {
            continue;
        }
        match token.text {
            "in" | "out" => {
                return Err(CompileError::at(
                    format!(
                        "generic shaders do not support GLSL interface qualifier '{}'; use usagi_main parameters",
                        token.text
                    ),
                    token.span,
                ));
            }
            "varying" => {
                return Err(CompileError::at(
                    "generic shaders do not support GLSL ES interface qualifier 'varying'; use usagi_main parameters",
                    token.span,
                ));
            }
            "layout" => {
                return Err(CompileError::at(
                    "generic shaders do not support layout qualifiers; Usagi owns target output layout",
                    token.span,
                ));
            }
            "precision" => {
                return Err(CompileError::at(
                    "generic shaders do not support precision declarations; Usagi emits target precision",
                    token.span,
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CheckedShader {
    expressions: Vec<CheckedExpression>,
}

impl CheckedShader {
    pub(super) fn expressions(&self) -> &[CheckedExpression] {
        &self.expressions
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CheckedExpression {
    pub(super) value_type: Option<ShaderType>,
    pub(super) span: super::syntax::SourceSpan,
}

#[derive(Clone, Copy, Debug)]
struct Symbol {
    ty: ShaderType,
}

#[derive(Clone, Copy, Debug)]
struct FunctionSignature {
    return_ty: ShaderType,
}

struct SemanticValidator<'module, 'src> {
    module: &'module UsagiShaderModule<'src>,
    globals: HashMap<&'src str, Symbol>,
    functions: HashMap<&'src str, FunctionSignature>,
    expressions: Vec<CheckedExpression>,
}

impl<'module, 'src> SemanticValidator<'module, 'src> {
    fn new(module: &'module UsagiShaderModule<'src>) -> CompileResult<Self> {
        let mut validator = Self {
            module,
            globals: HashMap::new(),
            functions: HashMap::new(),
            expressions: Vec::new(),
        };
        validator.collect_globals_and_signatures()?;
        Ok(validator)
    }

    fn collect_globals_and_signatures(&mut self) -> CompileResult<()> {
        let mut function_names = HashMap::new();

        for item in &self.module.items {
            match item {
                ShaderItem::Uniform(uniform) => self.collect_uniform(uniform)?,
                ShaderItem::Function(function) => {
                    if function.name != "usagi_main" && function.name.starts_with("usagi_") {
                        return Err(CompileError::at(
                            format!(
                                "'{}' uses the reserved usagi_ prefix; shader intrinsics own that namespace",
                                function.name
                            ),
                            function.name_span,
                        ));
                    }
                    if function_names
                        .insert(function.name, function.name_span)
                        .is_some()
                    {
                        return Err(CompileError::at(
                            format!(
                                "generic shaders do not support overloaded or duplicate function '{}'",
                                function.name
                            ),
                            function.name_span,
                        ));
                    }
                    let return_ty = ShaderType::parse(function.return_type).ok_or_else(|| {
                        CompileError::at(
                            format!(
                                "unsupported function return type '{}'",
                                function.return_type
                            ),
                            function.return_type_span,
                        )
                    })?;
                    if !return_ty.is_function_return() {
                        return Err(CompileError::at(
                            format!(
                                "function '{}' cannot return {} in a generic shader",
                                function.name,
                                return_ty.as_str()
                            ),
                            function.return_type_span,
                        ));
                    }
                    self.functions
                        .insert(function.name, FunctionSignature { return_ty });
                    self.validate_function_parameters(function)?;
                }
                ShaderItem::Raw(raw) => self.collect_global_declaration(raw)?,
            }
        }

        Ok(())
    }

    fn collect_uniform(&mut self, uniform: &UniformDecl<'src>) -> CompileResult<()> {
        let Some(ty) = ShaderType::parse(uniform.ty) else {
            return Err(CompileError::at(
                format!("unsupported uniform type '{}'", uniform.ty),
                uniform.ty_span,
            ));
        };
        if !ty.is_runtime_uniform() {
            return Err(CompileError::at(
                format!(
                    "generic shader uniforms support float, vec2, vec3, and vec4; '{}' is not supported",
                    uniform.ty
                ),
                uniform.ty_span,
            ));
        }

        for name in &uniform.names {
            validate_user_binding_name(name.name, name.span)?;
            if let Some(existing) = self.globals.insert(name.name, Symbol { ty }) {
                return Err(CompileError::at(
                    format!(
                        "duplicate global shader binding '{}'; first declaration has type {}",
                        name.name,
                        existing.ty.as_str()
                    ),
                    name.span,
                ));
            }
        }

        Ok(())
    }

    fn collect_global_declaration(&mut self, raw: &RawItem<'src>) -> CompileResult<()> {
        let Some(decl) = self.parse_declaration(&raw.expression, &self.globals)? else {
            return Ok(());
        };
        if !decl.ty.is_local_value() {
            return Err(CompileError::at(
                format!(
                    "global shader binding '{}' cannot use type {}",
                    decl.name,
                    decl.ty.as_str()
                ),
                decl.ty_span,
            ));
        }
        if let Some(actual) = decl.initializer_ty
            && actual != decl.ty
        {
            return Err(CompileError::at(
                format!(
                    "initializer type mismatch for '{}': expected {}, found {}",
                    decl.name,
                    decl.ty.as_str(),
                    actual.as_str()
                ),
                decl.initializer_span.unwrap_or(decl.name_span),
            ));
        }
        if let Some(existing) = self.globals.insert(decl.name, Symbol { ty: decl.ty }) {
            return Err(CompileError::at(
                format!(
                    "duplicate global shader binding '{}'; first declaration has type {}",
                    decl.name,
                    existing.ty.as_str()
                ),
                decl.name_span,
            ));
        }
        Ok(())
    }

    fn validate_function_parameters(&self, function: &FunctionDecl<'src>) -> CompileResult<()> {
        let mut names = HashSet::with_capacity(function.params.len());
        for param in &function.params {
            let Some(ty) = ShaderType::parse(param.ty) else {
                return Err(CompileError::at(
                    format!("unsupported parameter type '{}'", param.ty),
                    param.ty_span,
                ));
            };
            if !ty.is_function_parameter() {
                return Err(CompileError::at(
                    format!(
                        "function parameter '{}' cannot use type {} in a generic shader",
                        param.name,
                        ty.as_str()
                    ),
                    param.ty_span,
                ));
            }
            validate_user_binding_name(param.name, param.name_span)?;
            if !names.insert(param.name) {
                return Err(CompileError::at(
                    format!("duplicate parameter name '{}'", param.name),
                    param.name_span,
                ));
            }
        }
        Ok(())
    }

    fn analyze(mut self) -> CompileResult<CheckedShader> {
        for item in &self.module.items {
            match item {
                ShaderItem::Function(function) => self.validate_function(function)?,
                ShaderItem::Raw(raw) => {
                    let mut globals = self.globals.clone();
                    self.validate_expression(&raw.expression, &mut globals)?;
                }
                ShaderItem::Uniform(_) => {}
            }
        }
        Ok(CheckedShader {
            expressions: self.expressions,
        })
    }

    fn validate_function(&mut self, function: &FunctionDecl<'src>) -> CompileResult<()> {
        let return_ty = self
            .functions
            .get(function.name)
            .expect("function signatures are collected before validation")
            .return_ty;
        let mut symbols = self.globals.clone();

        for param in &function.params {
            let ty = ShaderType::parse(param.ty).expect("parameters validated before body checks");
            symbols.insert(param.name, Symbol { ty });
        }

        self.validate_block(&function.body, return_ty, &mut symbols)?;

        if return_ty != ShaderType::Void && !block_always_returns(&function.body) {
            return Err(CompileError::at(
                format!(
                    "function '{}' must return {} on all paths",
                    function.name,
                    return_ty.as_str()
                ),
                function.name_span,
            ));
        }

        Ok(())
    }

    fn validate_block(
        &mut self,
        block: &BlockAst<'src>,
        return_ty: ShaderType,
        symbols: &mut HashMap<&'src str, Symbol>,
    ) -> CompileResult<()> {
        for statement in &block.statements {
            self.validate_statement(statement, return_ty, symbols)?;
        }
        Ok(())
    }

    fn validate_statement(
        &mut self,
        statement: &StatementAst<'src>,
        return_ty: ShaderType,
        symbols: &mut HashMap<&'src str, Symbol>,
    ) -> CompileResult<()> {
        match statement {
            StatementAst::Return(stmt) => self.validate_return(stmt, return_ty, symbols),
            StatementAst::If(stmt) => {
                self.validate_expression(&stmt.condition, symbols)?;
                if let Some(condition_ty) = self.infer_expression_type(&stmt.condition, symbols)
                    && condition_ty != ShaderType::Bool
                {
                    return Err(CompileError::at(
                        format!("if condition must be bool; found {}", condition_ty.as_str()),
                        stmt.condition.span,
                    ));
                }

                let mut then_symbols = symbols.clone();
                self.validate_branch(&stmt.then_branch, return_ty, &mut then_symbols)?;
                if let Some(branch) = &stmt.else_branch {
                    let mut else_symbols = symbols.clone();
                    self.validate_branch(branch, return_ty, &mut else_symbols)?;
                }
                Ok(())
            }
            StatementAst::Block(block) => {
                let mut scoped = symbols.clone();
                self.validate_block(block, return_ty, &mut scoped)
            }
            StatementAst::Raw(stmt) => self.validate_raw_statement(stmt, symbols),
        }
    }

    fn validate_branch(
        &mut self,
        branch: &BranchAst<'src>,
        return_ty: ShaderType,
        symbols: &mut HashMap<&'src str, Symbol>,
    ) -> CompileResult<()> {
        match branch {
            BranchAst::Block(block) => self.validate_block(block, return_ty, symbols),
            BranchAst::Statement(statement) => {
                self.validate_statement(statement, return_ty, symbols)
            }
        }
    }

    fn validate_return(
        &mut self,
        stmt: &ReturnStmt<'src>,
        return_ty: ShaderType,
        symbols: &mut HashMap<&'src str, Symbol>,
    ) -> CompileResult<()> {
        match (&stmt.expression, return_ty) {
            (None, ShaderType::Void) => Ok(()),
            (None, _) => Err(CompileError::at(
                format!("return statement must return {}", return_ty.as_str()),
                stmt.span,
            )),
            (Some(expression), ShaderType::Void) => Err(CompileError::at(
                "void function must not return a value",
                expression.span,
            )),
            (Some(expression), expected) => {
                self.validate_expression(expression, symbols)?;
                if let Some(actual) = self.infer_expression_type(expression, symbols)
                    && actual != expected
                {
                    return Err(CompileError::at(
                        format!(
                            "return type mismatch: expected {}, found {}",
                            expected.as_str(),
                            actual.as_str()
                        ),
                        expression.span,
                    ));
                }
                Ok(())
            }
        }
    }

    fn validate_raw_statement(
        &mut self,
        stmt: &RawStmt<'src>,
        symbols: &mut HashMap<&'src str, Symbol>,
    ) -> CompileResult<()> {
        self.validate_expression(&stmt.expression, symbols)?;
        let Some(decl) = self.parse_declaration(&stmt.expression, symbols)? else {
            return Ok(());
        };
        if !decl.ty.is_local_value() {
            return Err(CompileError::at(
                format!(
                    "local shader variable '{}' cannot use type {}",
                    decl.name,
                    decl.ty.as_str()
                ),
                decl.ty_span,
            ));
        }
        if let Some(actual) = decl.initializer_ty
            && actual != decl.ty
        {
            return Err(CompileError::at(
                format!(
                    "initializer type mismatch for '{}': expected {}, found {}",
                    decl.name,
                    decl.ty.as_str(),
                    actual.as_str()
                ),
                decl.initializer_span.unwrap_or(decl.name_span),
            ));
        }
        symbols.insert(decl.name, Symbol { ty: decl.ty });
        Ok(())
    }

    fn validate_expression(
        &mut self,
        expression: &ExpressionAst<'src>,
        symbols: &mut HashMap<&'src str, Symbol>,
    ) -> CompileResult<()> {
        for node in &expression.nodes {
            match node {
                ExpressionNode::Token(idx) => {
                    let token = &self.module.tokens[*idx];
                    if is_code_token(token) && token.text == "texture0" {
                        return Err(CompileError::at(
                            "texture0 may only be used as the first argument to usagi_texture(...)",
                            token.span,
                        ));
                    }
                }
                ExpressionNode::Call(call) => self.validate_call(call, symbols)?,
            }
        }
        let value_type = self.infer_expression_type_checked(expression, symbols)?;
        self.expressions.push(CheckedExpression {
            value_type,
            span: expression.span,
        });
        Ok(())
    }

    fn validate_call(
        &mut self,
        call: &ExprCall<'src>,
        symbols: &mut HashMap<&'src str, Symbol>,
    ) -> CompileResult<()> {
        match call.kind {
            ExprCallKind::Intrinsic(IntrinsicKind::Texture) => {
                if !self.is_exact_texture0_argument(&call.args[0]) {
                    return Err(CompileError::at(
                        "usagi_texture sampler argument must be texture0",
                        call.args[0].span,
                    ));
                }
                self.validate_expression(&call.args[1], symbols)?;
                if let Some(actual) = self.infer_expression_type(&call.args[1], symbols)
                    && actual != ShaderType::Vec(2)
                {
                    return Err(CompileError::at(
                        format!(
                            "usagi_texture uv argument must be vec2; found {}",
                            actual.as_str()
                        ),
                        call.args[1].span,
                    ));
                }
            }
            ExprCallKind::Generic => {
                for arg in &call.args {
                    self.validate_expression(arg, symbols)?;
                }
            }
        }
        Ok(())
    }

    fn is_exact_texture0_argument(&self, expression: &ExpressionAst<'src>) -> bool {
        let mut code_nodes = expression
            .nodes
            .iter()
            .filter(|node| self.is_code_expression_node(node));
        let Some(ExpressionNode::Token(idx)) = code_nodes.next() else {
            return false;
        };
        code_nodes.next().is_none() && self.module.tokens[*idx].text == "texture0"
    }

    fn parse_declaration(
        &self,
        expression: &ExpressionAst<'src>,
        symbols: &HashMap<&'src str, Symbol>,
    ) -> CompileResult<Option<Declaration<'src>>> {
        let nodes: Vec<_> = expression
            .nodes
            .iter()
            .filter(|node| self.is_code_expression_node(node))
            .collect();
        if nodes.len() < 2 {
            return Ok(None);
        }

        let mut ty_node_idx = 0usize;
        if self.node_text(nodes[0]) == Some("const") {
            ty_node_idx = 1;
            if nodes.len() < 3 {
                return Ok(None);
            }
        }

        let Some(ty_text) = self.node_text(nodes[ty_node_idx]) else {
            return Ok(None);
        };
        let Some(ty) = ShaderType::parse(ty_text) else {
            return Ok(None);
        };
        let Some(ExpressionNode::Token(name_idx)) = nodes.get(ty_node_idx + 1).copied() else {
            return Ok(None);
        };
        let name_token = &self.module.tokens[*name_idx];
        if name_token.kind != TokenKind::Ident {
            return Ok(None);
        }
        validate_user_binding_name(name_token.text, name_token.span)?;

        let mut initializer_ty = None;
        let mut initializer_span = None;
        if let Some(eq_pos) = nodes
            .iter()
            .position(|node| self.node_text(node) == Some("="))
        {
            let init_nodes = &nodes[eq_pos + 1..];
            if !init_nodes.is_empty() {
                initializer_ty = self.infer_node_sequence_type_checked(init_nodes, symbols)?;
                initializer_span = Some(expression_nodes_span(init_nodes, &self.module.tokens));
            }
        }

        Ok(Some(Declaration {
            ty,
            ty_span: self.node_span(nodes[ty_node_idx]),
            name: name_token.text,
            name_span: name_token.span,
            initializer_ty,
            initializer_span,
        }))
    }

    fn infer_expression_type(
        &self,
        expression: &ExpressionAst<'src>,
        symbols: &HashMap<&'src str, Symbol>,
    ) -> Option<ShaderType> {
        self.infer_expression_type_checked(expression, symbols)
            .ok()
            .flatten()
    }

    fn infer_expression_type_checked(
        &self,
        expression: &ExpressionAst<'src>,
        symbols: &HashMap<&'src str, Symbol>,
    ) -> CompileResult<Option<ShaderType>> {
        let nodes: Vec<_> = expression
            .nodes
            .iter()
            .filter(|node| self.is_code_expression_node(node))
            .collect();
        self.infer_node_sequence_type_checked(&nodes, symbols)
    }

    fn infer_node_sequence_type_checked(
        &self,
        nodes: &[&ExpressionNode<'src>],
        symbols: &HashMap<&'src str, Symbol>,
    ) -> CompileResult<Option<ShaderType>> {
        let nodes = strip_outer_parentheses(self, nodes)?;
        if contains_assignment_operator(self, nodes) {
            return Ok(None);
        }

        match nodes {
            [] => Ok(None),
            [single] => self.infer_single_node_type_checked(single, symbols),
            [base, dot, swizzle] if self.node_text(dot) == Some(".") => {
                let Some(base_ty) = self.infer_single_node_type_checked(base, symbols)? else {
                    return Ok(None);
                };
                let Some(swizzle) = self.node_text(swizzle) else {
                    return Ok(None);
                };
                Ok(infer_swizzle_type(base_ty, swizzle))
            }
            _ => self.infer_binary_expression_type(nodes, symbols),
        }
    }

    fn infer_binary_expression_type(
        &self,
        nodes: &[&ExpressionNode<'src>],
        symbols: &HashMap<&'src str, Symbol>,
    ) -> CompileResult<Option<ShaderType>> {
        let Some((ty, next)) = self.parse_binary_type(nodes, symbols, 0, 0)? else {
            return Ok(None);
        };
        if next == nodes.len() {
            Ok(Some(ty))
        } else {
            Ok(None)
        }
    }

    fn parse_binary_type(
        &self,
        nodes: &[&ExpressionNode<'src>],
        symbols: &HashMap<&'src str, Symbol>,
        start: usize,
        min_precedence: u8,
    ) -> CompileResult<Option<(ShaderType, usize)>> {
        let Some((mut lhs, mut next)) = self.read_operand_type(nodes, start, symbols)? else {
            return Ok(None);
        };

        while let Some(op) = read_operator(self, nodes, next) {
            if op.precedence < min_precedence {
                break;
            }

            let Some((rhs, rhs_next)) =
                self.parse_binary_type(nodes, symbols, op.end_idx, op.precedence + 1)?
            else {
                return Ok(None);
            };

            let Some(result_ty) = infer_binary_operator_type(lhs, op.text, rhs) else {
                return Err(CompileError::at(
                    format!(
                        "operator '{}' cannot combine {} and {}",
                        op.text,
                        lhs.as_str(),
                        rhs.as_str()
                    ),
                    op.span,
                ));
            };
            lhs = result_ty;
            next = rhs_next;
        }

        Ok(Some((lhs, next)))
    }

    fn read_operand_type(
        &self,
        nodes: &[&ExpressionNode<'src>],
        start: usize,
        symbols: &HashMap<&'src str, Symbol>,
    ) -> CompileResult<Option<(ShaderType, usize)>> {
        let mut idx = start;
        let mut unary_sign = None;
        let mut logical_not = false;

        while let Some(text) = self.node_text_at(nodes, idx) {
            match text {
                "+" | "-" if unary_sign.is_none() => {
                    unary_sign = Some((text, self.node_span(nodes[idx])));
                    idx += 1;
                }
                "!" if !logical_not => {
                    logical_not = true;
                    idx += 1;
                }
                _ => break,
            }
        }

        let Some((mut ty, mut next)) = self.read_primary_type(nodes, idx, symbols)? else {
            return Ok(None);
        };

        if let Some((op, span)) = unary_sign
            && !ty.is_numeric()
        {
            return Err(CompileError::at(
                format!(
                    "unary '{}' requires a numeric operand; found {}",
                    op,
                    ty.as_str()
                ),
                span,
            ));
        }
        if logical_not {
            if ty != ShaderType::Bool {
                return Err(CompileError::at(
                    format!("unary '!' requires bool; found {}", ty.as_str()),
                    self.node_span(nodes[start]),
                ));
            }
            ty = ShaderType::Bool;
        }

        while self.node_text_at(nodes, next) == Some(".") {
            let Some(swizzle) = self.node_text_at(nodes, next + 1) else {
                return Ok(None);
            };
            let Some(swizzle_ty) = infer_swizzle_type(ty, swizzle) else {
                return Ok(None);
            };
            ty = swizzle_ty;
            next += 2;
        }

        Ok(Some((ty, next)))
    }

    fn read_primary_type(
        &self,
        nodes: &[&ExpressionNode<'src>],
        start: usize,
        symbols: &HashMap<&'src str, Symbol>,
    ) -> CompileResult<Option<(ShaderType, usize)>> {
        if self.node_text_at(nodes, start) == Some("(") {
            let close = matching_node_paren(self, nodes, start)?;
            let Some(ty) =
                self.infer_node_sequence_type_checked(&nodes[start + 1..close], symbols)?
            else {
                return Ok(None);
            };
            return Ok(Some((ty, close + 1)));
        }

        let Some(node) = nodes.get(start) else {
            return Ok(None);
        };
        let Some(ty) = self.infer_single_node_type_checked(node, symbols)? else {
            return Ok(None);
        };
        Ok(Some((ty, start + 1)))
    }

    fn infer_single_node_type_checked(
        &self,
        node: &ExpressionNode<'src>,
        symbols: &HashMap<&'src str, Symbol>,
    ) -> CompileResult<Option<ShaderType>> {
        match node {
            ExpressionNode::Token(idx) => {
                let token = &self.module.tokens[*idx];
                Ok(match token.kind {
                    TokenKind::Number => Some(number_type(token.text)),
                    TokenKind::Ident => match token.text {
                        "true" | "false" => Some(ShaderType::Bool),
                        name => symbols.get(name).map(|symbol| symbol.ty),
                    },
                    _ => None,
                })
            }
            ExpressionNode::Call(call) => self.infer_call_type_checked(call, symbols),
        }
    }

    fn infer_call_type_checked(
        &self,
        call: &ExprCall<'src>,
        symbols: &HashMap<&'src str, Symbol>,
    ) -> CompileResult<Option<ShaderType>> {
        match call.kind {
            ExprCallKind::Intrinsic(IntrinsicKind::Texture) => Ok(Some(ShaderType::Vec(4))),
            ExprCallKind::Generic => {
                if let Some(constructor_ty) = ShaderType::parse(call.name)
                    && constructor_ty != ShaderType::Void
                    && constructor_ty != ShaderType::Sampler2D
                {
                    return Ok(Some(constructor_ty));
                }
                if let Some(signature) = self.functions.get(call.name) {
                    return Ok(Some(signature.return_ty));
                }
                Ok(match call.name {
                    "dot" | "length" | "distance" => Some(ShaderType::Float),
                    "abs" | "floor" | "fract" | "sin" | "cos" | "tan" | "exp" | "pow" | "sqrt"
                    | "normalize" | "min" | "max" | "clamp" | "mix" | "step" | "smoothstep" => call
                        .args
                        .first()
                        .and_then(|arg| self.infer_expression_type(arg, symbols)),
                    _ => None,
                })
            }
        }
    }

    fn is_code_expression_node(&self, node: &ExpressionNode<'src>) -> bool {
        match node {
            ExpressionNode::Token(idx) => is_code_token(&self.module.tokens[*idx]),
            ExpressionNode::Call(_) => true,
        }
    }

    fn node_text(&self, node: &ExpressionNode<'src>) -> Option<&'src str> {
        match node {
            ExpressionNode::Token(idx) => Some(self.module.tokens[*idx].text),
            ExpressionNode::Call(call) => Some(call.name),
        }
    }

    fn node_span(&self, node: &ExpressionNode<'src>) -> super::syntax::SourceSpan {
        match node {
            ExpressionNode::Token(idx) => self.module.tokens[*idx].span,
            ExpressionNode::Call(call) => call.span,
        }
    }

    fn node_text_at(&self, nodes: &[&ExpressionNode<'src>], idx: usize) -> Option<&'src str> {
        nodes.get(idx).and_then(|node| self.node_text(node))
    }
}

#[derive(Clone, Copy, Debug)]
struct Declaration<'src> {
    ty: ShaderType,
    ty_span: super::syntax::SourceSpan,
    name: &'src str,
    name_span: super::syntax::SourceSpan,
    initializer_ty: Option<ShaderType>,
    initializer_span: Option<super::syntax::SourceSpan>,
}

fn validate_user_binding_name(name: &str, span: super::syntax::SourceSpan) -> CompileResult<()> {
    match name {
        "texture0" => Err(CompileError::at(
            "generic shaders must not declare texture0; Usagi binds it",
            span,
        )),
        "fragTexCoord" | "fragColor" => Err(CompileError::at(
            format!("'{name}' is engine-owned; use the usagi_main parameters instead"),
            span,
        )),
        "finalColor" | "gl_FragColor" => Err(CompileError::at(
            format!("'{name}' is target output state owned by the Usagi shader emitter"),
            span,
        )),
        "main" => Err(CompileError::at(
            "generic shaders must not declare main; Usagi emits it",
            span,
        )),
        name if name.starts_with("usagi_") => Err(CompileError::at(
            format!("'{name}' uses the reserved usagi_ prefix"),
            span,
        )),
        _ => Ok(()),
    }
}

fn block_always_returns(block: &BlockAst<'_>) -> bool {
    block.statements.iter().any(statement_always_returns)
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

fn branch_always_returns(branch: &BranchAst<'_>) -> bool {
    match branch {
        BranchAst::Block(block) => block_always_returns(block),
        BranchAst::Statement(statement) => statement_always_returns(statement),
    }
}

#[derive(Clone, Copy, Debug)]
struct BinaryOperator {
    text: &'static str,
    span: super::syntax::SourceSpan,
    precedence: u8,
    end_idx: usize,
}

fn strip_outer_parentheses<'a, 'src>(
    checker: &SemanticValidator<'_, 'src>,
    mut nodes: &'a [&ExpressionNode<'src>],
) -> CompileResult<&'a [&'a ExpressionNode<'src>]> {
    while nodes.len() >= 2
        && checker.node_text_at(nodes, 0) == Some("(")
        && checker.node_text_at(nodes, nodes.len() - 1) == Some(")")
    {
        if matching_node_paren(checker, nodes, 0)? != nodes.len() - 1 {
            break;
        }
        nodes = &nodes[1..nodes.len() - 1];
    }
    Ok(nodes)
}

fn contains_assignment_operator<'src>(
    checker: &SemanticValidator<'_, 'src>,
    nodes: &[&ExpressionNode<'src>],
) -> bool {
    for idx in 0..nodes.len() {
        let Some(text) = checker.node_text_at(nodes, idx) else {
            continue;
        };
        let prev = idx
            .checked_sub(1)
            .and_then(|prev_idx| checker.node_text_at(nodes, prev_idx));
        let next = checker.node_text_at(nodes, idx + 1);

        if text == "=" {
            if next == Some("=") || matches!(prev, Some("<" | ">" | "!" | "=")) {
                continue;
            }
            return true;
        }

        if matches!(text, "+" | "-" | "*" | "/" | "%") && next == Some("=") {
            return true;
        }
    }

    false
}

fn matching_node_paren<'src>(
    checker: &SemanticValidator<'_, 'src>,
    nodes: &[&ExpressionNode<'src>],
    open_idx: usize,
) -> CompileResult<usize> {
    let mut depth = 0usize;
    for idx in open_idx..nodes.len() {
        match checker.node_text_at(nodes, idx) {
            Some("(") => depth += 1,
            Some(")") => {
                depth = depth.checked_sub(1).ok_or_else(|| {
                    CompileError::at("unmatched ')' in expression", checker.node_span(nodes[idx]))
                })?;
                if depth == 0 {
                    return Ok(idx);
                }
            }
            _ => {}
        }
    }

    Err(CompileError::at(
        "unterminated parenthesized expression",
        checker.node_span(nodes[open_idx]),
    ))
}

fn read_operator<'src>(
    checker: &SemanticValidator<'_, 'src>,
    nodes: &[&ExpressionNode<'src>],
    idx: usize,
) -> Option<BinaryOperator> {
    let text = checker.node_text_at(nodes, idx)?;
    let next = checker.node_text_at(nodes, idx + 1);
    let (text, width) = match (text, next) {
        ("|", Some("|")) => ("||", 2),
        ("&", Some("&")) => ("&&", 2),
        ("=", Some("=")) => ("==", 2),
        ("!", Some("=")) => ("!=", 2),
        ("<", Some("=")) => ("<=", 2),
        (">", Some("=")) => (">=", 2),
        ("+", _) => ("+", 1),
        ("-", _) => ("-", 1),
        ("*", _) => ("*", 1),
        ("/", _) => ("/", 1),
        ("<", _) => ("<", 1),
        (">", _) => (">", 1),
        _ => return None,
    };

    Some(BinaryOperator {
        text,
        span: checker.node_span(nodes[idx]),
        precedence: operator_precedence(text),
        end_idx: idx + width,
    })
}

fn operator_precedence(op: &str) -> u8 {
    match op {
        "||" => 1,
        "&&" => 2,
        "==" | "!=" => 3,
        "<" | "<=" | ">" | ">=" => 4,
        "+" | "-" => 5,
        "*" | "/" => 6,
        _ => 0,
    }
}

fn infer_binary_operator_type(lhs: ShaderType, op: &str, rhs: ShaderType) -> Option<ShaderType> {
    match op {
        "+" | "-" | "*" | "/" => infer_arithmetic_binary_type(lhs, op, rhs),
        "<" | "<=" | ">" | ">=" if lhs.is_scalar_numeric() && rhs.is_scalar_numeric() => {
            Some(ShaderType::Bool)
        }
        "==" | "!=" if lhs == rhs || (lhs.is_scalar_numeric() && rhs.is_scalar_numeric()) => {
            Some(ShaderType::Bool)
        }
        "&&" | "||" if lhs == ShaderType::Bool && rhs == ShaderType::Bool => Some(ShaderType::Bool),
        _ => None,
    }
}

fn infer_arithmetic_binary_type(lhs: ShaderType, op: &str, rhs: ShaderType) -> Option<ShaderType> {
    if !lhs.is_numeric() || !rhs.is_numeric() {
        return None;
    }

    if lhs == rhs {
        return infer_same_type_arithmetic(lhs, op);
    }

    match (lhs, rhs) {
        (ShaderType::Int, ShaderType::Float) | (ShaderType::Float, ShaderType::Int) => {
            Some(ShaderType::Float)
        }
        (scalar, ShaderType::Vec(size)) if scalar.is_scalar_numeric() && rhs.is_float_vector() => {
            Some(ShaderType::Vec(size))
        }
        (ShaderType::Vec(size), scalar) if lhs.is_float_vector() && scalar.is_scalar_numeric() => {
            Some(ShaderType::Vec(size))
        }
        (ShaderType::Mat(size), scalar)
            if matches!(op, "*" | "/") && scalar.is_scalar_numeric() =>
        {
            Some(ShaderType::Mat(size))
        }
        (scalar, ShaderType::Mat(size)) if op == "*" && scalar.is_scalar_numeric() => {
            Some(ShaderType::Mat(size))
        }
        (ShaderType::Mat(size), ShaderType::Vec(vec_size)) if op == "*" && size == vec_size => {
            Some(ShaderType::Vec(size))
        }
        (ShaderType::Vec(vec_size), ShaderType::Mat(size)) if op == "*" && size == vec_size => {
            Some(ShaderType::Vec(vec_size))
        }
        _ => None,
    }
}

fn infer_same_type_arithmetic(ty: ShaderType, op: &str) -> Option<ShaderType> {
    match ty {
        ShaderType::Int | ShaderType::Float | ShaderType::Vec(_)
            if matches!(op, "+" | "-" | "*" | "/") =>
        {
            Some(ty)
        }
        ShaderType::Mat(_) if matches!(op, "+" | "-" | "*") => Some(ty),
        _ => None,
    }
}

fn infer_swizzle_type(base_ty: ShaderType, swizzle: &str) -> Option<ShaderType> {
    if swizzle.is_empty() || swizzle.len() > 4 {
        return None;
    }

    match base_ty {
        ShaderType::Vec(size) if swizzle_chars_match(swizzle, size, "xyzw", "rgba") => {
            match swizzle.len() {
                1 => Some(ShaderType::Float),
                len => Some(ShaderType::Vec(len as u8)),
            }
        }
        ShaderType::BVec(size) if swizzle_chars_match(swizzle, size, "xyzw", "rgba") => {
            match swizzle.len() {
                1 => Some(ShaderType::Bool),
                len => Some(ShaderType::BVec(len as u8)),
            }
        }
        ShaderType::IVec(size) if swizzle_chars_match(swizzle, size, "xyzw", "rgba") => {
            match swizzle.len() {
                1 => Some(ShaderType::Int),
                len => Some(ShaderType::IVec(len as u8)),
            }
        }
        _ => None,
    }
}

fn swizzle_chars_match(swizzle: &str, size: u8, first_set: &str, second_set: &str) -> bool {
    swizzle.chars().all(|ch| {
        first_set
            .find(ch)
            .or_else(|| second_set.find(ch))
            .is_some_and(|idx| idx < usize::from(size))
    })
}

fn number_type(text: &str) -> ShaderType {
    if text.contains('.') || text.contains('e') || text.contains('E') {
        ShaderType::Float
    } else {
        ShaderType::Int
    }
}

fn expression_nodes_span(
    nodes: &[&ExpressionNode<'_>],
    tokens: &[Token<'_>],
) -> super::syntax::SourceSpan {
    let first = node_span(nodes[0], tokens);
    let last = node_span(nodes[nodes.len() - 1], tokens);
    super::syntax::SourceSpan {
        start: first.start,
        end: last.end,
    }
}

fn node_span(node: &ExpressionNode<'_>, tokens: &[Token<'_>]) -> super::syntax::SourceSpan {
    match node {
        ExpressionNode::Token(idx) => tokens[*idx].span,
        ExpressionNode::Call(call) => call.span,
    }
}

#[cfg(test)]
mod tests {
    use super::super::syntax::UsagiShaderModule;
    use super::*;

    fn validate_fragment(src: &str, profile: ShaderProfile) -> Result<(), String> {
        let module = UsagiShaderModule::parse(src)?;
        validate(&module, profile).map_err(|err| err.render(src, module.source_offset))
    }

    #[test]
    fn target_validation_rejects_desktop_interface_qualifier_for_all_profiles() {
        let err = validate_fragment(
            "out vec4 customColor;\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("GLSL interface qualifier 'out'"));
        assert!(err.contains("line 1, column 1"));
    }

    #[test]
    fn target_validation_rejects_es_varying_qualifier_for_all_profiles() {
        let err = validate_fragment(
            "varying vec2 customUv;\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n",
            ShaderProfile::WebGlslEs100,
        )
        .unwrap_err();

        assert!(err.contains("varying"));
        assert!(err.contains("line 1, column 1"));
    }

    #[test]
    fn target_validation_rejects_layout_qualifier_for_all_profiles() {
        let err = validate_fragment(
            "layout(location = 0) out vec4 customColor;\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n",
            ShaderProfile::DesktopGlsl440,
        )
        .unwrap_err();

        assert!(err.contains("layout qualifiers"));
        assert!(err.contains("line 1, column 1"));
    }

    #[test]
    fn target_validation_rejects_precision_declaration_for_all_profiles() {
        let err = validate_fragment(
            "precision mediump float;\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n",
            ShaderProfile::WebGlslEs100,
        )
        .unwrap_err();

        assert!(err.contains("precision declarations"));
        assert!(err.contains("line 1, column 1"));
    }

    #[test]
    fn semantic_validation_rejects_missing_return_path() {
        let err = validate_fragment(
            "vec4 usagi_main(vec2 uv, vec4 color) {\n    if (uv.x > 0.5) return color;\n}\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("must return vec4 on all paths"));
        assert!(err.contains("line 1, column 6"));
    }

    #[test]
    fn semantic_validation_accepts_if_else_return_paths() {
        validate_fragment(
            "vec4 usagi_main(vec2 uv, vec4 color) {\n    if (uv.x > 0.5) return color;\n    else return vec4(0.0, 0.0, 0.0, 1.0);\n}\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap();
    }

    #[test]
    fn semantic_validation_rejects_return_without_value() {
        let err = validate_fragment(
            "vec4 usagi_main(vec2 uv, vec4 color) {\n    return;\n}\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("return statement must return vec4"));
    }

    #[test]
    fn semantic_validation_rejects_void_return_value() {
        let err = validate_fragment(
            "void helper() { return 1.0; }\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("void function must not return a value"));
    }

    #[test]
    fn semantic_validation_rejects_known_return_type_mismatch() {
        let err = validate_fragment(
            "vec4 usagi_main(vec2 uv, vec4 color) { return uv; }\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("return type mismatch"));
        assert!(err.contains("expected vec4, found vec2"));
    }

    #[test]
    fn semantic_validation_rejects_unsupported_uniform_types() {
        let err = validate_fragment(
            "uniform int u_mode;\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("uniforms support float, vec2, vec3, and vec4"));
        assert!(err.contains("'int' is not supported"));
    }

    #[test]
    fn semantic_validation_rejects_duplicate_uniform_names() {
        let err = validate_fragment(
            "uniform float u_time;\nuniform vec2 u_time;\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("duplicate global shader binding 'u_time'"));
    }

    #[test]
    fn semantic_validation_rejects_custom_texture_sampler_argument() {
        let err = validate_fragment(
            "uniform vec2 u_uv;\nvec4 usagi_main(vec2 uv, vec4 color) { return usagi_texture(u_uv, uv); }\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("sampler argument must be texture0"));
    }

    #[test]
    fn semantic_validation_rejects_direct_texture0_use() {
        let err = validate_fragment(
            "vec4 usagi_main(vec2 uv, vec4 color) { return vec4(texture0); }\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("texture0 may only be used"));
    }

    #[test]
    fn semantic_validation_rejects_texture_uv_type_mismatch() {
        let err = validate_fragment(
            "vec4 usagi_main(vec2 uv, vec4 color) { return usagi_texture(texture0, color); }\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("uv argument must be vec2"));
        assert!(err.contains("found vec4"));
    }

    #[test]
    fn semantic_validation_rejects_local_initializer_type_mismatch() {
        let err = validate_fragment(
            "vec4 usagi_main(vec2 uv, vec4 color) {\n    vec4 bad = uv;\n    return color;\n}\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("initializer type mismatch for 'bad'"));
        assert!(err.contains("expected vec4, found vec2"));
    }

    #[test]
    fn semantic_validation_accepts_scalar_vector_arithmetic_chains() {
        validate_fragment(
            "vec4 usagi_main(vec2 uv, vec4 color) {\n    vec2 adjusted = uv * 2.0 - 1.0;\n    return color * 0.5 + vec4(adjusted, 0.0, 1.0);\n}\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap();
    }

    #[test]
    fn semantic_validation_rejects_binary_vector_size_mismatch() {
        let err = validate_fragment(
            "vec4 usagi_main(vec2 uv, vec4 color) {\n    vec3 bad = uv + color.rgb;\n    return color;\n}\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("operator '+' cannot combine vec2 and vec3"));
        assert!(err.contains("line 2, column 19"));
    }

    #[test]
    fn semantic_validation_rejects_non_bool_binary_if_condition() {
        let err = validate_fragment(
            "vec4 usagi_main(vec2 uv, vec4 color) {\n    if (uv + 0.5) return color;\n    return color;\n}\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("if condition must be bool; found vec2"));
        assert!(err.contains("line 2, column 9"));
    }

    #[test]
    fn semantic_validation_accepts_logical_comparison_chains() {
        validate_fragment(
            "vec4 usagi_main(vec2 uv, vec4 color) {\n    if (uv.x < 0.0 || uv.x > 1.0) return color;\n    return color;\n}\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap();
    }

    #[test]
    fn semantic_validation_keeps_assignment_operators_raw() {
        validate_fragment(
            "vec4 usagi_main(vec2 uv, vec4 color) {\n    vec3 col = color.rgb;\n    col *= 0.5;\n    col.r = color.r;\n    return vec4(col, color.a);\n}\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap();
    }
}
