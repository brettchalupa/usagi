//! Compiler for the `.usagi.fs` shader dialect.
//!
//! The dialect is intentionally close to fragment GLSL, but the source
//! is not passed through verbatim. We parse a small module ABT, validate
//! the engine-owned bindings, lower Usagi intrinsics, then emit
//! target-specific GLSL.
//!
//! Current contract:
//! - an optional `#usagi shader 1` marker may appear as the first non-blank line;
//! - source must define exactly one `vec4 usagi_main(vec2 uv, vec4 color)`;
//! - `texture0`, `fragTexCoord`, `fragColor`, `finalColor`, `gl_FragColor`,
//!   and `main` are emitter-owned names;
//! - `usagi_texture(texture0, uv)` is the target-neutral texture intrinsic;
//! - direct `texture(...)` / `texture2D(...)` calls are rejected so generic
//!   sources remain portable across GLSL ES 100, GLSL 330, and staged GLSL 440.

use super::ShaderProfile;

pub(super) fn compile_fragment_with_metadata(
    src: &str,
    profile: ShaderProfile,
) -> Result<CompiledFragment, String> {
    let module = UsagiShaderModule::parse(src)?;
    let source = TargetEmitter { profile }.emit(&module)?;
    let metadata = ShaderMetadata::from_module(profile, &module);
    Ok(CompiledFragment { source, metadata })
}

type CompileResult<T> = Result<T, CompileError>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CompiledFragment {
    pub(super) source: String,
    pub(super) metadata: ShaderMetadata,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ShaderMetadata {
    pub(super) profile: ShaderProfile,
    pub(super) uniforms: Vec<ShaderUniform>,
}

impl ShaderMetadata {
    fn from_module(profile: ShaderProfile, module: &UsagiShaderModule<'_>) -> Self {
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

        Self { profile, uniforms }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ShaderUniform {
    pub(super) ty: String,
    pub(super) name: String,
    pub(super) ty_span: SourceSpan,
    pub(super) name_span: SourceSpan,
    pub(super) declaration_span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CompileError {
    message: String,
    span: Option<SourceSpan>,
}

impl CompileError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
        }
    }

    fn at(message: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            message: message.into(),
            span: Some(span),
        }
    }

    fn render(&self, src: &str, source_offset: usize) -> String {
        let Some(span) = self.span else {
            return self.message.clone();
        };

        let absolute_start = source_offset.saturating_add(span.start).min(src.len());
        let absolute_end = source_offset.saturating_add(span.end).min(src.len());
        let (line, column, line_start, line_end) = source_location(src, absolute_start);
        let line_text = src[line_start..line_end].trim_end_matches('\r');
        let marker_len = src[absolute_start..absolute_end]
            .chars()
            .take_while(|ch| *ch != '\n' && *ch != '\r')
            .count()
            .max(1);

        format!(
            "{} at line {}, column {}\n{}\n{}{}",
            self.message,
            line,
            column,
            line_text,
            " ".repeat(column.saturating_sub(1)),
            "^".repeat(marker_len)
        )
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

#[derive(Debug)]
struct UsagiShaderModule<'src> {
    tokens: Vec<Token<'src>>,
    source: ShaderSource,
    items: Vec<ShaderItem<'src>>,
    entrypoint_name: &'src str,
    source_offset: usize,
}

impl<'src> UsagiShaderModule<'src> {
    fn parse(src: &'src str) -> Result<Self, String> {
        let (body, source_offset) = shader_body(src);
        let mut module = Self::parse_body(body).map_err(|err| err.render(src, source_offset))?;
        module.source_offset = source_offset;
        Ok(module)
    }

    fn parse_body(body: &'src str) -> CompileResult<Self> {
        let tokens = lex(body)?;
        reject_version_directive(&tokens)?;
        reject_reserved_identifiers(&tokens)?;
        let items = parse_items(&tokens)?;
        validate_items(&items)?;
        let entrypoint_name = find_entrypoint(&items)?;
        let source = ShaderSource::parse(&tokens)?;
        Ok(Self {
            tokens,
            source,
            items,
            entrypoint_name,
            source_offset: 0,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SourceSpan {
    pub(super) start: usize,
    pub(super) end: usize,
}

impl SourceSpan {
    fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    fn join(self, other: Self) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    fn shifted(self, offset: usize) -> Self {
        Self {
            start: self.start + offset,
            end: self.end + offset,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Token<'src> {
    kind: TokenKind,
    text: &'src str,
    span: SourceSpan,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TokenKind {
    Ident,
    Number,
    Symbol,
    Whitespace,
    LineComment,
    BlockComment,
    PreprocessorLine,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ShaderItem<'src> {
    Function(FunctionDecl<'src>),
    Uniform(UniformDecl<'src>),
    Raw(SourceSpan),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FunctionDecl<'src> {
    return_type: &'src str,
    return_type_span: SourceSpan,
    name: &'src str,
    name_span: SourceSpan,
    params: Vec<ParamDecl<'src>>,
    span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ParamDecl<'src> {
    ty: &'src str,
    ty_span: SourceSpan,
    name: &'src str,
    name_span: SourceSpan,
    span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct UniformDecl<'src> {
    ty: &'src str,
    ty_span: SourceSpan,
    names: Vec<UniformName<'src>>,
    span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct UniformName<'src> {
    name: &'src str,
    span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ShaderSource {
    nodes: Vec<SourceNode>,
}

impl ShaderSource {
    fn parse(tokens: &[Token<'_>]) -> CompileResult<Self> {
        let mut nodes = Vec::with_capacity(tokens.len());
        let mut i = 0;
        while i < tokens.len() {
            let token = &tokens[i];
            if token.kind == TokenKind::Ident {
                match token.text {
                    "usagi_texture" => {
                        let call = parse_texture_intrinsic_call(tokens, i)?;
                        i = call.close_idx + 1;
                        nodes.push(SourceNode::IntrinsicCall(call));
                        continue;
                    }
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

            nodes.push(SourceNode::Token(i));
            i += 1;
        }

        Ok(Self { nodes })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SourceNode {
    Token(usize),
    IntrinsicCall(IntrinsicCall),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct IntrinsicCall {
    kind: IntrinsicKind,
    name_idx: usize,
    close_idx: usize,
    span: SourceSpan,
    args: Vec<CallArgument>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IntrinsicKind {
    Texture,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CallArgument {
    start_idx: usize,
    end_idx: usize,
    span: SourceSpan,
}

struct TargetEmitter {
    profile: ShaderProfile,
}

impl TargetEmitter {
    fn emit(&self, module: &UsagiShaderModule<'_>) -> Result<String, String> {
        let header = self.header();
        let footer = self.footer(module.entrypoint_name);
        let mut out = String::with_capacity(
            header.len() + source_len(&module.tokens) + footer.len() + module.items.len() * 2,
        );
        out.push_str(header);
        emit_source(&module.tokens, &module.source, self.profile, &mut out)?;
        out.push_str(&footer);
        Ok(out)
    }

    fn header(&self) -> &'static str {
        match self.profile {
            ShaderProfile::DesktopGlsl330 => {
                "#version 330\n\nin vec2 fragTexCoord;\nin vec4 fragColor;\nuniform sampler2D texture0;\nout vec4 finalColor;\n\n"
            }
            ShaderProfile::DesktopGlsl440 => {
                "#version 440 core\n\nin vec2 fragTexCoord;\nin vec4 fragColor;\nuniform sampler2D texture0;\nlayout(location = 0) out vec4 finalColor;\n\n"
            }
            ShaderProfile::WebGlslEs100 => {
                "#version 100\n\nprecision mediump float;\n\nvarying vec2 fragTexCoord;\nvarying vec4 fragColor;\nuniform sampler2D texture0;\n\n"
            }
        }
    }

    fn footer(&self, entrypoint: &str) -> String {
        match self.profile {
            ShaderProfile::DesktopGlsl330 | ShaderProfile::DesktopGlsl440 => {
                format!(
                    "\n\nvoid main() {{\n    finalColor = {entrypoint}(fragTexCoord, fragColor);\n}}\n"
                )
            }
            ShaderProfile::WebGlslEs100 => {
                format!(
                    "\n\nvoid main() {{\n    gl_FragColor = {entrypoint}(fragTexCoord, fragColor);\n}}\n"
                )
            }
        }
    }
}

fn emit_source(
    tokens: &[Token<'_>],
    source: &ShaderSource,
    profile: ShaderProfile,
    out: &mut String,
) -> Result<(), String> {
    for node in &source.nodes {
        match node {
            SourceNode::Token(idx) => out.push_str(tokens[*idx].text),
            SourceNode::IntrinsicCall(call) => emit_intrinsic_call(tokens, call, profile, out),
        }
    }
    Ok(())
}

fn emit_intrinsic_call(
    tokens: &[Token<'_>],
    call: &IntrinsicCall,
    profile: ShaderProfile,
    out: &mut String,
) {
    match call.kind {
        IntrinsicKind::Texture => out.push_str(texture_builtin(profile)),
    }
    for token in &tokens[call.name_idx + 1..=call.close_idx] {
        out.push_str(token.text);
    }
}

fn source_len(tokens: &[Token<'_>]) -> usize {
    tokens.iter().map(|token| token.text.len()).sum()
}

fn texture_builtin(profile: ShaderProfile) -> &'static str {
    match profile {
        ShaderProfile::DesktopGlsl330 | ShaderProfile::DesktopGlsl440 => "texture",
        ShaderProfile::WebGlslEs100 => "texture2D",
    }
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

fn source_location(src: &str, byte_idx: usize) -> (usize, usize, usize, usize) {
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

fn reject_reserved_identifiers(tokens: &[Token<'_>]) -> CompileResult<()> {
    for token in tokens {
        if token.kind != TokenKind::Ident {
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
            out.push(ShaderItem::Raw(SourceSpan::new(
                tokens[start].span.start,
                tokens[end].span.end,
            )));
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
                "[" => expect_name = false,
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

fn is_code_token(token: &Token<'_>) -> bool {
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

fn parse_texture_intrinsic_call(
    tokens: &[Token<'_>],
    name_idx: usize,
) -> CompileResult<IntrinsicCall> {
    let Some(open_idx) = next_code(tokens, name_idx + 1) else {
        return Err(CompileError::at(
            "usagi_texture must be called as usagi_texture(sampler, uv)",
            tokens[name_idx].span,
        ));
    };
    if token_text(tokens, open_idx) != Some("(") {
        return Err(CompileError::at(
            "usagi_texture must be called as usagi_texture(sampler, uv)",
            tokens[name_idx].span,
        ));
    }

    let close_idx = find_matching(tokens, open_idx, "(", ")").ok_or_else(|| {
        CompileError::at(
            "usagi_texture has an unterminated argument list",
            tokens[name_idx].span,
        )
    })?;
    let args = split_call_args(tokens, open_idx + 1, close_idx)?;
    if args.len() != 2 {
        return Err(CompileError::at(
            "usagi_texture expects 2 arguments: sampler and uv",
            SourceSpan::new(tokens[name_idx].span.start, tokens[close_idx].span.end),
        ));
    }

    Ok(IntrinsicCall {
        kind: IntrinsicKind::Texture,
        name_idx,
        close_idx,
        span: SourceSpan::new(tokens[name_idx].span.start, tokens[close_idx].span.end),
        args,
    })
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

    fn compile_fragment(src: &str, profile: ShaderProfile) -> Result<String, String> {
        compile_fragment_with_metadata(src, profile).map(|compiled| compiled.source)
    }

    #[test]
    fn compiler_lowers_texture_intrinsic_without_macros() {
        let src = "#usagi shader 1\nvec4 usagi_main(vec2 uv, vec4 color) {\n    return usagi_texture(texture0, uv) * color;\n}\n";
        let out = compile_fragment(src, ShaderProfile::DesktopGlsl330).unwrap();

        assert!(out.contains("#version 330"));
        assert!(out.contains("return texture(texture0, uv) * color;"));
        assert!(!out.contains("#define usagi_texture"));
        assert!(out.contains("finalColor = usagi_main(fragTexCoord, fragColor);"));
    }

    #[test]
    fn compiler_emits_glsl_es_100_texture2d() {
        let src = "vec4 usagi_main(vec2 uv, vec4 color) {\n    return usagi_texture(texture0, uv) * color;\n}\n";
        let out = compile_fragment(src, ShaderProfile::WebGlslEs100).unwrap();

        assert!(out.contains("#version 100"));
        assert!(out.contains("precision mediump float;"));
        assert!(out.contains("return texture2D(texture0, uv) * color;"));
        assert!(out.contains("gl_FragColor = usagi_main(fragTexCoord, fragColor);"));
    }

    #[test]
    fn compiler_has_forward_glsl_440_emitter() {
        let src = "vec4 usagi_main(vec2 uv, vec4 color) { return color; }\n";
        let out = compile_fragment(src, ShaderProfile::DesktopGlsl440).unwrap();

        assert!(out.contains("#version 440 core"));
        assert!(out.contains("layout(location = 0) out vec4 finalColor;"));
        assert!(out.contains("finalColor = usagi_main(fragTexCoord, fragColor);"));
    }

    #[test]
    fn parser_records_texture_intrinsic_as_call_node() {
        let src = "vec4 usagi_main(vec2 uv, vec4 color) { return usagi_texture(texture0, clamp(uv, 0.0, 1.0)); }\n";
        let module = UsagiShaderModule::parse(src).unwrap();
        let calls: Vec<_> = module
            .source
            .nodes
            .iter()
            .filter_map(|node| match node {
                SourceNode::IntrinsicCall(call) => Some(call),
                SourceNode::Token(_) => None,
            })
            .collect();

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].kind, IntrinsicKind::Texture);
        assert_eq!(calls[0].args.len(), 2);
        assert_eq!(module.tokens[calls[0].name_idx].text, "usagi_texture");
        assert_eq!(module.tokens[calls[0].args[0].start_idx].text, "texture0");
        assert_eq!(module.tokens[calls[0].args[1].start_idx].text, "clamp");
        assert_eq!(
            &src[calls[0].span.start..calls[0].span.end],
            "usagi_texture(texture0, clamp(uv, 0.0, 1.0))"
        );
    }

    #[test]
    fn parser_records_declaration_spans() {
        let src = "uniform float u_time;\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n";
        let module = UsagiShaderModule::parse(src).unwrap();
        let uniform = module
            .items
            .iter()
            .find_map(|item| match item {
                ShaderItem::Uniform(uniform) => Some(uniform),
                ShaderItem::Function(_) | ShaderItem::Raw(_) => None,
            })
            .unwrap();
        let function = module
            .items
            .iter()
            .find_map(|item| match item {
                ShaderItem::Function(function) => Some(function),
                ShaderItem::Uniform(_) | ShaderItem::Raw(_) => None,
            })
            .unwrap();

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
    fn compiler_errors_include_line_column_and_source_snippet() {
        let src = "#usagi shader 1\n\nvec4 usagi_main(vec2 uv, vec4 color) {\n    return texture(texture0, uv);\n}\n";
        let err = compile_fragment(src, ShaderProfile::DesktopGlsl330).unwrap_err();

        assert!(err.contains("line 4, column 12"));
        assert!(err.contains("return texture(texture0, uv);"));
        assert!(err.contains("           ^^^^^^^"));
    }

    #[test]
    fn compiler_semantic_errors_point_to_reserved_uniform_name() {
        let err = compile_fragment(
            "uniform sampler2D texture0;\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("line 1, column 19"));
        assert!(err.contains("uniform sampler2D texture0;"));
        assert!(err.contains("                  ^^^^^^^^"));
    }

    #[test]
    fn parser_rejects_texture_intrinsic_with_wrong_arity() {
        let err = compile_fragment(
            "vec4 usagi_main(vec2 uv, vec4 color) { return usagi_texture(texture0); }\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("usagi_texture expects 2 arguments"));
    }

    #[test]
    fn parser_rejects_empty_texture_intrinsic_arguments() {
        let err = compile_fragment(
            "vec4 usagi_main(vec2 uv, vec4 color) { return usagi_texture(texture0, uv,); }\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("argument cannot be empty"));
    }

    #[test]
    fn parser_rejects_target_specific_texture_calls() {
        let err = compile_fragment(
            "vec4 usagi_main(vec2 uv, vec4 color) { return texture(texture0, uv); }\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("usagi_texture"));
    }

    #[test]
    fn parser_rejects_reserved_engine_bindings() {
        let err = compile_fragment(
            "uniform sampler2D texture0;\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("texture0"));
    }

    #[test]
    fn parser_rejects_duplicate_entrypoint() {
        let err = compile_fragment(
            "vec4 usagi_main(vec2 uv, vec4 color) { return color; }\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("exactly one"));
    }

    #[test]
    fn parser_rejects_bad_entrypoint_signature() {
        let err = compile_fragment(
            "vec3 usagi_main(vec2 uv, vec4 color) { return color.rgb; }\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("signature"));
    }

    #[test]
    fn parser_rejects_entrypoint_with_wrong_parameter_names() {
        let err = compile_fragment(
            "vec4 usagi_main(vec2 texcoord, vec4 tint) { return tint; }\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("vec4 usagi_main(vec2 uv, vec4 color)"));
    }

    #[test]
    fn parser_rejects_function_parameters_without_commas() {
        let err = compile_fragment(
            "vec4 usagi_main(vec2 uv vec4 color) { return color; }\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("parameters must be separated by commas"));
    }

    #[test]
    fn parser_rejects_trailing_function_parameter_comma() {
        let err = compile_fragment(
            "vec4 usagi_main(vec2 uv, vec4 color,) { return color; }\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("function parameter cannot be empty"));
    }

    #[test]
    fn parser_rejects_unmatched_texture_intrinsic_delimiters() {
        let err = compile_fragment(
            "vec4 usagi_main(vec2 uv, vec4 color) { return usagi_texture(texture0, uv[0); }\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("unmatched delimiters"));
    }

    #[test]
    fn parser_rejects_unmatched_texture_intrinsic_closer() {
        let err = compile_fragment(
            "vec4 usagi_main(vec2 uv, vec4 color) { return usagi_texture(texture0, uv]); }\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("unmatched ']'"));
    }

    #[test]
    fn parser_ignores_comment_mentions_of_reserved_words() {
        let src = "// fragColor finalColor texture()\nvec4 usagi_main(vec2 uv, vec4 color) { return usagi_texture(texture0, uv); }\n";
        assert!(compile_fragment(src, ShaderProfile::DesktopGlsl330).is_ok());
    }
}
