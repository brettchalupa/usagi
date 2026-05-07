//! Minimal language-server surface for `.usagi.fs` editor tooling.
//!
//! This intentionally stays in-process with the existing Usagi CLI so it can
//! reuse the production shader compiler. The protocol surface is stdio LSP
//! with full-document sync and a small custom generated-GLSL request for the
//! future editor extension.

use super::ShaderProfile;
use super::compiler::{self, ShaderDiagnostic, ShaderInspection, ShaderSymbolKind};
use crate::{Error, Result};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{self, BufRead, BufWriter, Write};

const JSONRPC_VERSION: &str = "2.0";
const DIAGNOSTIC_SOURCE: &str = "usagi shader";
const GENERATED_GLSL_METHOD: &str = "usagi/generatedGlsl";

pub(crate) fn run() -> Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut server = ShaderLanguageServer::new();
    server.run_stdio(stdin.lock(), BufWriter::new(stdout.lock()))
}

#[derive(Debug)]
struct ShaderLanguageServer {
    documents: HashMap<String, String>,
    target: LspTarget,
    shutdown_requested: bool,
}

impl ShaderLanguageServer {
    fn new() -> Self {
        Self {
            documents: HashMap::new(),
            target: LspTarget::Desktop,
            shutdown_requested: false,
        }
    }

    fn run_stdio<R: BufRead, W: Write>(&mut self, mut input: R, mut output: W) -> Result<()> {
        while let Some(body) = read_lsp_message(&mut input)? {
            let message: Value = serde_json::from_slice(&body)
                .map_err(|e| Error::Cli(format!("lsp: invalid JSON message: {e}")))?;
            let outbound = self.handle_message(message);
            for response in outbound {
                write_lsp_message(&mut output, &response)?;
            }
            output
                .flush()
                .map_err(|e| Error::Cli(format!("lsp: flushing stdout: {e}")))?;
            if self.shutdown_requested {
                break;
            }
        }
        Ok(())
    }

    fn handle_message(&mut self, message: Value) -> Vec<Value> {
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return Vec::new();
        };
        let id = message.get("id").cloned();
        let params = message.get("params").unwrap_or(&Value::Null);

        match method {
            "initialize" => {
                self.target = target_from_initialization_options(params).unwrap_or(self.target);
                id.map(|id| response(id, initialize_result()))
                    .into_iter()
                    .collect()
            }
            "initialized" => Vec::new(),
            "shutdown" => {
                self.shutdown_requested = true;
                id.map(|id| response(id, Value::Null)).into_iter().collect()
            }
            "exit" => {
                self.shutdown_requested = true;
                Vec::new()
            }
            "textDocument/didOpen" => self.did_open(params),
            "textDocument/didChange" => self.did_change(params),
            "textDocument/didClose" => self.did_close(params),
            "textDocument/completion" => {
                self.with_request(id, params, |server, uri, _| server.completions_for_uri(uri))
            }
            "textDocument/hover" => self.with_request(id, params, |server, uri, params| {
                server.hover_for_uri(uri, params)
            }),
            "textDocument/signatureHelp" => self.with_request(id, params, |server, uri, params| {
                server.signature_help_for_uri(uri, params)
            }),
            "textDocument/documentSymbol" => self.with_request(id, params, |server, uri, _| {
                server.document_symbols_for_uri(uri)
            }),
            "textDocument/definition" => self.with_request(id, params, |server, uri, params| {
                server.definition_for_uri(uri, params)
            }),
            GENERATED_GLSL_METHOD => self.with_request(id, params, |server, uri, params| {
                server.generated_glsl_for_uri(uri, params)
            }),
            _ => id.map(|id| response(id, Value::Null)).into_iter().collect(),
        }
    }

    fn with_request(
        &mut self,
        id: Option<Value>,
        params: &Value,
        handler: impl FnOnce(&mut Self, &str, &Value) -> Value,
    ) -> Vec<Value> {
        let Some(id) = id else {
            return Vec::new();
        };
        let Some(uri) = text_document_uri(params) else {
            return vec![response(id, Value::Null)];
        };
        let result = handler(self, uri, params);
        vec![response(id, result)]
    }

    fn did_open(&mut self, params: &Value) -> Vec<Value> {
        let Some(uri) = text_document_uri(params) else {
            return Vec::new();
        };
        let text = params
            .pointer("/textDocument/text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        self.documents.insert(uri.to_string(), text);
        self.publish_diagnostics(uri)
    }

    fn did_change(&mut self, params: &Value) -> Vec<Value> {
        let Some(uri) = text_document_uri(params) else {
            return Vec::new();
        };
        let Some(text) = params
            .get("contentChanges")
            .and_then(Value::as_array)
            .and_then(|changes| changes.last())
            .and_then(|change| change.get("text"))
            .and_then(Value::as_str)
        else {
            return Vec::new();
        };
        self.documents.insert(uri.to_string(), text.to_string());
        self.publish_diagnostics(uri)
    }

    fn did_close(&mut self, params: &Value) -> Vec<Value> {
        let Some(uri) = text_document_uri(params) else {
            return Vec::new();
        };
        self.documents.remove(uri);
        vec![publish_diagnostics(uri, Vec::new())]
    }

    fn publish_diagnostics(&self, uri: &str) -> Vec<Value> {
        let Some(text) = self.documents.get(uri) else {
            return Vec::new();
        };
        vec![publish_diagnostics(
            uri,
            diagnostics_for_text(text, self.target),
        )]
    }

    fn completions_for_uri(&self, uri: &str) -> Value {
        let inspection = self
            .documents
            .get(uri)
            .and_then(|text| compiler::inspect_fragment(text).ok());
        completion_items(inspection.as_ref())
    }

    fn hover_for_uri(&self, uri: &str, params: &Value) -> Value {
        let Some(text) = self.documents.get(uri) else {
            return Value::Null;
        };
        let Some(position) = lsp_position(params) else {
            return Value::Null;
        };
        let byte = byte_offset_at_position(text, position.line, position.character);
        let Some(word) = word_at_byte(text, byte) else {
            return Value::Null;
        };
        hover_for_word(text, &word)
    }

    fn signature_help_for_uri(&self, uri: &str, params: &Value) -> Value {
        let Some(text) = self.documents.get(uri) else {
            return Value::Null;
        };
        let Some(position) = lsp_position(params) else {
            return Value::Null;
        };
        let byte = byte_offset_at_position(text, position.line, position.character);
        signature_help_at_byte(text, byte).unwrap_or(Value::Null)
    }

    fn document_symbols_for_uri(&self, uri: &str) -> Value {
        let Some(text) = self.documents.get(uri) else {
            return json!([]);
        };
        let Ok(inspection) = compiler::inspect_fragment(text) else {
            return json!([]);
        };
        json!(
            inspection
                .symbols
                .iter()
                .map(|symbol| {
                    json!({
                        "name": symbol.name,
                        "kind": lsp_symbol_kind(symbol.kind),
                        "detail": symbol.ty,
                        "range": range_for_span(text, symbol.declaration_span.start, symbol.declaration_span.end),
                        "selectionRange": range_for_span(text, symbol.name_span.start, symbol.name_span.end),
                    })
                })
                .collect::<Vec<_>>()
        )
    }

    fn definition_for_uri(&self, uri: &str, params: &Value) -> Value {
        let Some(text) = self.documents.get(uri) else {
            return Value::Null;
        };
        let Some(position) = lsp_position(params) else {
            return Value::Null;
        };
        let byte = byte_offset_at_position(text, position.line, position.character);
        let Some(word) = word_at_byte(text, byte) else {
            return Value::Null;
        };
        let Ok(inspection) = compiler::inspect_fragment(text) else {
            return Value::Null;
        };
        let Some(symbol) = inspection.symbols.iter().find(|symbol| symbol.name == word) else {
            return Value::Null;
        };
        json!({
            "uri": uri,
            "range": range_for_span(text, symbol.name_span.start, symbol.name_span.end),
        })
    }

    fn generated_glsl_for_uri(&self, uri: &str, params: &Value) -> Value {
        let Some(text) = self.documents.get(uri) else {
            return json!({
                "ok": false,
                "diagnostics": [{
                    "message": "document is not open",
                    "source": DIAGNOSTIC_SOURCE,
                }],
            });
        };
        let profile =
            target_profile_from_params(params).unwrap_or_else(|| self.target.profiles()[0]);
        match super::compile_generic_fragment_with_report(text, profile) {
            Ok(compiled) => json!({
                "ok": true,
                "profile": profile.label(),
                "source": compiled.source,
                "sourceMap": compiled.metadata.source_map.lines.iter().map(|line| {
                    json!({
                        "generatedLine": line.generated_line,
                        "sourceLine": line.source_line,
                        "kind": line.kind.as_str(),
                    })
                }).collect::<Vec<_>>(),
                "uniforms": compiled.metadata.uniforms.iter().map(|uniform| {
                    json!({
                        "name": uniform.name,
                        "type": uniform.ty,
                        "range": range_for_span(text, uniform.declaration_span.start, uniform.declaration_span.end),
                    })
                }).collect::<Vec<_>>(),
                "warnings": compiled.metadata.warnings.iter().map(|warning| {
                    lsp_warning(text, profile, warning)
                }).collect::<Vec<_>>(),
            }),
            Err(failure) => json!({
                "ok": false,
                "profile": profile.label(),
                "diagnostics": [lsp_diagnostic(text, profile, failure.diagnostic.as_ref())],
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LspTarget {
    Desktop,
    Web,
    All,
}

impl LspTarget {
    fn profiles(self) -> Vec<ShaderProfile> {
        match self {
            Self::Desktop => vec![ShaderProfile::DesktopGlsl330],
            Self::Web => vec![ShaderProfile::WebGlslEs100],
            Self::All => ShaderProfile::ALL.to_vec(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LspPosition {
    line: usize,
    character: usize,
}

fn initialize_result() -> Value {
    json!({
        "capabilities": {
            "textDocumentSync": 1,
            "completionProvider": {
                "triggerCharacters": ["u", "_", "."],
                "resolveProvider": false,
            },
            "hoverProvider": true,
            "signatureHelpProvider": {
                "triggerCharacters": ["(", ","],
            },
            "documentSymbolProvider": true,
            "definitionProvider": true,
            "executeCommandProvider": {
                "commands": [GENERATED_GLSL_METHOD],
            },
        },
        "serverInfo": {
            "name": "usagi-shader-lsp",
            "version": env!("CARGO_PKG_VERSION"),
        },
    })
}

fn diagnostics_for_text(text: &str, target: LspTarget) -> Vec<Value> {
    if let Err(failure) = compiler::inspect_fragment(text) {
        return vec![lsp_diagnostic(
            text,
            target.profiles()[0],
            failure.diagnostic.as_ref(),
        )];
    }

    let mut diagnostics = Vec::new();
    for profile in target.profiles() {
        match super::compile_generic_fragment_with_report(text, profile) {
            Ok(compiled) => {
                diagnostics.extend(
                    compiled
                        .metadata
                        .warnings
                        .iter()
                        .map(|warning| lsp_warning(text, profile, warning)),
                );
            }
            Err(failure) => {
                diagnostics.push(lsp_diagnostic(text, profile, failure.diagnostic.as_ref()));
            }
        }
    }
    diagnostics
}

fn lsp_diagnostic(text: &str, profile: ShaderProfile, diagnostic: &ShaderDiagnostic) -> Value {
    let range = match (diagnostic.byte_start, diagnostic.byte_end) {
        (Some(start), Some(end)) => range_for_span(text, start, end),
        _ => json!({
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 1 },
        }),
    };

    json!({
        "range": range,
        "severity": 1,
        "code": profile.label(),
        "source": DIAGNOSTIC_SOURCE,
        "message": format!("[{}] {}", profile.label(), diagnostic.message),
    })
}

fn lsp_warning(text: &str, profile: ShaderProfile, diagnostic: &ShaderDiagnostic) -> Value {
    let range = match (diagnostic.byte_start, diagnostic.byte_end) {
        (Some(start), Some(end)) => range_for_span(text, start, end),
        _ => json!({
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 1 },
        }),
    };

    json!({
        "range": range,
        "severity": 2,
        "code": profile.label(),
        "source": DIAGNOSTIC_SOURCE,
        "message": format!("[{}] {}", profile.label(), diagnostic.message),
    })
}

fn completion_items(inspection: Option<&ShaderInspection>) -> Value {
    let mut items = vec![
        json!({
            "label": "usagi_texture",
            "kind": 3,
            "detail": "vec4 usagi_texture(sampler2D sampler, vec2 uv)",
            "documentation": "Target-neutral texture sampling intrinsic.",
            "insertText": "usagi_texture(texture0, ${1:uv})",
            "insertTextFormat": 2,
        }),
        json!({
            "label": "texture0",
            "kind": 6,
            "detail": "sampler2D texture0",
            "documentation": "Engine-bound game render target sampler.",
        }),
        json!({
            "label": "usagi_main",
            "kind": 3,
            "detail": "vec4 usagi_main(vec2 uv, vec4 color)",
            "documentation": "Required generic fragment entrypoint.",
        }),
    ];

    for ty in [
        "float", "vec2", "vec3", "vec4", "bool", "int", "mat2", "mat3", "mat4",
    ] {
        items.push(json!({
            "label": ty,
            "kind": 14,
            "detail": "Usagi shader type",
        }));
    }

    if let Some(inspection) = inspection {
        for symbol in &inspection.symbols {
            items.push(json!({
                "label": symbol.name,
                "kind": match symbol.kind {
                    ShaderSymbolKind::Function => 3,
                    ShaderSymbolKind::Uniform => 6,
                },
                "detail": format!("{} {}", symbol.ty, symbol.name),
            }));
        }
    }

    json!({
        "isIncomplete": false,
        "items": items,
    })
}

fn hover_for_word(text: &str, word: &str) -> Value {
    let contents = match word {
        "usagi_texture" => {
            "```glsl\nvec4 usagi_texture(sampler2D sampler, vec2 uv)\n```\nTarget-neutral texture sampling. Emits `texture2D` for GLSL ES 100 and `texture` for desktop GLSL."
        }
        "texture0" => {
            "```glsl\nsampler2D texture0\n```\nEngine-bound sampler for the game render target. Do not declare it in `.usagi.fs`."
        }
        "usagi_main" => {
            "```glsl\nvec4 usagi_main(vec2 uv, vec4 color)\n```\nRequired entrypoint. Usagi emits the target-specific `main()` wrapper."
        }
        "fragTexCoord" | "fragColor" | "finalColor" | "gl_FragColor" | "main" => {
            "Reserved engine-owned shader binding. Use `usagi_main` parameters and Usagi intrinsics instead."
        }
        _ => return hover_for_symbol(text, word),
    };

    json!({
        "contents": {
            "kind": "markdown",
            "value": contents,
        },
    })
}

fn hover_for_symbol(text: &str, word: &str) -> Value {
    let Ok(inspection) = compiler::inspect_fragment(text) else {
        return Value::Null;
    };
    let Some(symbol) = inspection.symbols.iter().find(|symbol| symbol.name == word) else {
        return Value::Null;
    };
    let label = match symbol.kind {
        ShaderSymbolKind::Function => format!("{} {}(...)", symbol.ty, symbol.name),
        ShaderSymbolKind::Uniform => format!("uniform {} {};", symbol.ty, symbol.name),
    };
    json!({
        "contents": {
            "kind": "markdown",
            "value": format!("```glsl\n{label}\n```"),
        },
    })
}

fn signature_help_at_byte(text: &str, byte: usize) -> Option<Value> {
    let prefix = text.get(..byte.min(text.len()))?;
    let call_start = prefix.rfind("usagi_texture(")?;
    if !inside_open_call(&text[call_start..byte]) {
        return None;
    }
    let args = &text[call_start + "usagi_texture(".len()..byte];
    let active_parameter = usize::from(top_level_comma_count(args) > 0);

    Some(json!({
        "signatures": [{
            "label": "vec4 usagi_texture(sampler2D sampler, vec2 uv)",
            "documentation": {
                "kind": "markdown",
                "value": "Target-neutral texture read from the game render target."
            },
            "parameters": [
                { "label": "sampler2D sampler" },
                { "label": "vec2 uv" }
            ]
        }],
        "activeSignature": 0,
        "activeParameter": active_parameter,
    }))
}

fn inside_open_call(src: &str) -> bool {
    let mut depth = 0usize;
    for ch in src.chars() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth > 0
}

fn top_level_comma_count(src: &str) -> usize {
    let mut depth = 0usize;
    let mut count = 0usize;
    for ch in src.chars() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => count += 1,
            _ => {}
        }
    }
    count
}

fn publish_diagnostics(uri: &str, diagnostics: Vec<Value>) -> Value {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "method": "textDocument/publishDiagnostics",
        "params": {
            "uri": uri,
            "diagnostics": diagnostics,
        },
    })
}

fn response(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "result": result,
    })
}

fn text_document_uri(params: &Value) -> Option<&str> {
    params
        .pointer("/textDocument/uri")
        .or_else(|| params.pointer("/textDocumentIdentifier/uri"))
        .and_then(Value::as_str)
}

fn lsp_position(params: &Value) -> Option<LspPosition> {
    let line = params.pointer("/position/line")?.as_u64()? as usize;
    let character = params.pointer("/position/character")?.as_u64()? as usize;
    Some(LspPosition { line, character })
}

fn target_from_initialization_options(params: &Value) -> Option<LspTarget> {
    let options = params.get("initializationOptions")?;
    options
        .get("target")
        .or_else(|| options.get("targetProfile"))
        .and_then(Value::as_str)
        .and_then(parse_lsp_target)
}

fn target_profile_from_params(params: &Value) -> Option<ShaderProfile> {
    params
        .get("target")
        .or_else(|| params.get("targetProfile"))
        .and_then(Value::as_str)
        .and_then(parse_shader_profile)
}

fn parse_lsp_target(value: &str) -> Option<LspTarget> {
    match value.to_ascii_lowercase().as_str() {
        "desktop" | "glsl330" | "glsl-330" | "330" => Some(LspTarget::Desktop),
        "web" | "es100" | "glsl-es-100" | "100" => Some(LspTarget::Web),
        "all" => Some(LspTarget::All),
        _ => None,
    }
}

fn parse_shader_profile(value: &str) -> Option<ShaderProfile> {
    match value.to_ascii_lowercase().as_str() {
        "desktop" | "glsl330" | "glsl-330" | "330" => Some(ShaderProfile::DesktopGlsl330),
        "web" | "es100" | "glsl-es-100" | "100" => Some(ShaderProfile::WebGlslEs100),
        "glsl440" | "glsl-440" | "440" => Some(ShaderProfile::DesktopGlsl440),
        _ => None,
    }
}

fn lsp_symbol_kind(kind: ShaderSymbolKind) -> u8 {
    match kind {
        ShaderSymbolKind::Function => 12,
        ShaderSymbolKind::Uniform => 13,
    }
}

fn range_for_span(text: &str, start: usize, end: usize) -> Value {
    let start = byte_position(text, start);
    let end = byte_position(text, end);
    json!({
        "start": { "line": start.line, "character": start.character },
        "end": { "line": end.line, "character": end.character },
    })
}

fn byte_position(text: &str, byte: usize) -> LspPosition {
    let byte = byte.min(text.len());
    let mut line = 0usize;
    let mut line_start = 0usize;
    for (idx, ch) in text.char_indices() {
        if idx >= byte {
            break;
        }
        if ch == '\n' {
            line += 1;
            line_start = idx + ch.len_utf8();
        }
    }
    LspPosition {
        line,
        character: text[line_start..byte].chars().count(),
    }
}

fn byte_offset_at_position(text: &str, target_line: usize, target_character: usize) -> usize {
    let mut line = 0usize;
    let mut line_start = 0usize;
    for (idx, ch) in text.char_indices() {
        if line == target_line {
            break;
        }
        if ch == '\n' {
            line += 1;
            line_start = idx + ch.len_utf8();
        }
    }

    if line != target_line {
        return text.len();
    }

    text[line_start..]
        .char_indices()
        .take(target_character)
        .last()
        .map_or(line_start, |(idx, ch)| line_start + idx + ch.len_utf8())
        .min(text.len())
}

fn word_at_byte(text: &str, byte: usize) -> Option<String> {
    if text.is_empty() {
        return None;
    }
    let byte = byte.min(text.len());
    let mut start = byte;
    while start > 0 {
        let ch = text[..start].chars().next_back()?;
        if !is_word_char(ch) {
            break;
        }
        start -= ch.len_utf8();
    }

    let mut end = byte;
    while end < text.len() {
        let ch = text[end..].chars().next()?;
        if !is_word_char(ch) {
            break;
        }
        end += ch.len_utf8();
    }

    (start < end).then(|| text[start..end].to_string())
}

fn is_word_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn read_lsp_message<R: BufRead>(input: &mut R) -> Result<Option<Vec<u8>>> {
    let mut content_length = None;
    let mut line = String::new();

    loop {
        line.clear();
        let n = input
            .read_line(&mut line)
            .map_err(|e| Error::Cli(format!("lsp: reading header: {e}")))?;
        if n == 0 {
            return Ok(None);
        }
        let header = line.trim_end_matches(['\r', '\n']);
        if header.is_empty() {
            break;
        }
        if let Some(value) = header.strip_prefix("Content-Length:") {
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|e| Error::Cli(format!("lsp: invalid Content-Length: {e}")))?,
            );
        }
    }

    let Some(content_length) = content_length else {
        return Err(Error::Cli("lsp: missing Content-Length header".to_string()));
    };
    let mut body = vec![0u8; content_length];
    input
        .read_exact(&mut body)
        .map_err(|e| Error::Cli(format!("lsp: reading body: {e}")))?;
    Ok(Some(body))
}

fn write_lsp_message<W: Write>(output: &mut W, value: &Value) -> Result<()> {
    let body = value.to_string();
    write!(output, "Content-Length: {}\r\n\r\n{}", body.len(), body)
        .map_err(|e| Error::Cli(format!("lsp: writing response: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const URI: &str = "file:///project/shaders/crt.usagi.fs";
    const VALID_SHADER: &str = concat!(
        "#usagi shader 1\n\n",
        "uniform float u_time;\n",
        "vec4 helper(vec2 uv) { return usagi_texture(texture0, uv); }\n",
        "vec4 usagi_main(vec2 uv, vec4 color) { return helper(uv) * color * u_time; }\n",
    );

    fn request(id: i32, method: &str, params: Value) -> Value {
        json!({
            "jsonrpc": JSONRPC_VERSION,
            "id": id,
            "method": method,
            "params": params,
        })
    }

    fn notification(method: &str, params: Value) -> Value {
        json!({
            "jsonrpc": JSONRPC_VERSION,
            "method": method,
            "params": params,
        })
    }

    #[test]
    fn initialize_advertises_core_shader_editor_features() {
        let mut server = ShaderLanguageServer::new();
        let responses = server.handle_message(request(
            1,
            "initialize",
            json!({ "initializationOptions": { "target": "all" } }),
        ));

        assert_eq!(responses.len(), 1);
        let capabilities = &responses[0]["result"]["capabilities"];
        assert_eq!(capabilities["hoverProvider"], true);
        assert_eq!(capabilities["definitionProvider"], true);
        assert_eq!(
            capabilities["executeCommandProvider"]["commands"][0],
            GENERATED_GLSL_METHOD
        );
        assert_eq!(server.target, LspTarget::All);
    }

    #[test]
    fn did_open_publishes_compiler_diagnostic() {
        let mut server = ShaderLanguageServer::new();
        let responses = server.handle_message(notification(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": URI,
                    "text": "vec4 usagi_main(vec2 uv, vec4 color) { return texture(texture0, uv); }\n",
                }
            }),
        ));

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0]["method"], "textDocument/publishDiagnostics");
        assert_eq!(
            responses[0]["params"]["diagnostics"][0]["source"],
            DIAGNOSTIC_SOURCE
        );
        assert!(
            responses[0]["params"]["diagnostics"][0]["message"]
                .as_str()
                .unwrap()
                .contains("usagi_texture")
        );
    }

    #[test]
    fn completion_includes_intrinsics_and_document_symbols() {
        let mut server = ShaderLanguageServer::new();
        server
            .documents
            .insert(URI.to_string(), VALID_SHADER.to_string());
        let responses = server.handle_message(request(
            2,
            "textDocument/completion",
            json!({ "textDocument": { "uri": URI }, "position": { "line": 4, "character": 5 } }),
        ));
        let labels: Vec<_> = responses[0]["result"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["label"].as_str().unwrap())
            .collect();

        assert!(labels.contains(&"usagi_texture"));
        assert!(labels.contains(&"u_time"));
        assert!(labels.contains(&"helper"));
    }

    #[test]
    fn document_symbols_include_uniforms_and_functions() {
        let mut server = ShaderLanguageServer::new();
        server
            .documents
            .insert(URI.to_string(), VALID_SHADER.to_string());
        let responses = server.handle_message(request(
            3,
            "textDocument/documentSymbol",
            json!({ "textDocument": { "uri": URI } }),
        ));
        let symbols = responses[0]["result"].as_array().unwrap();
        let names: Vec<_> = symbols
            .iter()
            .map(|symbol| symbol["name"].as_str().unwrap())
            .collect();

        assert!(names.contains(&"u_time"));
        assert!(names.contains(&"helper"));
        assert!(names.contains(&"usagi_main"));
    }

    #[test]
    fn generated_glsl_request_returns_selected_profile_source() {
        let mut server = ShaderLanguageServer::new();
        server
            .documents
            .insert(URI.to_string(), VALID_SHADER.to_string());
        let responses = server.handle_message(request(
            4,
            GENERATED_GLSL_METHOD,
            json!({ "textDocument": { "uri": URI }, "target": "web" }),
        ));

        assert_eq!(responses[0]["result"]["ok"], true);
        assert_eq!(responses[0]["result"]["profile"], "GLSL ES 100");
        assert!(
            responses[0]["result"]["source"]
                .as_str()
                .unwrap()
                .contains("texture2D(texture0, uv)")
        );
        assert!(
            responses[0]["result"]["sourceMap"]
                .as_array()
                .unwrap()
                .iter()
                .any(|line| line["sourceLine"] == 4)
        );
        assert_eq!(responses[0]["result"]["warnings"], serde_json::json!([]));
    }

    #[test]
    fn did_open_publishes_compiler_warnings() {
        let mut server = ShaderLanguageServer::new();
        let responses = server.handle_message(notification(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": URI,
                    "text": concat!(
                        "vec4 usagi_main(vec2 uv, vec4 color) {\n",
                        "    vec4 a = usagi_texture(texture0, uv);\n",
                        "    vec4 b = usagi_texture(texture0, uv);\n",
                        "    return a + b;\n",
                        "}\n",
                    ),
                }
            }),
        ));

        assert_eq!(responses.len(), 1);
        let diagnostic = &responses[0]["params"]["diagnostics"][0];
        assert_eq!(diagnostic["severity"], 2);
        assert!(
            diagnostic["message"]
                .as_str()
                .unwrap()
                .contains("duplicate usagi_texture")
        );
    }

    #[test]
    fn lsp_message_framing_round_trips_json() {
        let inbound = request(5, "initialize", json!({}));
        let body = inbound.to_string();
        let raw = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        let mut cursor = Cursor::new(raw.into_bytes());

        let parsed = read_lsp_message(&mut cursor).unwrap().unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&parsed).unwrap(), inbound);

        let mut outbound = Vec::new();
        write_lsp_message(&mut outbound, &response(json!(5), Value::Null)).unwrap();
        let outbound = String::from_utf8(outbound).unwrap();
        assert!(outbound.starts_with("Content-Length: "));
        assert!(outbound.contains("\r\n\r\n"));
    }
}
