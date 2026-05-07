//! Raylib shader-driver log capture and source remapping.

use super::compiler::ShaderMetadata;
use sola_raylib::prelude::*;

#[cfg(not(target_os = "emscripten"))]
use std::sync::{Mutex, Once, OnceLock};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct RaylibShaderLog {
    entries: Vec<RaylibLogEntry>,
}

impl RaylibShaderLog {
    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(super) fn format_remapped(
        &self,
        shader_key: &str,
        metadata: Option<&ShaderMetadata>,
    ) -> String {
        if self.entries.is_empty() {
            return String::new();
        }

        let mut out = String::new();
        for entry in &self.entries {
            out.push_str("\n  [driver-log] ");
            out.push_str(entry.level.label());
            out.push_str(": ");
            out.push_str(&remap_driver_log_line(&entry.message, shader_key, metadata));
        }
        out
    }

    pub(super) fn forward(&self) {
        for entry in &self.entries {
            forward_raylib_log(entry.level, &entry.message);
        }
    }

    #[cfg(test)]
    pub(super) fn from_entries(entries: Vec<(&str, RaylibLogLevel)>) -> Self {
        Self {
            entries: entries
                .into_iter()
                .map(|(message, level)| RaylibLogEntry {
                    level,
                    message: message.to_string(),
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RaylibLogEntry {
    level: RaylibLogLevel,
    message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(
    dead_code,
    reason = "all variants are populated by the native raylib trace callback; web keeps the type for an empty log object"
)]
pub(super) enum RaylibLogLevel {
    Trace,
    Debug,
    Info,
    Warning,
    Error,
    Fatal,
    None,
}

impl RaylibLogLevel {
    fn label(self) -> &'static str {
        match self {
            Self::Trace => "TRACE",
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warning => "WARNING",
            Self::Error => "ERROR",
            Self::Fatal => "FATAL",
            Self::None => "NONE",
        }
    }
}

pub(super) fn load_shader_from_memory_with_log(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    vs_src: Option<&str>,
    fs_src: Option<&str>,
) -> (Shader, RaylibShaderLog) {
    #[cfg(not(target_os = "emscripten"))]
    {
        capture_raylib_logs(|| rl.load_shader_from_memory(thread, vs_src, fs_src))
    }

    #[cfg(target_os = "emscripten")]
    {
        (
            rl.load_shader_from_memory(thread, vs_src, fs_src),
            RaylibShaderLog::default(),
        )
    }
}

#[cfg(not(target_os = "emscripten"))]
fn capture_raylib_logs<T>(load: impl FnOnce() -> T) -> (T, RaylibShaderLog) {
    ensure_trace_callback();
    let state = trace_state();
    let _capture_guard = state.capture_lock.lock().unwrap_or_else(|e| e.into_inner());
    {
        let mut entries = state.entries.lock().unwrap_or_else(|e| e.into_inner());
        *entries = Some(Vec::new());
    }

    let result = load();
    let entries = state
        .entries
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .unwrap_or_default();
    (result, RaylibShaderLog { entries })
}

#[cfg(not(target_os = "emscripten"))]
struct RaylibTraceState {
    capture_lock: Mutex<()>,
    entries: Mutex<Option<Vec<RaylibLogEntry>>>,
}

#[cfg(not(target_os = "emscripten"))]
fn trace_state() -> &'static RaylibTraceState {
    static STATE: OnceLock<RaylibTraceState> = OnceLock::new();
    STATE.get_or_init(|| RaylibTraceState {
        capture_lock: Mutex::new(()),
        entries: Mutex::new(None),
    })
}

#[cfg(not(target_os = "emscripten"))]
fn ensure_trace_callback() {
    static INIT: Once = Once::new();
    let _ = trace_state();
    INIT.call_once(|| {
        if let Err(e) = set_trace_log_callback(raylib_trace_callback) {
            crate::msg::warn!("raylib trace capture unavailable: {e}");
        }
    });
}

#[cfg(not(target_os = "emscripten"))]
fn raylib_trace_callback(level: TraceLogLevel, text: &str) {
    let entry = RaylibLogEntry {
        level: RaylibLogLevel::from(level),
        message: text.to_string(),
    };
    let state = trace_state();
    let mut entries = state.entries.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(entries) = entries.as_mut() {
        entries.push(entry);
        return;
    }
    drop(entries);
    forward_raylib_log(entry.level, &entry.message);
}

#[cfg(not(target_os = "emscripten"))]
impl From<TraceLogLevel> for RaylibLogLevel {
    fn from(level: TraceLogLevel) -> Self {
        match level {
            TraceLogLevel::LOG_TRACE => Self::Trace,
            TraceLogLevel::LOG_DEBUG => Self::Debug,
            TraceLogLevel::LOG_INFO => Self::Info,
            TraceLogLevel::LOG_WARNING => Self::Warning,
            TraceLogLevel::LOG_ERROR => Self::Error,
            TraceLogLevel::LOG_FATAL => Self::Fatal,
            TraceLogLevel::LOG_ALL => Self::Trace,
            TraceLogLevel::LOG_NONE => Self::None,
        }
    }
}

fn forward_raylib_log(level: RaylibLogLevel, message: &str) {
    eprintln!("{}: {}", level.label(), message);
}

fn remap_driver_log_line(
    message: &str,
    shader_key: &str,
    metadata: Option<&ShaderMetadata>,
) -> String {
    let Some(metadata) = metadata else {
        return message.to_string();
    };
    let mappings = generated_line_refs(message)
        .into_iter()
        .filter_map(|generated_line| {
            if let Some(source_line) = metadata
                .source_map
                .original_line_for_generated_line(generated_line)
            {
                return Some(format!(
                    "generated line {generated_line} -> {shader_key}:{source_line}"
                ));
            }
            metadata
                .source_map
                .original_source_line_range()
                .filter(|(first, last)| (*first..=*last).contains(&generated_line))
                .map(|_| format!("source line {generated_line} -> {shader_key}:{generated_line}"))
        })
        .collect::<Vec<_>>();

    if mappings.is_empty() {
        message.to_string()
    } else {
        format!("{} [{}]", message, mappings.join(", "))
    }
}

fn generated_line_refs(message: &str) -> Vec<usize> {
    let mut refs = Vec::new();
    collect_parenthesized_line_refs(message, &mut refs);
    collect_colon_line_refs(message, &mut refs);
    refs.sort_unstable();
    refs.dedup();
    refs
}

fn collect_parenthesized_line_refs(message: &str, refs: &mut Vec<usize>) {
    let bytes = message.as_bytes();
    let mut i = 0usize;
    while i + 3 < bytes.len() {
        if !bytes[i].is_ascii_digit() || bytes[i + 1] != b'(' {
            i += 1;
            continue;
        }
        let start = i + 2;
        let mut end = start;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        if end > start
            && end < bytes.len()
            && bytes[end] == b')'
            && is_token_boundary(bytes, i.saturating_sub(1))
            && bytes.get(i.saturating_sub(1)) != Some(&b':')
            && let Ok(line) = message[start..end].parse::<usize>()
        {
            refs.push(line);
        }
        i = end.saturating_add(1);
    }
}

fn collect_colon_line_refs(message: &str, refs: &mut Vec<usize>) {
    let bytes = message.as_bytes();
    let mut i = 0usize;
    while i + 2 < bytes.len() {
        if bytes[i] != b':' || !bytes[i + 1].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i + 1;
        let mut end = start;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        if end < bytes.len()
            && matches!(bytes[end], b':' | b'(')
            && let Ok(line) = message[start..end].parse::<usize>()
        {
            refs.push(line);
        }
        i = end.saturating_add(1);
    }
}

fn is_token_boundary(bytes: &[u8], idx: usize) -> bool {
    bytes
        .get(idx)
        .is_none_or(|byte| !byte.is_ascii_alphanumeric() && *byte != b'_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shader::ShaderProfile;
    use crate::shader::compiler::{
        ShaderMetadata, ShaderSourceMap, ShaderSourceMapLine, ShaderSourceMapLineKind,
    };

    fn metadata() -> ShaderMetadata {
        ShaderMetadata {
            profile: ShaderProfile::DesktopGlsl330,
            uniforms: Vec::new(),
            warnings: Vec::new(),
            source_map: ShaderSourceMap {
                lines: vec![
                    ShaderSourceMapLine {
                        generated_line: 1,
                        source_line: None,
                        kind: ShaderSourceMapLineKind::Generated,
                    },
                    ShaderSourceMapLine {
                        generated_line: 8,
                        source_line: Some(3),
                        kind: ShaderSourceMapLineKind::Source,
                    },
                    ShaderSourceMapLine {
                        generated_line: 9,
                        source_line: Some(4),
                        kind: ShaderSourceMapLineKind::Source,
                    },
                ],
            },
        }
    }

    #[test]
    fn driver_log_parser_extracts_common_glsl_line_formats() {
        assert_eq!(
            generated_line_refs("SHADER: [ID 7] Compile error: 0(9) : error C0000"),
            vec![9]
        );
        assert_eq!(
            generated_line_refs("ERROR: 0:8: 'missing' : undeclared identifier"),
            vec![8]
        );
        assert_eq!(
            generated_line_refs("0:9(12): error: syntax error, unexpected IDENTIFIER"),
            vec![9]
        );
    }

    #[test]
    fn remap_driver_log_line_adds_source_location_for_generated_line() {
        let line = remap_driver_log_line(
            "ERROR: 0:9: 'missing' : undeclared identifier",
            "shaders/crt.usagi.fs",
            Some(&metadata()),
        );

        assert!(line.contains("generated line 9 -> shaders/crt.usagi.fs:4"));
    }

    #[test]
    fn remap_driver_log_line_accepts_line_directive_source_line() {
        let line = remap_driver_log_line(
            "ERROR: 0:4: 'missing' : undeclared identifier",
            "shaders/crt.usagi.fs",
            Some(&metadata()),
        );

        assert!(line.contains("source line 4 -> shaders/crt.usagi.fs:4"));
    }

    #[test]
    fn formatted_log_keeps_actual_driver_text_and_remap() {
        let log = RaylibShaderLog::from_entries(vec![(
            "SHADER: [ID 7] Compile error: 0(8) : error C0000",
            RaylibLogLevel::Warning,
        )]);
        let formatted = log.format_remapped("shaders/crt.usagi.fs", Some(&metadata()));

        assert!(formatted.contains("[driver-log] WARNING: SHADER: [ID 7] Compile error"));
        assert!(formatted.contains("generated line 8 -> shaders/crt.usagi.fs:3"));
    }
}
