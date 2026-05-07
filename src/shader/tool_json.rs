//! Shared JSON helpers for native shader tooling.

use super::ShaderProfile;
use super::compiler::ShaderDiagnostic;
use serde_json::{Map, Value, json};

pub(super) struct ToolDiagnostic<'a> {
    pub(super) kind: Option<&'static str>,
    pub(super) message: &'a str,
    pub(super) line: Option<usize>,
    pub(super) column: Option<usize>,
    pub(super) byte_start: Option<usize>,
    pub(super) byte_end: Option<usize>,
    pub(super) source_line: Option<&'a str>,
    pub(super) marker_len: Option<usize>,
}

impl<'a> ToolDiagnostic<'a> {
    pub(super) fn compiler(diagnostic: &'a ShaderDiagnostic) -> Self {
        Self {
            kind: None,
            message: &diagnostic.message,
            line: diagnostic.line,
            column: diagnostic.column,
            byte_start: diagnostic.byte_start,
            byte_end: diagnostic.byte_end,
            source_line: diagnostic.source_line.as_deref(),
            marker_len: diagnostic.marker_len,
        }
    }
}

pub(super) fn diagnostic_fields(diagnostic: ToolDiagnostic<'_>) -> Map<String, Value> {
    let mut fields = Map::new();
    if let Some(kind) = diagnostic.kind {
        fields.insert("kind".to_string(), json!(kind));
    }
    fields.insert("message".to_string(), json!(diagnostic.message));
    fields.insert("line".to_string(), json!(diagnostic.line));
    fields.insert("column".to_string(), json!(diagnostic.column));
    fields.insert("byte_start".to_string(), json!(diagnostic.byte_start));
    fields.insert("byte_end".to_string(), json!(diagnostic.byte_end));
    fields.insert("source_line".to_string(), json!(diagnostic.source_line));
    fields.insert("marker_len".to_string(), json!(diagnostic.marker_len));
    fields
}

pub(super) fn diagnostic_json(diagnostic: ToolDiagnostic<'_>) -> Value {
    Value::Object(diagnostic_fields(diagnostic))
}

pub(super) fn compiler_diagnostic_json(diagnostic: &ShaderDiagnostic) -> Value {
    diagnostic_json(ToolDiagnostic::compiler(diagnostic))
}

pub(super) fn profile_compiler_failure_json(
    profile: ShaderProfile,
    diagnostic: &ShaderDiagnostic,
) -> Value {
    json!({
        "profile": profile.label(),
        "diagnostic": compiler_diagnostic_json(diagnostic),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiler_diagnostic_json_preserves_span_fields() {
        let diagnostic = ShaderDiagnostic {
            message: "bad shader".to_string(),
            line: Some(2),
            column: Some(5),
            byte_start: Some(12),
            byte_end: Some(15),
            source_line: Some("return bad;".to_string()),
            marker_len: Some(3),
        };
        let value = compiler_diagnostic_json(&diagnostic);

        assert_eq!(value["message"], "bad shader");
        assert_eq!(value["line"], 2);
        assert_eq!(value["column"], 5);
        assert_eq!(value["byte_start"], 12);
        assert_eq!(value["byte_end"], 15);
        assert_eq!(value["source_line"], "return bad;");
        assert_eq!(value["marker_len"], 3);
    }
}
