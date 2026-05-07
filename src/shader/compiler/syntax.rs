use super::{CompileError, CompileResult};

#[derive(Debug)]
pub(super) struct UsagiShaderModule<'src> {
    pub(super) tokens: Vec<Token<'src>>,
    pub(super) source: ShaderSource,
    pub(super) items: Vec<ShaderItem<'src>>,
    pub(super) entrypoint_name: &'src str,
    pub(super) source_offset: usize,
    pub(super) source_start_line: usize,
}

impl<'src> UsagiShaderModule<'src> {
    #[cfg(test)]
    pub(super) fn parse(src: &'src str) -> Result<Self, String> {
        Self::parse_with_diagnostic(src).map_err(|err| err.error.render(src, err.source_offset))
    }

    pub(super) fn parse_with_diagnostic(src: &'src str) -> Result<Self, ParseFailure> {
        let (body, source_offset) = shader_body(src);
        let mut module = Self::parse_body(body).map_err(|error| ParseFailure {
            error,
            source_offset,
        })?;
        module.source_offset = source_offset;
        module.source_start_line = source_line_at_offset(src, source_offset);
        Ok(module)
    }

    fn parse_body(body: &'src str) -> CompileResult<Self> {
        let tokens = lex(body)?;
        reject_version_directive(&tokens)?;
        reject_preprocessor_lines(&tokens)?;
        reject_reserved_identifiers(&tokens)?;
        let items = parse_items(&tokens)?;
        validate_items(&items)?;
        let entrypoint_name = find_entrypoint(&items)?;
        let source = ShaderSource::from_items(&items);
        Ok(Self {
            tokens,
            source,
            items,
            entrypoint_name,
            source_offset: 0,
            source_start_line: 1,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ParseFailure {
    pub(super) error: CompileError,
    pub(super) source_offset: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SourceSpan {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl SourceSpan {
    pub(super) fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    fn join(self, other: Self) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    pub(super) fn shifted(self, offset: usize) -> Self {
        Self {
            start: self.start + offset,
            end: self.end + offset,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Token<'src> {
    pub(super) kind: TokenKind,
    pub(super) text: &'src str,
    pub(super) span: SourceSpan,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TokenKind {
    Ident,
    Number,
    Symbol,
    Whitespace,
    LineComment,
    BlockComment,
    PreprocessorLine,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ShaderItem<'src> {
    Function(FunctionDecl<'src>),
    Uniform(UniformDecl<'src>),
    Raw(RawItem<'src>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FunctionDecl<'src> {
    pub(super) return_type: &'src str,
    pub(super) return_type_span: SourceSpan,
    pub(super) name: &'src str,
    pub(super) name_span: SourceSpan,
    pub(super) params: Vec<ParamDecl<'src>>,
    pub(super) body: BlockAst<'src>,
    pub(super) span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ParamDecl<'src> {
    pub(super) ty: &'src str,
    pub(super) ty_span: SourceSpan,
    pub(super) name: &'src str,
    pub(super) name_span: SourceSpan,
    pub(super) span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct UniformDecl<'src> {
    pub(super) ty: &'src str,
    pub(super) ty_span: SourceSpan,
    pub(super) names: Vec<UniformName<'src>>,
    pub(super) span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct UniformName<'src> {
    pub(super) name: &'src str,
    pub(super) span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RawItem<'src> {
    pub(super) expression: ExpressionAst<'src>,
    pub(super) span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct BlockAst<'src> {
    pub(super) open_idx: usize,
    pub(super) close_idx: usize,
    pub(super) statements: Vec<StatementAst<'src>>,
    pub(super) span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum StatementAst<'src> {
    Return(ReturnStmt<'src>),
    If(IfStmt<'src>),
    Block(BlockAst<'src>),
    Raw(RawStmt<'src>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ReturnStmt<'src> {
    pub(super) expression: Option<ExpressionAst<'src>>,
    pub(super) span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct IfStmt<'src> {
    pub(super) condition: ExpressionAst<'src>,
    pub(super) then_branch: BranchAst<'src>,
    pub(super) else_branch: Option<BranchAst<'src>>,
    pub(super) span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum BranchAst<'src> {
    Block(BlockAst<'src>),
    Statement(Box<StatementAst<'src>>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RawStmt<'src> {
    pub(super) expression: ExpressionAst<'src>,
    pub(super) span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ExpressionAst<'src> {
    pub(super) nodes: Vec<ExpressionNode<'src>>,
    pub(super) span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ExpressionNode<'src> {
    Token(usize),
    Call(ExprCall<'src>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ExprCall<'src> {
    pub(super) kind: ExprCallKind,
    pub(super) name: &'src str,
    pub(super) name_idx: usize,
    pub(super) close_idx: usize,
    pub(super) args: Vec<ExpressionAst<'src>>,
    pub(super) span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CallArgument {
    start_idx: usize,
    end_idx: usize,
    span: SourceSpan,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExprCallKind {
    Generic,
    Intrinsic(IntrinsicKind),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ShaderSource {
    pub(super) rewrites: Vec<SourceRewrite>,
}

impl ShaderSource {
    fn from_items(items: &[ShaderItem<'_>]) -> Self {
        let mut rewrites = Vec::new();
        collect_source_rewrites_from_items(items, &mut rewrites);
        sort_source_rewrites(&mut rewrites);
        Self { rewrites }
    }

    pub(super) fn with_additional_rewrites(&self, additional: Vec<SourceRewrite>) -> Self {
        let mut rewrites = Vec::with_capacity(self.rewrites.len() + additional.len());
        rewrites.extend(self.rewrites.iter().cloned());
        rewrites.extend(additional);
        sort_source_rewrites(&mut rewrites);
        Self { rewrites }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SourceRewrite {
    pub(super) kind: SourceRewriteKind,
    pub(super) start_idx: usize,
    pub(super) end_idx: usize,
    pub(super) span: SourceSpan,
}

impl SourceRewrite {
    pub(super) fn intrinsic_call(kind: IntrinsicKind, name_idx: usize, close_idx: usize) -> Self {
        Self {
            kind: SourceRewriteKind::Intrinsic(kind),
            start_idx: name_idx,
            end_idx: close_idx + 1,
            span: SourceSpan::new(0, 0),
        }
    }

    pub(super) fn replacement(
        start_idx: usize,
        end_idx: usize,
        span: SourceSpan,
        replacement: String,
    ) -> Self {
        Self {
            kind: SourceRewriteKind::Replacement(replacement),
            start_idx,
            end_idx,
            span,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum SourceRewriteKind {
    Intrinsic(IntrinsicKind),
    Replacement(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum IntrinsicKind {
    Texture,
}

fn shader_body(src: &str) -> (&str, usize) {
    let mut start = 0;
    for line in src.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            start += line.len();
            continue;
        }
        if trimmed == "#usagi shader 1" {
            start += line.len();
            break;
        }
        break;
    }

    let body = &src[start..];
    let trimmed = body.trim_start_matches('\n');
    let skipped = body.len() - trimmed.len();
    (trimmed, start + skipped)
}

pub(super) fn source_location(src: &str, byte_idx: usize) -> (usize, usize, usize, usize) {
    let byte_idx = byte_idx.min(src.len());
    let prefix = &src[..byte_idx];
    let line = prefix.bytes().filter(|b| *b == b'\n').count() + 1;
    let line_start = prefix.rfind('\n').map_or(0, |idx| idx + 1);
    let line_end = src[byte_idx..]
        .find('\n')
        .map_or(src.len(), |idx| byte_idx + idx);
    let column = src[line_start..byte_idx].chars().count() + 1;
    (line, column, line_start, line_end)
}

fn source_line_at_offset(src: &str, byte_idx: usize) -> usize {
    src[..byte_idx.min(src.len())]
        .bytes()
        .filter(|b| *b == b'\n')
        .count()
        + 1
}

fn lex(src: &str) -> CompileResult<Vec<Token<'_>>> {
    let mut tokens = Vec::new();
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_whitespace() {
            let start = i;
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            push_token(&mut tokens, TokenKind::Whitespace, &src[start..i], start, i);
        } else if b == b'/' && bytes.get(i + 1) == Some(&b'/') {
            let start = i;
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            if i < bytes.len() {
                i += 1;
            }
            push_token(
                &mut tokens,
                TokenKind::LineComment,
                &src[start..i],
                start,
                i,
            );
        } else if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
            let start = i;
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            if i + 1 >= bytes.len() {
                return Err(CompileError::at(
                    "unterminated block comment",
                    SourceSpan::new(start, src.len()),
                ));
            }
            i += 2;
            push_token(
                &mut tokens,
                TokenKind::BlockComment,
                &src[start..i],
                start,
                i,
            );
        } else if b == b'#' {
            let start = i;
            i += 1;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            if i < bytes.len() {
                i += 1;
            }
            push_token(
                &mut tokens,
                TokenKind::PreprocessorLine,
                &src[start..i],
                start,
                i,
            );
        } else if is_ident_start(b) {
            let start = i;
            i += 1;
            while i < bytes.len() && is_ident_continue(bytes[i]) {
                i += 1;
            }
            push_token(&mut tokens, TokenKind::Ident, &src[start..i], start, i);
        } else if b.is_ascii_digit()
            || (b == b'.' && bytes.get(i + 1).is_some_and(u8::is_ascii_digit))
        {
            let start = i;
            i += 1;
            while i < bytes.len() && is_number_continue(bytes[i]) {
                i += 1;
            }
            push_token(&mut tokens, TokenKind::Number, &src[start..i], start, i);
        } else {
            let start = i;
            let ch = src[i..]
                .chars()
                .next()
                .ok_or_else(|| CompileError::new("invalid shader source character boundary"))?;
            i += ch.len_utf8();
            push_token(&mut tokens, TokenKind::Symbol, &src[start..i], start, i);
        }
    }
    Ok(tokens)
}

fn push_token<'src>(
    tokens: &mut Vec<Token<'src>>,
    kind: TokenKind,
    text: &'src str,
    start: usize,
    end: usize,
) {
    tokens.push(Token {
        kind,
        text,
        span: SourceSpan::new(start, end),
    });
}

fn is_ident_start(b: u8) -> bool {
    b == b'_' || b.is_ascii_alphabetic()
}

fn is_ident_continue(b: u8) -> bool {
    is_ident_start(b) || b.is_ascii_digit()
}

fn is_number_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'+' | b'-' | b'_')
}

fn reject_version_directive(tokens: &[Token<'_>]) -> CompileResult<()> {
    if let Some(token) = tokens.iter().find(|t| {
        t.kind == TokenKind::PreprocessorLine && t.text.trim_start().starts_with("#version")
    }) {
        return Err(CompileError::at(
            "generic shaders must not declare #version",
            token.span,
        ));
    }
    Ok(())
}

fn reject_preprocessor_lines(tokens: &[Token<'_>]) -> CompileResult<()> {
    if let Some(token) = tokens
        .iter()
        .find(|token| token.kind == TokenKind::PreprocessorLine)
    {
        return Err(CompileError::at(
            "generic shaders do not support GLSL preprocessor directives; use native .fs fallbacks for target-specific GLSL",
            token.span,
        ));
    }
    Ok(())
}

fn reject_reserved_identifiers(tokens: &[Token<'_>]) -> CompileResult<()> {
    for (idx, token) in tokens.iter().enumerate() {
        if token.kind != TokenKind::Ident {
            continue;
        }
        if previous_code_token_text(tokens, idx) == Some(".") {
            continue;
        }
        match token.text {
            "fragTexCoord" | "fragColor" => {
                return Err(CompileError::at(
                    format!(
                        "'{}' is engine-owned; use the usagi_main parameters instead",
                        token.text
                    ),
                    token.span,
                ));
            }
            "finalColor" | "gl_FragColor" => {
                return Err(CompileError::at(
                    format!(
                        "'{}' is target output state owned by the Usagi shader emitter",
                        token.text
                    ),
                    token.span,
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

fn previous_code_token_text<'src>(tokens: &[Token<'src>], idx: usize) -> Option<&'src str> {
    tokens[..idx]
        .iter()
        .rev()
        .find(|token| is_code_token(token))
        .map(|token| token.text)
}

fn parse_items<'src>(tokens: &[Token<'src>]) -> CompileResult<Vec<ShaderItem<'src>>> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(start) = next_code(tokens, i) {
        if token_text(tokens, start) == Some("uniform") {
            let end = find_top_level_semicolon(tokens, start).ok_or_else(|| {
                CompileError::at("unterminated uniform declaration", tokens[start].span)
            })?;
            out.push(ShaderItem::Uniform(parse_uniform(tokens, start, end)?));
            i = end + 1;
            continue;
        }

        if let Some((decl, next)) = parse_function(tokens, start)? {
            out.push(ShaderItem::Function(decl));
            i = next;
            continue;
        }

        if let Some(end) = find_top_level_semicolon(tokens, start) {
            out.push(ShaderItem::Raw(parse_raw_item(tokens, start, end)?));
            i = end + 1;
        } else {
            return Err(CompileError::at(
                format!(
                    "could not parse top-level shader item near '{}'",
                    token_text(tokens, start).unwrap_or("<unknown>")
                ),
                tokens[start].span,
            ));
        }
    }
    Ok(out)
}

fn parse_raw_item<'src>(
    tokens: &[Token<'src>],
    start: usize,
    end: usize,
) -> CompileResult<RawItem<'src>> {
    Ok(RawItem {
        expression: parse_expression(tokens, start, end)?,
        span: SourceSpan::new(tokens[start].span.start, tokens[end].span.end),
    })
}

fn parse_uniform<'src>(
    tokens: &[Token<'src>],
    start: usize,
    end: usize,
) -> CompileResult<UniformDecl<'src>> {
    let mut code_count = 0usize;
    let mut ty = None;
    let mut ty_span = None;
    let mut names = Vec::new();
    let mut expect_name = false;

    for token in tokens[start..end]
        .iter()
        .filter(|token| is_code_token(token))
    {
        match code_count {
            0 => {}
            1 => {
                ty = Some(token.text);
                ty_span = Some(token.span);
                expect_name = true;
            }
            _ => match token.text {
                "," => expect_name = true,
                "[" => {
                    return Err(CompileError::at(
                        "generic shader uniform arrays are not supported; declare separate float/vec uniforms for Lua writes",
                        token.span,
                    ));
                }
                "]" => {}
                name if expect_name && token.kind == TokenKind::Ident => {
                    names.push(UniformName {
                        name,
                        span: token.span,
                    });
                    expect_name = false;
                }
                _ => {}
            },
        }
        code_count += 1;
    }

    if code_count < 3 {
        return Err(CompileError::at(
            "uniform declaration must include a type and name",
            tokens[start].span,
        ));
    }
    if names.is_empty() {
        return Err(CompileError::at(
            "uniform declaration must include at least one name",
            tokens[start].span,
        ));
    }
    Ok(UniformDecl {
        ty: ty.expect("uniform code count guarantees a type token"),
        ty_span: ty_span.expect("uniform code count guarantees a type token"),
        names,
        span: SourceSpan::new(tokens[start].span.start, tokens[end].span.end),
    })
}

fn parse_function<'src>(
    tokens: &[Token<'src>],
    start: usize,
) -> CompileResult<Option<(FunctionDecl<'src>, usize)>> {
    let Some(ret) = token_text(tokens, start) else {
        return Ok(None);
    };
    if !is_type_name(ret) {
        return Ok(None);
    }
    let Some(name_idx) = next_code(tokens, start + 1) else {
        return Ok(None);
    };
    let Some(name) = token_text(tokens, name_idx) else {
        return Ok(None);
    };
    if tokens[name_idx].kind != TokenKind::Ident {
        return Ok(None);
    }
    let Some(open_paren) = next_code(tokens, name_idx + 1) else {
        return Ok(None);
    };
    if token_text(tokens, open_paren) != Some("(") {
        return Ok(None);
    }
    let close_paren = find_matching(tokens, open_paren, "(", ")").ok_or_else(|| {
        CompileError::at(
            format!("function '{name}' has an unterminated parameter list"),
            tokens[name_idx].span,
        )
    })?;
    let Some(open_brace) = next_code(tokens, close_paren + 1) else {
        return Ok(None);
    };
    if token_text(tokens, open_brace) != Some("{") {
        return Ok(None);
    }
    let close_brace = find_matching(tokens, open_brace, "{", "}").ok_or_else(|| {
        CompileError::at(
            format!("function '{name}' has an unterminated body"),
            tokens[name_idx].span,
        )
    })?;
    Ok(Some((
        FunctionDecl {
            return_type: ret,
            return_type_span: tokens[start].span,
            name,
            name_span: tokens[name_idx].span,
            params: parse_params(tokens, open_paren + 1, close_paren)?,
            body: parse_block(tokens, open_brace, close_brace)?,
            span: SourceSpan::new(tokens[start].span.start, tokens[close_brace].span.end),
        },
        close_brace + 1,
    )))
}

fn parse_params<'src>(
    tokens: &[Token<'src>],
    start: usize,
    end: usize,
) -> CompileResult<Vec<ParamDecl<'src>>> {
    let mut code = tokens[start..end]
        .iter()
        .filter(|token| is_code_token(token));
    let Some(first) = code.next() else {
        return Ok(Vec::new());
    };

    let mut params = Vec::new();
    let mut pending_ty = Some(first);
    let mut expect_comma = false;

    for token in code {
        if expect_comma {
            if token.text != "," {
                return Err(CompileError::at(
                    "function parameters must be separated by commas",
                    token.span,
                ));
            }
            pending_ty = None;
            expect_comma = false;
            continue;
        }

        let Some(ty) = pending_ty else {
            if token.text == "," {
                return Err(CompileError::at(
                    "function parameter cannot be empty",
                    token.span,
                ));
            }
            pending_ty = Some(token);
            continue;
        };

        if ty.kind != TokenKind::Ident || token.kind != TokenKind::Ident {
            return Err(CompileError::at(
                "function parameter must use simple 'type name' syntax",
                ty.span.join(token.span),
            ));
        }

        params.push(ParamDecl {
            ty: ty.text,
            ty_span: ty.span,
            name: token.text,
            name_span: token.span,
            span: ty.span.join(token.span),
        });
        pending_ty = None;
        expect_comma = true;
    }

    if let Some(ty) = pending_ty {
        return Err(CompileError::at(
            "function parameter must include type and name",
            ty.span,
        ));
    }
    if !expect_comma && !params.is_empty() {
        return Err(CompileError::at(
            "function parameter cannot be empty",
            tokens[end.saturating_sub(1)].span,
        ));
    }
    Ok(params)
}

fn parse_block<'src>(
    tokens: &[Token<'src>],
    open_idx: usize,
    close_idx: usize,
) -> CompileResult<BlockAst<'src>> {
    let mut statements = Vec::new();
    let mut i = open_idx + 1;
    while let Some(start) = next_code(tokens, i) {
        if start >= close_idx {
            break;
        }
        let (statement, next) = parse_statement(tokens, start, close_idx)?;
        statements.push(statement);
        i = next;
    }

    Ok(BlockAst {
        open_idx,
        close_idx,
        statements,
        span: SourceSpan::new(tokens[open_idx].span.start, tokens[close_idx].span.end),
    })
}

fn parse_statement<'src>(
    tokens: &[Token<'src>],
    start: usize,
    limit: usize,
) -> CompileResult<(StatementAst<'src>, usize)> {
    match token_text(tokens, start) {
        Some("return") => parse_return_statement(tokens, start, limit),
        Some("if") => parse_if_statement(tokens, start, limit),
        Some("{") => {
            let close_idx = find_matching(tokens, start, "{", "}").ok_or_else(|| {
                CompileError::at("shader block has an unterminated body", tokens[start].span)
            })?;
            if close_idx > limit {
                return Err(CompileError::at(
                    "shader block closes outside its parent block",
                    tokens[start].span,
                ));
            }
            Ok((
                StatementAst::Block(parse_block(tokens, start, close_idx)?),
                close_idx + 1,
            ))
        }
        _ => parse_raw_statement(tokens, start, limit),
    }
}

fn parse_return_statement<'src>(
    tokens: &[Token<'src>],
    start: usize,
    limit: usize,
) -> CompileResult<(StatementAst<'src>, usize)> {
    let end = find_statement_semicolon(tokens, start, limit).ok_or_else(|| {
        CompileError::at("return statement must end with ';'", tokens[start].span)
    })?;
    let expression = match next_code(tokens, start + 1) {
        Some(expr_start) if expr_start < end => Some(parse_expression(tokens, expr_start, end)?),
        _ => None,
    };
    Ok((
        StatementAst::Return(ReturnStmt {
            expression,
            span: SourceSpan::new(tokens[start].span.start, tokens[end].span.end),
        }),
        end + 1,
    ))
}

fn parse_if_statement<'src>(
    tokens: &[Token<'src>],
    start: usize,
    limit: usize,
) -> CompileResult<(StatementAst<'src>, usize)> {
    let Some(open_paren) = next_code(tokens, start + 1) else {
        return Err(CompileError::at(
            "if statement must include a condition",
            tokens[start].span,
        ));
    };
    if token_text(tokens, open_paren) != Some("(") {
        return Err(CompileError::at(
            "if statement condition must start with '('",
            tokens[open_paren].span,
        ));
    }
    let close_paren = find_matching(tokens, open_paren, "(", ")").ok_or_else(|| {
        CompileError::at(
            "if statement has an unterminated condition",
            tokens[start].span,
        )
    })?;
    if close_paren > limit {
        return Err(CompileError::at(
            "if statement condition closes outside its parent block",
            tokens[start].span,
        ));
    }

    let condition = parse_expression(tokens, open_paren + 1, close_paren)?;
    let Some(then_start) = next_code(tokens, close_paren + 1) else {
        return Err(CompileError::at(
            "if statement must include a body",
            tokens[start].span,
        ));
    };
    let (then_branch, then_next) = parse_branch(tokens, then_start, limit)?;
    let mut next = then_next;
    let else_branch = match next_code(tokens, then_next) {
        Some(else_idx) if else_idx < limit && token_text(tokens, else_idx) == Some("else") => {
            let Some(else_body_start) = next_code(tokens, else_idx + 1) else {
                return Err(CompileError::at(
                    "else statement must include a body",
                    tokens[else_idx].span,
                ));
            };
            let (branch, branch_next) = parse_branch(tokens, else_body_start, limit)?;
            next = branch_next;
            Some(branch)
        }
        _ => None,
    };

    let span_end = statement_branch_span_end(else_branch.as_ref().unwrap_or(&then_branch));
    Ok((
        StatementAst::If(IfStmt {
            condition,
            then_branch,
            else_branch,
            span: SourceSpan::new(tokens[start].span.start, span_end),
        }),
        next,
    ))
}

fn parse_branch<'src>(
    tokens: &[Token<'src>],
    start: usize,
    limit: usize,
) -> CompileResult<(BranchAst<'src>, usize)> {
    if token_text(tokens, start) == Some("{") {
        let close_idx = find_matching(tokens, start, "{", "}").ok_or_else(|| {
            CompileError::at("shader block has an unterminated body", tokens[start].span)
        })?;
        if close_idx > limit {
            return Err(CompileError::at(
                "shader block closes outside its parent block",
                tokens[start].span,
            ));
        }
        return Ok((
            BranchAst::Block(parse_block(tokens, start, close_idx)?),
            close_idx + 1,
        ));
    }

    let (statement, next) = parse_statement(tokens, start, limit)?;
    Ok((BranchAst::Statement(Box::new(statement)), next))
}

fn parse_raw_statement<'src>(
    tokens: &[Token<'src>],
    start: usize,
    limit: usize,
) -> CompileResult<(StatementAst<'src>, usize)> {
    let end = find_statement_semicolon(tokens, start, limit).ok_or_else(|| {
        CompileError::at(
            format!(
                "could not parse shader statement near '{}'",
                token_text(tokens, start).unwrap_or("<unknown>")
            ),
            tokens[start].span,
        )
    })?;
    Ok((
        StatementAst::Raw(RawStmt {
            expression: parse_expression(tokens, start, end)?,
            span: SourceSpan::new(tokens[start].span.start, tokens[end].span.end),
        }),
        end + 1,
    ))
}

fn statement_branch_span_end(branch: &BranchAst<'_>) -> usize {
    match branch {
        BranchAst::Block(block) => block.span.end,
        BranchAst::Statement(statement) => statement_span(statement).end,
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

fn parse_expression<'src>(
    tokens: &[Token<'src>],
    start: usize,
    end: usize,
) -> CompileResult<ExpressionAst<'src>> {
    let mut nodes = Vec::new();
    let mut i = start;
    while i < end {
        let token = &tokens[i];
        if token.kind == TokenKind::Ident {
            if let Some(open_idx) = next_code(tokens, i + 1)
                && open_idx < end
                && token_text(tokens, open_idx) == Some("(")
            {
                let call = parse_call_expression(tokens, i, open_idx, end)?;
                i = call.close_idx + 1;
                nodes.push(ExpressionNode::Call(call));
                continue;
            }

            match token.text {
                "usagi_main" => {}
                name if name.starts_with("usagi_") => {
                    return Err(CompileError::at(
                        format!("unknown Usagi shader intrinsic '{name}'"),
                        token.span,
                    ));
                }
                "texture" | "texture2D" => {
                    return Err(CompileError::at(
                        "generic shaders must use usagi_texture(...) instead of target-specific texture functions",
                        token.span,
                    ));
                }
                _ => {}
            }
        }

        nodes.push(ExpressionNode::Token(i));
        i += 1;
    }

    Ok(ExpressionAst {
        nodes,
        span: expression_span(tokens, start, end),
    })
}

fn parse_call_expression<'src>(
    tokens: &[Token<'src>],
    name_idx: usize,
    open_idx: usize,
    limit: usize,
) -> CompileResult<ExprCall<'src>> {
    let name = tokens[name_idx].text;
    let close_idx = find_matching(tokens, open_idx, "(", ")").ok_or_else(|| {
        CompileError::at(
            format!("function call '{name}' has an unterminated argument list"),
            tokens[name_idx].span,
        )
    })?;
    if close_idx >= limit {
        return Err(CompileError::at(
            format!("function call '{name}' closes outside its expression"),
            tokens[name_idx].span,
        ));
    }

    let arg_ranges = split_call_args(tokens, open_idx + 1, close_idx)?;
    let mut args = Vec::with_capacity(arg_ranges.len());
    for arg in arg_ranges {
        args.push(parse_expression(tokens, arg.start_idx, arg.end_idx + 1)?);
    }

    let kind = match name {
        "usagi_texture" => {
            if args.len() != 2 {
                return Err(CompileError::at(
                    "usagi_texture expects 2 arguments: sampler and uv",
                    SourceSpan::new(tokens[name_idx].span.start, tokens[close_idx].span.end),
                ));
            }
            ExprCallKind::Intrinsic(IntrinsicKind::Texture)
        }
        "texture" | "texture2D" => {
            return Err(CompileError::at(
                "generic shaders must use usagi_texture(...) instead of target-specific texture functions",
                tokens[name_idx].span,
            ));
        }
        name if name.starts_with("usagi_") => {
            return Err(CompileError::at(
                format!("unknown Usagi shader intrinsic '{name}'"),
                tokens[name_idx].span,
            ));
        }
        _ => ExprCallKind::Generic,
    };

    Ok(ExprCall {
        kind,
        name,
        name_idx,
        close_idx,
        args,
        span: SourceSpan::new(tokens[name_idx].span.start, tokens[close_idx].span.end),
    })
}

fn expression_span(tokens: &[Token<'_>], start: usize, end: usize) -> SourceSpan {
    let start_span = tokens
        .get(start)
        .map(|token| token.span)
        .unwrap_or_else(|| SourceSpan::new(0, 0));
    let end_span = tokens
        .get(end.saturating_sub(1))
        .map(|token| token.span)
        .unwrap_or(start_span);
    SourceSpan::new(start_span.start, end_span.end)
}

fn collect_source_rewrites_from_items(items: &[ShaderItem<'_>], out: &mut Vec<SourceRewrite>) {
    for item in items {
        match item {
            ShaderItem::Function(function) => {
                collect_source_rewrites_from_block(&function.body, out)
            }
            ShaderItem::Raw(raw) => collect_source_rewrites_from_expression(&raw.expression, out),
            ShaderItem::Uniform(_) => {}
        }
    }
}

fn collect_source_rewrites_from_block(block: &BlockAst<'_>, out: &mut Vec<SourceRewrite>) {
    for statement in &block.statements {
        collect_source_rewrites_from_statement(statement, out);
    }
}

fn collect_source_rewrites_from_statement(
    statement: &StatementAst<'_>,
    out: &mut Vec<SourceRewrite>,
) {
    match statement {
        StatementAst::Return(stmt) => {
            if let Some(expression) = &stmt.expression {
                collect_source_rewrites_from_expression(expression, out);
            }
        }
        StatementAst::If(stmt) => {
            collect_source_rewrites_from_expression(&stmt.condition, out);
            collect_source_rewrites_from_branch(&stmt.then_branch, out);
            if let Some(branch) = &stmt.else_branch {
                collect_source_rewrites_from_branch(branch, out);
            }
        }
        StatementAst::Block(block) => collect_source_rewrites_from_block(block, out),
        StatementAst::Raw(stmt) => collect_source_rewrites_from_expression(&stmt.expression, out),
    }
}

fn collect_source_rewrites_from_branch(branch: &BranchAst<'_>, out: &mut Vec<SourceRewrite>) {
    match branch {
        BranchAst::Block(block) => collect_source_rewrites_from_block(block, out),
        BranchAst::Statement(statement) => collect_source_rewrites_from_statement(statement, out),
    }
}

fn collect_source_rewrites_from_expression(
    expression: &ExpressionAst<'_>,
    out: &mut Vec<SourceRewrite>,
) {
    for node in &expression.nodes {
        let ExpressionNode::Call(call) = node else {
            continue;
        };
        if let ExprCallKind::Intrinsic(kind) = call.kind {
            let mut rewrite = SourceRewrite::intrinsic_call(kind, call.name_idx, call.close_idx);
            rewrite.span = call.span;
            out.push(rewrite);
        }
        for arg in &call.args {
            collect_source_rewrites_from_expression(arg, out);
        }
    }
}

fn sort_source_rewrites(rewrites: &mut [SourceRewrite]) {
    rewrites.sort_by_key(|rewrite| (rewrite.start_idx, rewrite.end_idx));
}

fn validate_items(items: &[ShaderItem<'_>]) -> CompileResult<()> {
    for item in items {
        match item {
            ShaderItem::Function(function) if function.name == "main" => {
                return Err(CompileError::at(
                    "generic shaders must not declare main(); Usagi emits it",
                    function.name_span,
                ));
            }
            ShaderItem::Function(_) | ShaderItem::Raw(_) => {}
            ShaderItem::Uniform(uniform) => {
                if let Some(name) = uniform.names.iter().find(|name| name.name == "texture0") {
                    return Err(CompileError::at(
                        "generic shaders must not declare texture0; Usagi binds it",
                        name.span,
                    ));
                }
            }
        }
    }
    Ok(())
}

fn find_entrypoint<'src>(items: &[ShaderItem<'src>]) -> CompileResult<&'src str> {
    let mut matches = items.iter().filter_map(|item| match item {
        ShaderItem::Function(function) if function.name == "usagi_main" => Some(function),
        _ => None,
    });
    let Some(entrypoint) = matches.next() else {
        return Err(CompileError::new(
            "generic shaders must define vec4 usagi_main(vec2 uv, vec4 color)",
        ));
    };
    if let Some(duplicate) = matches.next() {
        return Err(CompileError::at(
            "generic shaders must define exactly one usagi_main function",
            duplicate.name_span,
        ));
    }
    if entrypoint.return_type != "vec4"
        || entrypoint.params.len() != 2
        || entrypoint.params[0].ty != "vec2"
        || entrypoint.params[1].ty != "vec4"
        || entrypoint.params[0].name != "uv"
        || entrypoint.params[1].name != "color"
    {
        return Err(CompileError::at(
            "usagi_main signature must be vec4 usagi_main(vec2 uv, vec4 color)",
            entrypoint.span,
        ));
    }
    Ok(entrypoint.name)
}

fn is_type_name(name: &str) -> bool {
    matches!(
        name,
        "void"
            | "bool"
            | "int"
            | "float"
            | "vec2"
            | "vec3"
            | "vec4"
            | "bvec2"
            | "bvec3"
            | "bvec4"
            | "ivec2"
            | "ivec3"
            | "ivec4"
            | "mat2"
            | "mat3"
            | "mat4"
    )
}

fn next_code(tokens: &[Token<'_>], start: usize) -> Option<usize> {
    tokens
        .iter()
        .enumerate()
        .skip(start)
        .find(|(_, t)| is_code_token(t))
        .map(|(i, _)| i)
}

pub(super) fn is_code_token(token: &Token<'_>) -> bool {
    !matches!(
        token.kind,
        TokenKind::Whitespace
            | TokenKind::LineComment
            | TokenKind::BlockComment
            | TokenKind::PreprocessorLine
    )
}

fn token_text<'src>(tokens: &[Token<'src>], idx: usize) -> Option<&'src str> {
    tokens.get(idx).map(|t| t.text)
}

fn find_top_level_semicolon(tokens: &[Token<'_>], start: usize) -> Option<usize> {
    let mut paren = 0usize;
    let mut brace = 0usize;
    let mut bracket = 0usize;
    for (i, token) in tokens.iter().enumerate().skip(start) {
        if !is_code_token(token) {
            continue;
        }
        match token.text {
            "(" => paren += 1,
            ")" => paren = paren.saturating_sub(1),
            "{" => brace += 1,
            "}" => brace = brace.saturating_sub(1),
            "[" => bracket += 1,
            "]" => bracket = bracket.saturating_sub(1),
            ";" if paren == 0 && brace == 0 && bracket == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

fn find_statement_semicolon(tokens: &[Token<'_>], start: usize, limit: usize) -> Option<usize> {
    let mut paren = 0usize;
    let mut brace = 0usize;
    for (i, token) in tokens.iter().enumerate().take(limit).skip(start) {
        if !is_code_token(token) {
            continue;
        }
        match token.text {
            "(" => paren += 1,
            ")" => paren = paren.saturating_sub(1),
            "{" => brace += 1,
            "}" => brace = brace.saturating_sub(1),
            ";" if paren == 0 && brace == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

fn find_matching(tokens: &[Token<'_>], open_idx: usize, open: &str, close: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, token) in tokens.iter().enumerate().skip(open_idx) {
        if !is_code_token(token) {
            continue;
        }
        match token.text {
            text if text == open => depth += 1,
            text if text == close => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn split_call_args(
    tokens: &[Token<'_>],
    start: usize,
    end: usize,
) -> CompileResult<Vec<CallArgument>> {
    let mut args = Vec::new();
    let mut arg_start = None;
    let mut arg_end = None;
    let mut paren = 0usize;
    let mut bracket = 0usize;
    let mut brace = 0usize;
    let mut ended_with_separator = false;

    for (i, token) in tokens.iter().enumerate().take(end).skip(start) {
        if !is_code_token(token) {
            continue;
        }
        if arg_start.is_none() {
            arg_start = Some(i);
        }

        match token.text {
            "," if paren == 0 && bracket == 0 && brace == 0 => {
                args.push(finish_call_argument(tokens, arg_start, arg_end)?);
                arg_start = None;
                arg_end = None;
                ended_with_separator = true;
                continue;
            }
            "(" => paren += 1,
            ")" => {
                paren = paren.checked_sub(1).ok_or_else(|| {
                    CompileError::at("shader intrinsic argument has unmatched ')'", token.span)
                })?;
            }
            "[" => bracket += 1,
            "]" => {
                bracket = bracket.checked_sub(1).ok_or_else(|| {
                    CompileError::at("shader intrinsic argument has unmatched ']'", token.span)
                })?;
            }
            "{" => brace += 1,
            "}" => {
                brace = brace.checked_sub(1).ok_or_else(|| {
                    CompileError::at("shader intrinsic argument has unmatched '}'", token.span)
                })?;
            }
            _ => {}
        }

        arg_end = Some(i);
        ended_with_separator = false;
    }

    if ended_with_separator {
        return Err(CompileError::at(
            "shader intrinsic argument cannot be empty",
            tokens[end.saturating_sub(1)].span,
        ));
    }

    if paren != 0 || bracket != 0 || brace != 0 {
        return Err(CompileError::at(
            "shader intrinsic argument list has unmatched delimiters",
            tokens[start.saturating_sub(1)].span,
        ));
    }

    if arg_start.is_some() || arg_end.is_some() {
        args.push(finish_call_argument(tokens, arg_start, arg_end)?);
    }

    Ok(args)
}

fn finish_call_argument(
    tokens: &[Token<'_>],
    start_idx: Option<usize>,
    end_idx: Option<usize>,
) -> CompileResult<CallArgument> {
    let (Some(start_idx), Some(end_idx)) = (start_idx, end_idx) else {
        return Err(CompileError::new(
            "shader intrinsic argument cannot be empty",
        ));
    };
    Ok(CallArgument {
        start_idx,
        end_idx,
        span: SourceSpan::new(tokens[start_idx].span.start, tokens[end_idx].span.end),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_function<'module, 'src>(
        module: &'module UsagiShaderModule<'src>,
    ) -> &'module FunctionDecl<'src> {
        module
            .items
            .iter()
            .find_map(|item| match item {
                ShaderItem::Function(function) => Some(function),
                ShaderItem::Uniform(_) | ShaderItem::Raw(_) => None,
            })
            .unwrap()
    }

    fn first_uniform<'module, 'src>(
        module: &'module UsagiShaderModule<'src>,
    ) -> &'module UniformDecl<'src> {
        module
            .items
            .iter()
            .find_map(|item| match item {
                ShaderItem::Uniform(uniform) => Some(uniform),
                ShaderItem::Function(_) | ShaderItem::Raw(_) => None,
            })
            .unwrap()
    }

    fn first_intrinsic_call<'a, 'src>(
        expression: &'a ExpressionAst<'src>,
    ) -> Option<&'a ExprCall<'src>> {
        for node in &expression.nodes {
            let ExpressionNode::Call(call) = node else {
                continue;
            };
            if matches!(call.kind, ExprCallKind::Intrinsic(_)) {
                return Some(call);
            }
            for arg in &call.args {
                if let Some(found) = first_intrinsic_call(arg) {
                    return Some(found);
                }
            }
        }
        None
    }

    #[test]
    fn parser_records_texture_intrinsic_as_ast_call_node() {
        let src = "vec4 usagi_main(vec2 uv, vec4 color) { return usagi_texture(texture0, clamp(uv, 0.0, 1.0)); }\n";
        let module = UsagiShaderModule::parse(src).unwrap();
        let function = first_function(&module);
        let StatementAst::Return(return_stmt) = &function.body.statements[0] else {
            panic!("expected return statement");
        };
        let expression = return_stmt.expression.as_ref().unwrap();
        let call = first_intrinsic_call(expression).unwrap();

        assert_eq!(call.kind, ExprCallKind::Intrinsic(IntrinsicKind::Texture));
        assert_eq!(call.args.len(), 2);
        assert_eq!(call.name, "usagi_texture");
        assert_eq!(module.tokens[call.name_idx].text, "usagi_texture");
        let ExpressionNode::Token(first_arg_token) = call.args[0].nodes[0] else {
            panic!("expected texture0 token argument");
        };
        assert_eq!(module.tokens[first_arg_token].text, "texture0");
        let ExpressionNode::Call(arg_call) = &call.args[1].nodes[0] else {
            panic!("expected clamp call argument");
        };
        assert_eq!(arg_call.name, "clamp");
        assert_eq!(
            &src[call.span.start..call.span.end],
            "usagi_texture(texture0, clamp(uv, 0.0, 1.0))"
        );
        assert_eq!(module.source.rewrites.len(), 1);
        assert_eq!(module.source.rewrites[0].start_idx, call.name_idx);
    }

    #[test]
    fn parser_records_declaration_spans() {
        let src = "uniform float u_time;\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n";
        let module = UsagiShaderModule::parse(src).unwrap();
        let uniform = first_uniform(&module);
        let function = first_function(&module);

        assert_eq!(
            &src[uniform.span.start..uniform.span.end],
            "uniform float u_time;"
        );
        assert_eq!(&src[uniform.ty_span.start..uniform.ty_span.end], "float");
        assert_eq!(uniform.names[0].name, "u_time");
        assert_eq!(
            &src[uniform.names[0].span.start..uniform.names[0].span.end],
            "u_time"
        );
        assert_eq!(
            &src[function.name_span.start..function.name_span.end],
            "usagi_main"
        );
        assert_eq!(
            &src[function.params[0].span.start..function.params[0].span.end],
            "vec2 uv"
        );
        assert_eq!(
            &src[function.params[1].name_span.start..function.params[1].name_span.end],
            "color"
        );
    }

    #[test]
    fn parser_rejects_texture_intrinsic_with_wrong_arity() {
        let err = UsagiShaderModule::parse(
            "vec4 usagi_main(vec2 uv, vec4 color) { return usagi_texture(texture0); }\n",
        )
        .unwrap_err();

        assert!(err.contains("usagi_texture expects 2 arguments"));
    }

    #[test]
    fn parser_rejects_empty_texture_intrinsic_arguments() {
        let err = UsagiShaderModule::parse(
            "vec4 usagi_main(vec2 uv, vec4 color) { return usagi_texture(texture0, uv,); }\n",
        )
        .unwrap_err();

        assert!(err.contains("argument cannot be empty"));
    }

    #[test]
    fn parser_rejects_target_specific_texture_calls() {
        let err = UsagiShaderModule::parse(
            "vec4 usagi_main(vec2 uv, vec4 color) { return texture(texture0, uv); }\n",
        )
        .unwrap_err();

        assert!(err.contains("usagi_texture"));
    }

    #[test]
    fn parser_rejects_reserved_engine_bindings() {
        let err = UsagiShaderModule::parse(
            "uniform sampler2D texture0;\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n",
        )
        .unwrap_err();

        assert!(err.contains("texture0"));
    }

    #[test]
    fn parser_rejects_preprocessor_lines_after_usagi_marker() {
        let err = UsagiShaderModule::parse(
            "#usagi shader 1\n#extension GL_OES_standard_derivatives : enable\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n",
        )
        .unwrap_err();

        assert!(err.contains("preprocessor directives"));
        assert!(err.contains("line 2, column 1"));
    }

    #[test]
    fn parser_rejects_uniform_arrays() {
        let err = UsagiShaderModule::parse(
            "uniform float u_kernel[9];\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n",
        )
        .unwrap_err();

        assert!(err.contains("uniform arrays are not supported"));
        assert!(err.contains("line 1, column 23"));
    }

    #[test]
    fn parser_rejects_duplicate_entrypoint() {
        let err = UsagiShaderModule::parse(
            "vec4 usagi_main(vec2 uv, vec4 color) { return color; }\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n",
        )
        .unwrap_err();

        assert!(err.contains("exactly one"));
    }

    #[test]
    fn parser_rejects_bad_entrypoint_signature() {
        let err = UsagiShaderModule::parse(
            "vec3 usagi_main(vec2 uv, vec4 color) { return color.rgb; }\n",
        )
        .unwrap_err();

        assert!(err.contains("signature"));
    }

    #[test]
    fn parser_rejects_entrypoint_with_wrong_parameter_names() {
        let err = UsagiShaderModule::parse(
            "vec4 usagi_main(vec2 texcoord, vec4 tint) { return tint; }\n",
        )
        .unwrap_err();

        assert!(err.contains("vec4 usagi_main(vec2 uv, vec4 color)"));
    }

    #[test]
    fn parser_rejects_function_parameters_without_commas() {
        let err =
            UsagiShaderModule::parse("vec4 usagi_main(vec2 uv vec4 color) { return color; }\n")
                .unwrap_err();

        assert!(err.contains("parameters must be separated by commas"));
    }

    #[test]
    fn parser_rejects_trailing_function_parameter_comma() {
        let err =
            UsagiShaderModule::parse("vec4 usagi_main(vec2 uv, vec4 color,) { return color; }\n")
                .unwrap_err();

        assert!(err.contains("function parameter cannot be empty"));
    }

    #[test]
    fn parser_rejects_unmatched_texture_intrinsic_delimiters() {
        let err = UsagiShaderModule::parse(
            "vec4 usagi_main(vec2 uv, vec4 color) { return usagi_texture(texture0, uv[0); }\n",
        )
        .unwrap_err();

        assert!(err.contains("unmatched delimiters"));
    }

    #[test]
    fn parser_rejects_unmatched_texture_intrinsic_closer() {
        let err = UsagiShaderModule::parse(
            "vec4 usagi_main(vec2 uv, vec4 color) { return usagi_texture(texture0, uv]); }\n",
        )
        .unwrap_err();

        assert!(err.contains("unmatched ']'"));
    }

    #[test]
    fn parser_ignores_comment_mentions_of_reserved_words() {
        let src = "// fragColor finalColor texture()\nvec4 usagi_main(vec2 uv, vec4 color) { return usagi_texture(texture0, uv); }\n";

        assert!(UsagiShaderModule::parse(src).is_ok());
    }

    #[test]
    fn parser_allows_member_access_matching_reserved_engine_names() {
        let src = concat!(
            "vec4 usagi_main(vec2 uv, vec4 color) {\n",
            "    return material.fragTexCoord.xxyy + material.fragColor + ",
            "material.finalColor + material.gl_FragColor;\n",
            "}\n",
        );

        assert!(UsagiShaderModule::parse(src).is_ok());
    }
}
