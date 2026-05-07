use std::collections::{HashMap, HashSet};

use super::emit_glsl::{self, GlslTarget};
use super::syntax::{
    BlockAst, BranchAst, ExprCall, ExprCallKind, ExpressionAst, ExpressionNode, FunctionDecl,
    IntrinsicKind, RawItem, RawStmt, ReturnStmt, ShaderItem, StatementAst, Token, TokenKind,
    UniformDecl, UsagiShaderModule, is_code_token,
};
use super::{CompileError, CompileResult, CompileWarning, ShaderProfile};

pub(super) fn validate(
    module: &UsagiShaderModule<'_>,
    profile: ShaderProfile,
) -> CompileResult<()> {
    validate_target_tokens(&module.tokens, emit_glsl::target(profile))?;
    SemanticValidator::new(module)?.validate()
}

pub(super) fn warnings(module: &UsagiShaderModule<'_>) -> Vec<CompileWarning> {
    DuplicateTextureSampleAnalyzer::new(module).collect()
}

fn validate_target_tokens(tokens: &[Token<'_>], target: &GlslTarget) -> CompileResult<()> {
    for token in tokens {
        if !is_code_token(token) {
            continue;
        }
        match token.text {
            "in" | "out" if !target.supports_desktop_interface_qualifiers => {
                return Err(CompileError::at(
                    format!(
                        "{} generic shaders do not support desktop interface qualifier '{}'",
                        target.name, token.text
                    ),
                    token.span,
                ));
            }
            "varying" if !target.supports_es_varying_qualifier => {
                return Err(CompileError::at(
                    format!(
                        "{} generic shaders do not support GLSL ES interface qualifier 'varying'",
                        target.name
                    ),
                    token.span,
                ));
            }
            "layout" if !target.supports_layout_qualifier => {
                return Err(CompileError::at(
                    format!(
                        "{} generic shaders do not support layout qualifiers",
                        target.name
                    ),
                    token.span,
                ));
            }
            "precision" if !target.supports_precision_directive => {
                return Err(CompileError::at(
                    format!(
                        "{} generic shaders do not support precision declarations",
                        target.name
                    ),
                    token.span,
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShaderType {
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

impl ShaderType {
    fn parse(src: &str) -> Option<Self> {
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

    fn as_str(self) -> &'static str {
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

    fn is_runtime_uniform(self) -> bool {
        matches!(
            self,
            Self::Float | Self::Vec(2) | Self::Vec(3) | Self::Vec(4)
        )
    }

    fn is_function_return(self) -> bool {
        !matches!(self, Self::Sampler2D)
    }

    fn is_function_parameter(self) -> bool {
        !matches!(self, Self::Void | Self::Sampler2D)
    }

    fn is_local_value(self) -> bool {
        !matches!(self, Self::Void | Self::Sampler2D)
    }
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
}

struct DuplicateTextureSampleAnalyzer<'module, 'src> {
    module: &'module UsagiShaderModule<'src>,
    seen: HashMap<String, ExprCall<'src>>,
    warnings: Vec<CompileWarning>,
}

impl<'module, 'src> DuplicateTextureSampleAnalyzer<'module, 'src> {
    fn new(module: &'module UsagiShaderModule<'src>) -> Self {
        Self {
            module,
            seen: HashMap::new(),
            warnings: Vec::new(),
        }
    }

    fn collect(mut self) -> Vec<CompileWarning> {
        for item in &self.module.items {
            match item {
                ShaderItem::Function(function) => self.visit_block(&function.body),
                ShaderItem::Raw(raw) => self.visit_expression(&raw.expression),
                ShaderItem::Uniform(_) => {}
            }
        }
        self.warnings
    }

    fn visit_block(&mut self, block: &BlockAst<'src>) {
        for statement in &block.statements {
            self.visit_statement(statement);
        }
    }

    fn visit_statement(&mut self, statement: &StatementAst<'src>) {
        match statement {
            StatementAst::Return(stmt) => {
                if let Some(expression) = &stmt.expression {
                    self.visit_expression(expression);
                }
            }
            StatementAst::If(stmt) => {
                self.visit_expression(&stmt.condition);
                self.visit_branch(&stmt.then_branch);
                if let Some(branch) = &stmt.else_branch {
                    self.visit_branch(branch);
                }
            }
            StatementAst::Block(block) => self.visit_block(block),
            StatementAst::Raw(stmt) => self.visit_expression(&stmt.expression),
        }
    }

    fn visit_branch(&mut self, branch: &BranchAst<'src>) {
        match branch {
            BranchAst::Block(block) => self.visit_block(block),
            BranchAst::Statement(statement) => self.visit_statement(statement),
        }
    }

    fn visit_expression(&mut self, expression: &ExpressionAst<'src>) {
        for node in &expression.nodes {
            let ExpressionNode::Call(call) = node else {
                continue;
            };

            if call.kind == ExprCallKind::Intrinsic(IntrinsicKind::Texture) {
                self.record_texture_sample(call);
            }
            for arg in &call.args {
                self.visit_expression(arg);
            }
        }
    }

    fn record_texture_sample(&mut self, call: &ExprCall<'src>) {
        let key = normalized_expression_key(self.module, &call.args[1]);
        if self.seen.contains_key(&key) {
            self.warnings.push(CompileWarning::at(
                format!(
                    "duplicate usagi_texture(texture0, {key}) sample; reuse the first sample when possible"
                ),
                call.span,
            ));
            return;
        }
        self.seen.insert(key, call.clone());
    }
}

impl<'module, 'src> SemanticValidator<'module, 'src> {
    fn new(module: &'module UsagiShaderModule<'src>) -> CompileResult<Self> {
        let mut validator = Self {
            module,
            globals: HashMap::new(),
            functions: HashMap::new(),
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

    fn validate(self) -> CompileResult<()> {
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
        Ok(())
    }

    fn validate_function(&self, function: &FunctionDecl<'src>) -> CompileResult<()> {
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
        &self,
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
        &self,
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
        &self,
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
        &self,
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
        &self,
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
        &self,
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
        Ok(())
    }

    fn validate_call(
        &self,
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
                initializer_ty = self.infer_node_sequence_type(init_nodes, symbols);
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
        let nodes: Vec<_> = expression
            .nodes
            .iter()
            .filter(|node| self.is_code_expression_node(node))
            .collect();
        self.infer_node_sequence_type(&nodes, symbols)
    }

    fn infer_node_sequence_type(
        &self,
        nodes: &[&ExpressionNode<'src>],
        symbols: &HashMap<&'src str, Symbol>,
    ) -> Option<ShaderType> {
        match nodes {
            [single] => self.infer_single_node_type(single, symbols),
            [base, dot, swizzle] if self.node_text(dot) == Some(".") => {
                let base_ty = self.infer_single_node_type(base, symbols)?;
                let swizzle = self.node_text(swizzle)?;
                infer_swizzle_type(base_ty, swizzle)
            }
            _ => None,
        }
    }

    fn infer_single_node_type(
        &self,
        node: &ExpressionNode<'src>,
        symbols: &HashMap<&'src str, Symbol>,
    ) -> Option<ShaderType> {
        match node {
            ExpressionNode::Token(idx) => {
                let token = &self.module.tokens[*idx];
                match token.kind {
                    TokenKind::Number => Some(number_type(token.text)),
                    TokenKind::Ident => match token.text {
                        "true" | "false" => Some(ShaderType::Bool),
                        name => symbols.get(name).map(|symbol| symbol.ty),
                    },
                    _ => None,
                }
            }
            ExpressionNode::Call(call) => self.infer_call_type(call, symbols),
        }
    }

    fn infer_call_type(
        &self,
        call: &ExprCall<'src>,
        symbols: &HashMap<&'src str, Symbol>,
    ) -> Option<ShaderType> {
        match call.kind {
            ExprCallKind::Intrinsic(IntrinsicKind::Texture) => Some(ShaderType::Vec(4)),
            ExprCallKind::Generic => {
                if let Some(constructor_ty) = ShaderType::parse(call.name)
                    && constructor_ty != ShaderType::Void
                    && constructor_ty != ShaderType::Sampler2D
                {
                    return Some(constructor_ty);
                }
                if let Some(signature) = self.functions.get(call.name) {
                    return Some(signature.return_ty);
                }
                match call.name {
                    "dot" | "length" | "distance" => Some(ShaderType::Float),
                    "abs" | "floor" | "fract" | "sin" | "cos" | "tan" | "exp" | "pow" | "sqrt"
                    | "normalize" | "min" | "max" | "clamp" | "mix" | "step" | "smoothstep" => call
                        .args
                        .first()
                        .and_then(|arg| self.infer_expression_type(arg, symbols)),
                    _ => None,
                }
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

fn normalized_expression_key(
    module: &UsagiShaderModule<'_>,
    expression: &ExpressionAst<'_>,
) -> String {
    let mut out = String::new();
    write_normalized_expression_key(module, expression, &mut out);
    out
}

fn write_normalized_expression_key(
    module: &UsagiShaderModule<'_>,
    expression: &ExpressionAst<'_>,
    out: &mut String,
) {
    for node in &expression.nodes {
        match node {
            ExpressionNode::Token(idx) => {
                let token = &module.tokens[*idx];
                if is_code_token(token) {
                    out.push_str(token.text);
                }
            }
            ExpressionNode::Call(call) => {
                out.push_str(call.name);
                out.push('(');
                for (idx, arg) in call.args.iter().enumerate() {
                    if idx > 0 {
                        out.push(',');
                    }
                    write_normalized_expression_key(module, arg, out);
                }
                out.push(')');
            }
        }
    }
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
    fn target_validation_rejects_desktop_interface_qualifier_for_es_100() {
        let err = validate_fragment(
            "out vec4 customColor;\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n",
            ShaderProfile::WebGlslEs100,
        )
        .unwrap_err();

        assert!(err.contains("GLSL ES 100"));
        assert!(err.contains("desktop interface qualifier 'out'"));
        assert!(err.contains("line 1, column 1"));
    }

    #[test]
    fn target_validation_rejects_es_varying_qualifier_for_desktop() {
        let err = validate_fragment(
            "varying vec2 customUv;\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("GLSL 330"));
        assert!(err.contains("varying"));
        assert!(err.contains("line 1, column 1"));
    }

    #[test]
    fn target_validation_rejects_layout_qualifier_for_330() {
        let err = validate_fragment(
            "layout(location = 0) out vec4 customColor;\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("GLSL 330"));
        assert!(err.contains("layout qualifiers"));
        assert!(err.contains("line 1, column 1"));
    }

    #[test]
    fn target_validation_rejects_precision_declaration_for_desktop() {
        let err = validate_fragment(
            "precision mediump float;\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n",
            ShaderProfile::DesktopGlsl440,
        )
        .unwrap_err();

        assert!(err.contains("GLSL 440"));
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
}
