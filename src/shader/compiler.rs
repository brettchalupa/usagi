//! Compiler for the `.usagi.fs` shader dialect.
//!
//! The dialect is intentionally close to fragment GLSL, but the source
//! is not passed through verbatim. We parse a small module ABT, validate
//! the engine-owned bindings, lower Usagi intrinsics, then emit
//! target-specific GLSL.

use super::ShaderProfile;

pub(super) fn compile_fragment(src: &str, profile: ShaderProfile) -> Result<String, String> {
    let module = UsagiShaderModule::parse(src)?;
    TargetEmitter { profile }.emit(&module)
}

#[derive(Debug)]
struct UsagiShaderModule {
    tokens: Vec<Token>,
    items: Vec<ShaderItem>,
    entrypoint: FunctionDecl,
}

impl UsagiShaderModule {
    fn parse(src: &str) -> Result<Self, String> {
        let body = strip_usagi_shader_directive(src).trim_start_matches('\n');
        let tokens = lex(body)?;
        reject_version_directive(&tokens)?;
        reject_reserved_identifiers(&tokens)?;
        let items = parse_items(&tokens)?;
        validate_items(&items)?;
        let entrypoint = find_entrypoint(&items)?;
        Ok(Self {
            tokens,
            items,
            entrypoint,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Token {
    kind: TokenKind,
    text: String,
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
enum ShaderItem {
    Function(FunctionDecl),
    Uniform(UniformDecl),
    Raw,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FunctionDecl {
    return_type: String,
    name: String,
    params: Vec<ParamDecl>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ParamDecl {
    ty: String,
    name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct UniformDecl {
    ty: String,
    names: Vec<String>,
}

struct TargetEmitter {
    profile: ShaderProfile,
}

impl TargetEmitter {
    fn emit(&self, module: &UsagiShaderModule) -> Result<String, String> {
        let header = self.header();
        let translated = translate_tokens(&module.tokens, self.profile)?;
        let footer = self.footer(&module.entrypoint.name);
        let mut out = String::with_capacity(
            header.len() + translated.len() + footer.len() + module.items.len() * 2,
        );
        out.push_str(header);
        out.push_str(&translated);
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

fn translate_tokens(tokens: &[Token], profile: ShaderProfile) -> Result<String, String> {
    let mut out = String::new();
    for token in tokens {
        if token.kind == TokenKind::Ident {
            match token.text.as_str() {
                "usagi_texture" => out.push_str(texture_builtin(profile)),
                "usagi_main" => out.push_str(&token.text),
                name if name.starts_with("usagi_") => {
                    return Err(format!("unknown Usagi shader intrinsic '{name}'"));
                }
                "texture" | "texture2D" => {
                    return Err(
                        "generic shaders must use usagi_texture(...) instead of target-specific texture functions"
                            .to_string(),
                    );
                }
                _ => out.push_str(&token.text),
            }
        } else {
            out.push_str(&token.text);
        }
    }
    Ok(out)
}

fn texture_builtin(profile: ShaderProfile) -> &'static str {
    match profile {
        ShaderProfile::DesktopGlsl330 | ShaderProfile::DesktopGlsl440 => "texture",
        ShaderProfile::WebGlslEs100 => "texture2D",
    }
}

fn strip_usagi_shader_directive(src: &str) -> &str {
    let mut start = 0;
    for line in src.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            start += line.len();
            continue;
        }
        if trimmed == "#usagi shader 1" {
            return &src[start + line.len()..];
        }
        return &src[start..];
    }
    &src[start..]
}

fn lex(src: &str) -> Result<Vec<Token>, String> {
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
            push_token(&mut tokens, TokenKind::Whitespace, &src[start..i]);
        } else if b == b'/' && bytes.get(i + 1) == Some(&b'/') {
            let start = i;
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            if i < bytes.len() {
                i += 1;
            }
            push_token(&mut tokens, TokenKind::LineComment, &src[start..i]);
        } else if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
            let start = i;
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            if i + 1 >= bytes.len() {
                return Err("unterminated block comment".to_string());
            }
            i += 2;
            push_token(&mut tokens, TokenKind::BlockComment, &src[start..i]);
        } else if b == b'#' {
            let start = i;
            i += 1;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            if i < bytes.len() {
                i += 1;
            }
            push_token(&mut tokens, TokenKind::PreprocessorLine, &src[start..i]);
        } else if is_ident_start(b) {
            let start = i;
            i += 1;
            while i < bytes.len() && is_ident_continue(bytes[i]) {
                i += 1;
            }
            push_token(&mut tokens, TokenKind::Ident, &src[start..i]);
        } else if b.is_ascii_digit()
            || (b == b'.' && bytes.get(i + 1).is_some_and(u8::is_ascii_digit))
        {
            let start = i;
            i += 1;
            while i < bytes.len() && is_number_continue(bytes[i]) {
                i += 1;
            }
            push_token(&mut tokens, TokenKind::Number, &src[start..i]);
        } else {
            let start = i;
            let ch = src[i..]
                .chars()
                .next()
                .ok_or_else(|| "invalid shader source character boundary".to_string())?;
            i += ch.len_utf8();
            push_token(&mut tokens, TokenKind::Symbol, &src[start..i]);
        }
    }
    Ok(tokens)
}

fn push_token(tokens: &mut Vec<Token>, kind: TokenKind, text: &str) {
    tokens.push(Token {
        kind,
        text: text.to_string(),
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

fn reject_version_directive(tokens: &[Token]) -> Result<(), String> {
    if tokens.iter().any(|t| {
        t.kind == TokenKind::PreprocessorLine && t.text.trim_start().starts_with("#version")
    }) {
        return Err("generic shaders must not declare #version".to_string());
    }
    Ok(())
}

fn reject_reserved_identifiers(tokens: &[Token]) -> Result<(), String> {
    for token in tokens {
        if token.kind != TokenKind::Ident {
            continue;
        }
        match token.text.as_str() {
            "fragTexCoord" | "fragColor" => {
                return Err(format!(
                    "'{}' is engine-owned; use the usagi_main parameters instead",
                    token.text
                ));
            }
            "finalColor" | "gl_FragColor" => {
                return Err(format!(
                    "'{}' is target output state owned by the Usagi shader emitter",
                    token.text
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

fn parse_items(tokens: &[Token]) -> Result<Vec<ShaderItem>, String> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(start) = next_code(tokens, i) {
        if token_text(tokens, start) == Some("uniform") {
            let end = find_top_level_semicolon(tokens, start)
                .ok_or_else(|| "unterminated uniform declaration".to_string())?;
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
            out.push(ShaderItem::Raw);
            i = end + 1;
        } else {
            return Err(format!(
                "could not parse top-level shader item near '{}'",
                token_text(tokens, start).unwrap_or("<unknown>")
            ));
        }
    }
    Ok(out)
}

fn parse_uniform(tokens: &[Token], start: usize, end: usize) -> Result<UniformDecl, String> {
    let code = code_tokens(tokens, start, end);
    if code.len() < 3 {
        return Err("uniform declaration must include a type and name".to_string());
    }
    let ty = code[1].text.clone();
    let mut names = Vec::new();
    let mut expect_name = true;
    for token in code.iter().skip(2) {
        match token.text.as_str() {
            "," => expect_name = true,
            "[" => expect_name = false,
            "]" => {}
            name if expect_name && token.kind == TokenKind::Ident => {
                names.push(name.to_string());
                expect_name = false;
            }
            _ => {}
        }
    }
    if names.is_empty() {
        return Err("uniform declaration must include at least one name".to_string());
    }
    Ok(UniformDecl { ty, names })
}

fn parse_function(tokens: &[Token], start: usize) -> Result<Option<(FunctionDecl, usize)>, String> {
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
    let close_paren = find_matching(tokens, open_paren, "(", ")")
        .ok_or_else(|| format!("function '{name}' has an unterminated parameter list"))?;
    let Some(open_brace) = next_code(tokens, close_paren + 1) else {
        return Ok(None);
    };
    if token_text(tokens, open_brace) != Some("{") {
        return Ok(None);
    }
    let close_brace = find_matching(tokens, open_brace, "{", "}")
        .ok_or_else(|| format!("function '{name}' has an unterminated body"))?;
    Ok(Some((
        FunctionDecl {
            return_type: ret.to_string(),
            name: name.to_string(),
            params: parse_params(tokens, open_paren + 1, close_paren)?,
        },
        close_brace + 1,
    )))
}

fn parse_params(tokens: &[Token], start: usize, end: usize) -> Result<Vec<ParamDecl>, String> {
    let code = code_tokens(tokens, start, end);
    if code.is_empty() {
        return Ok(Vec::new());
    }
    let mut params = Vec::new();
    let mut i = 0;
    while i < code.len() {
        if code[i].text == "," {
            i += 1;
            continue;
        }
        if i + 1 >= code.len() {
            return Err("function parameter must include type and name".to_string());
        }
        let ty = code[i].text.clone();
        let name = code[i + 1].text.clone();
        if code[i].kind != TokenKind::Ident || code[i + 1].kind != TokenKind::Ident {
            return Err("function parameter must use simple 'type name' syntax".to_string());
        }
        params.push(ParamDecl { ty, name });
        i += 2;
        if i < code.len() && code[i].text == "," {
            i += 1;
        }
    }
    Ok(params)
}

fn validate_items(items: &[ShaderItem]) -> Result<(), String> {
    for item in items {
        match item {
            ShaderItem::Function(function) if function.name == "main" => {
                return Err("generic shaders must not declare main(); Usagi emits it".to_string());
            }
            ShaderItem::Function(_) | ShaderItem::Raw => {}
            ShaderItem::Uniform(uniform) => {
                if uniform.names.iter().any(|name| name == "texture0") {
                    return Err(
                        "generic shaders must not declare texture0; Usagi binds it".to_string()
                    );
                }
            }
        }
    }
    Ok(())
}

fn find_entrypoint(items: &[ShaderItem]) -> Result<FunctionDecl, String> {
    let mut matches = items.iter().filter_map(|item| match item {
        ShaderItem::Function(function) if function.name == "usagi_main" => Some(function.clone()),
        _ => None,
    });
    let Some(entrypoint) = matches.next() else {
        return Err("generic shaders must define vec4 usagi_main(vec2 uv, vec4 color)".to_string());
    };
    if matches.next().is_some() {
        return Err("generic shaders must define exactly one usagi_main function".to_string());
    }
    if entrypoint.return_type != "vec4"
        || entrypoint.params.len() != 2
        || entrypoint.params[0].ty != "vec2"
        || entrypoint.params[1].ty != "vec4"
    {
        return Err(
            "usagi_main signature must be vec4 usagi_main(vec2 uv, vec4 color)".to_string(),
        );
    }
    Ok(entrypoint)
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

fn next_code(tokens: &[Token], start: usize) -> Option<usize> {
    tokens
        .iter()
        .enumerate()
        .skip(start)
        .find(|(_, t)| is_code_token(t))
        .map(|(i, _)| i)
}

fn code_tokens(tokens: &[Token], start: usize, end: usize) -> Vec<Token> {
    tokens[start..end]
        .iter()
        .filter(|t| is_code_token(t))
        .cloned()
        .collect()
}

fn is_code_token(token: &Token) -> bool {
    !matches!(
        token.kind,
        TokenKind::Whitespace
            | TokenKind::LineComment
            | TokenKind::BlockComment
            | TokenKind::PreprocessorLine
    )
}

fn token_text(tokens: &[Token], idx: usize) -> Option<&str> {
    tokens.get(idx).map(|t| t.text.as_str())
}

fn find_top_level_semicolon(tokens: &[Token], start: usize) -> Option<usize> {
    let mut paren = 0usize;
    let mut brace = 0usize;
    let mut bracket = 0usize;
    for (i, token) in tokens.iter().enumerate().skip(start) {
        if !is_code_token(token) {
            continue;
        }
        match token.text.as_str() {
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

fn find_matching(tokens: &[Token], open_idx: usize, open: &str, close: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, token) in tokens.iter().enumerate().skip(open_idx) {
        if !is_code_token(token) {
            continue;
        }
        match token.text.as_str() {
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn parser_ignores_comment_mentions_of_reserved_words() {
        let src = "// fragColor finalColor texture()\nvec4 usagi_main(vec2 uv, vec4 color) { return usagi_texture(texture0, uv); }\n";
        assert!(compile_fragment(src, ShaderProfile::DesktopGlsl330).is_ok());
    }
}
