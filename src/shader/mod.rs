//! Post-process shader support.
//!
//! Games declare a fragment shader that runs as a final pass on the
//! game render target as it's blitted to the window. Lua API:
//!
//! - `gfx.shader_set("name")` activates `shaders/<name>.usagi.fs`,
//!   or falls back to native `shaders/<name>.fs` / `<name>_es.fs`
//!   plus an optional `<name>.vs`. Pass `nil` to clear.
//! - `gfx.shader_uniform(name, value)` queues a uniform write. Number
//!   maps to float; 2/3/4-length tables map to vec2/vec3/vec4.
//!
//! Lua closures only enqueue requests against this manager. The
//! session drains them once per frame (where `&mut RaylibHandle` is
//! available) so that GPU resource creation and uniform writes happen
//! on the render thread.

#[cfg(not(target_os = "emscripten"))]
pub(crate) mod check_cli;
mod compiler;
mod driver_log;
#[cfg(not(target_os = "emscripten"))]
pub(crate) mod emit_cli;
#[cfg(not(target_os = "emscripten"))]
pub(crate) mod inspect_cli;
#[cfg(not(target_os = "emscripten"))]
pub(crate) mod lsp;
#[cfg(not(target_os = "emscripten"))]
pub(crate) mod profile_cli;
#[cfg(not(target_os = "emscripten"))]
pub(crate) use compiler::compile_fragment_with_report as compile_generic_fragment_with_report;

use crate::vfs::VirtualFs;
use sola_raylib::prelude::*;
use std::collections::{HashMap, HashSet, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::time::SystemTime;

const GENERIC_SHADER_COMPILER_CACHE_LIMIT: usize = 64;

/// One uniform value. Covers the types the Lua bridge can express
/// from a number or a small numeric table.
#[derive(Clone, Copy, Debug)]
pub enum ShaderValue {
    Float(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
}

impl ShaderValue {
    fn apply(self, shader: &mut Shader, loc: i32) {
        if loc < 0 {
            return;
        }
        match self {
            ShaderValue::Float(v) => shader.set_shader_value(loc, v),
            ShaderValue::Vec2(v) => shader.set_shader_value(loc, v),
            ShaderValue::Vec3(v) => shader.set_shader_value(loc, v),
            ShaderValue::Vec4(v) => shader.set_shader_value(loc, v),
        }
    }
}

/// Resolved active shader plus the metadata needed for live reload
/// and uniform replay across reloads.
struct Active {
    name: String,
    shader: Shader,
    /// Bundle/vfs key the fragment source was read from. May be
    /// `<name>.usagi.fs`, `<name>.fs`, or `<name>_es.fs`.
    fs_key: String,
    /// Same for the optional vertex source.
    vs_key: Option<String>,
    fs_mtime: Option<SystemTime>,
    vs_mtime: Option<SystemTime>,
    /// Last value seen for each uniform name. Replayed on live reload
    /// so the new shader picks up where the old one left off without
    /// the user having to call `shader_uniform` again.
    last_uniforms: HashMap<String, ShaderValue>,
    metadata: Option<compiler::ShaderMetadata>,
    uniform_types: HashMap<String, String>,
    reported_uniform_type_errors: HashSet<String>,
}

#[derive(Default)]
pub struct ShaderManager {
    /// Outer Some = a request is pending. Inner Some(name) = activate
    /// that shader; inner None = clear.
    pending_shader: Option<Option<String>>,
    pending_uniforms: Vec<(String, ShaderValue)>,
    active: Option<Active>,
    generic_compiler_cache: GenericShaderCompilerCache,
}

impl ShaderManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request_set(&mut self, name: Option<String>) {
        self.pending_shader = Some(name);
    }

    pub fn queue_uniform(&mut self, name: String, value: ShaderValue) {
        self.pending_uniforms.push((name, value));
    }

    /// Drains pending requests. Call once per frame, before drawing.
    pub fn apply_pending(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        vfs: &dyn VirtualFs,
    ) {
        if let Some(req) = self.pending_shader.take() {
            match req {
                Some(name) if !self.is_same_active_shader(&name) => {
                    self.load(rl, thread, vfs, &name, LoadPolicy::Request);
                }
                Some(_) => {}
                None => self.active = None,
            }
        }
        if let Some(active) = self.active.as_mut() {
            for (uname, value) in self.pending_uniforms.drain(..) {
                if active.metadata.is_some() {
                    if let Some(expected_ty) = active.uniform_types.get(uname.as_str()) {
                        if !shader_uniform_accepts_value(expected_ty, value) {
                            if active.reported_uniform_type_errors.insert(uname.clone()) {
                                crate::msg::err!(
                                    "shader '{}': uniform '{}' expects {}, got {}; write ignored",
                                    active.name,
                                    uname,
                                    expected_ty,
                                    shader_value_kind(value)
                                );
                            }
                            continue;
                        }
                        active.reported_uniform_type_errors.remove(&uname);
                    } else {
                        active.reported_uniform_type_errors.remove(uname.as_str());
                    }
                }

                let loc = active.shader.get_shader_location(&uname);
                value.apply(&mut active.shader, loc);
                active.last_uniforms.insert(uname, value);
            }
        } else {
            // No shader bound: drop queued writes so they don't leak
            // forward and apply to a different shader later.
            self.pending_uniforms.clear();
        }
    }

    /// Re-reads source from the vfs if either the fragment or vertex
    /// mtime has moved. Replays cached uniforms onto the rebuilt shader.
    pub fn reload_if_changed(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        vfs: &dyn VirtualFs,
    ) -> bool {
        let Some(active) = self.active.as_ref() else {
            return false;
        };
        let fs_mtime = file_mtime(vfs, &active.fs_key);
        let vs_mtime = active.vs_key.as_deref().and_then(|k| file_mtime(vfs, k));
        if fs_mtime == active.fs_mtime && vs_mtime == active.vs_mtime {
            return false;
        }
        let name = active.name.clone();
        let cached = active.last_uniforms.clone();
        if self.load(rl, thread, vfs, &name, LoadPolicy::Reload) != LoadOutcome::Loaded {
            return false;
        }
        if let Some(active) = self.active.as_mut() {
            let mut replayed = HashMap::with_capacity(cached.len());
            for (uname, value) in &cached {
                if active.metadata.is_some()
                    && let Some(expected_ty) = active.uniform_types.get(uname.as_str())
                    && !shader_uniform_accepts_value(expected_ty, *value)
                {
                    if active.reported_uniform_type_errors.insert(uname.clone()) {
                        crate::msg::err!(
                            "shader '{}': uniform '{}' expects {}, got {}; replay ignored",
                            active.name,
                            uname,
                            expected_ty,
                            shader_value_kind(*value)
                        );
                    }
                    continue;
                }

                let loc = active.shader.get_shader_location(uname);
                value.apply(&mut active.shader, loc);
                replayed.insert(uname.clone(), *value);
            }
            active.last_uniforms = replayed;
        }
        true
    }

    fn is_same_active_shader(&self, name: &str) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| active.name == name)
    }

    /// Returns the currently-bound shader, if any. Used by the blit
    /// path to wrap the RT draw in `begin_shader_mode`.
    pub fn active_shader_mut(&mut self) -> Option<&mut Shader> {
        self.active.as_mut().map(|a| &mut a.shader)
    }

    #[cfg(not(target_os = "emscripten"))]
    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    fn load(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        vfs: &dyn VirtualFs,
        name: &str,
        policy: LoadPolicy,
    ) -> LoadOutcome {
        let fragment = match read_fragment_source(vfs, name, &mut self.generic_compiler_cache) {
            Ok(src) => src,
            Err(e) => {
                crate::msg::err!("{}", e.render(name));
                return self.source_load_failed(policy);
            }
        };
        let vs_pair = read_with_fallback(vfs, name, "vs");
        let vs_src = match vs_pair.as_ref() {
            Some((_, bytes)) => match std::str::from_utf8(bytes) {
                Ok(s) => Some(s.to_string()),
                Err(e) => {
                    let key = vs_pair
                        .as_ref()
                        .map(|(key, _)| key.clone())
                        .unwrap_or_else(|| format!("shaders/{name}.vs"));
                    crate::msg::err!(
                        "{}",
                        ShaderSourceError::InvalidUtf8 {
                            key,
                            detail: format!("vertex source not valid utf-8: {e}"),
                        }
                        .render(name)
                    );
                    return self.source_load_failed(policy);
                }
            },
            None => None,
        };

        let (shader, driver_log) = driver_log::load_shader_from_memory_with_log(
            rl,
            thread,
            vs_src.as_deref(),
            Some(&fragment.source),
        );
        if !shader.is_shader_valid() {
            crate::msg::err!(
                "{}",
                gl_driver_failure_message(name, &fragment, &driver_log)
            );
            // Drop the bad shader and keep whatever was active before
            // unset. Returning early here means `active` isn't
            // overwritten with a broken handle.
            return LoadOutcome::FailedKeepPrevious;
        }
        driver_log.forward();
        report_shader_compiler_warnings(name, &fragment);

        let fs_mtime = file_mtime(vfs, &fragment.key);
        let vs_key = vs_pair.as_ref().map(|(k, _)| k.clone());
        let vs_mtime = vs_key.as_deref().and_then(|k| file_mtime(vfs, k));
        let uniform_types = fragment
            .metadata
            .as_ref()
            .map(shader_uniform_type_map)
            .unwrap_or_default();
        self.active = Some(Active {
            name: name.to_string(),
            shader,
            fs_key: fragment.key,
            vs_key,
            fs_mtime,
            vs_mtime,
            last_uniforms: HashMap::new(),
            metadata: fragment.metadata,
            uniform_types,
            reported_uniform_type_errors: HashSet::new(),
        });
        LoadOutcome::Loaded
    }

    fn source_load_failed(&mut self, policy: LoadPolicy) -> LoadOutcome {
        match policy {
            LoadPolicy::Request => {
                self.active = None;
                LoadOutcome::FailedCleared
            }
            LoadPolicy::Reload => LoadOutcome::FailedKeepPrevious,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoadPolicy {
    Request,
    Reload,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoadOutcome {
    Loaded,
    FailedKeepPrevious,
    FailedCleared,
}

#[derive(Default)]
struct GenericShaderCompilerCache {
    entries: HashMap<GenericShaderCompilerCacheKey, Vec<GenericShaderCompilerCacheEntry>>,
    len: usize,
    clock: u64,
}

impl GenericShaderCompilerCache {
    fn compile(
        &mut self,
        src: &str,
        profile: ShaderProfile,
    ) -> Result<compiler::CompiledFragment, String> {
        let key = GenericShaderCompilerCacheKey::new(src, profile);
        let clock = self.next_clock();

        if let Some(bucket) = self.entries.get_mut(&key)
            && let Some(entry) = bucket.iter_mut().find(|entry| entry.source == src)
        {
            entry.last_used = clock;
            return Ok(entry.compiled.clone());
        }

        let compiled = generate_generic_fragment_with_metadata(src, profile)?;
        if self.len >= GENERIC_SHADER_COMPILER_CACHE_LIMIT {
            self.evict_lru();
        }
        self.entries
            .entry(key)
            .or_default()
            .push(GenericShaderCompilerCacheEntry {
                source: src.to_string(),
                compiled: compiled.clone(),
                last_used: clock,
            });
        self.len += 1;
        Ok(compiled)
    }

    fn next_clock(&mut self) -> u64 {
        self.clock = self.clock.wrapping_add(1);
        self.clock
    }

    fn evict_lru(&mut self) {
        let mut victim = None;
        for (key, bucket) in &self.entries {
            for (index, entry) in bucket.iter().enumerate() {
                if victim.is_none_or(|(_, _, last_used)| entry.last_used < last_used) {
                    victim = Some((*key, index, entry.last_used));
                }
            }
        }

        let Some((key, index, _)) = victim else {
            return;
        };
        let remove_bucket = {
            let bucket = self
                .entries
                .get_mut(&key)
                .expect("cache victim key must exist");
            bucket.swap_remove(index);
            bucket.is_empty()
        };
        if remove_bucket {
            self.entries.remove(&key);
        }
        self.len = self.len.saturating_sub(1);
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.len
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct GenericShaderCompilerCacheKey {
    profile: ShaderProfile,
    source_hash: u64,
    source_len: usize,
}

impl GenericShaderCompilerCacheKey {
    fn new(src: &str, profile: ShaderProfile) -> Self {
        Self {
            profile,
            source_hash: source_hash(src),
            source_len: src.len(),
        }
    }
}

struct GenericShaderCompilerCacheEntry {
    source: String,
    compiled: compiler::CompiledFragment,
    last_used: u64,
}

#[derive(Debug)]
struct FragmentSource {
    key: String,
    source: String,
    metadata: Option<compiler::ShaderMetadata>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ShaderSourceError {
    Missing { message: String },
    InvalidUtf8 { key: String, detail: String },
    Compiler { key: String, diagnostic: String },
}

impl ShaderSourceError {
    fn render(&self, shader_name: &str) -> String {
        match self {
            Self::Missing { message } => {
                format!("shader '{shader_name}' [source]: {message}")
            }
            Self::InvalidUtf8 { key, detail } => {
                format!("shader '{shader_name}' [source]: {key}: {detail}")
            }
            Self::Compiler { key, diagnostic } => {
                format!("shader '{shader_name}' [compiler]: {key}\n{diagnostic}")
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShaderBuildTarget {
    Desktop,
    Web,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ShaderProfile {
    DesktopGlsl330,
    #[allow(
        dead_code,
        reason = "forward-compatible emitter target; runtime selection currently uses GLSL 330 for desktop"
    )]
    DesktopGlsl440,
    WebGlslEs100,
}

impl ShaderProfile {
    #[cfg(not(target_os = "emscripten"))]
    pub(crate) const ALL: [Self; 3] = [
        Self::WebGlslEs100,
        Self::DesktopGlsl330,
        Self::DesktopGlsl440,
    ];

    fn for_build_target(target: ShaderBuildTarget) -> Self {
        match target {
            ShaderBuildTarget::Desktop => Self::DesktopGlsl330,
            ShaderBuildTarget::Web => Self::WebGlslEs100,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::DesktopGlsl330 => "GLSL 330",
            Self::DesktopGlsl440 => "GLSL 440",
            Self::WebGlslEs100 => "GLSL ES 100",
        }
    }
}

fn read_fragment_source(
    vfs: &dyn VirtualFs,
    name: &str,
    cache: &mut GenericShaderCompilerCache,
) -> Result<FragmentSource, ShaderSourceError> {
    let generic_key = generic_fragment_key(name);
    if let Some(bytes) = vfs.read_file(&generic_key) {
        let src = std::str::from_utf8(&bytes).map_err(|e| ShaderSourceError::InvalidUtf8 {
            key: generic_key.clone(),
            detail: format!("source not valid utf-8: {e}"),
        })?;
        let compiled =
            cache
                .compile(src, target_profile())
                .map_err(|e| ShaderSourceError::Compiler {
                    key: generic_key.clone(),
                    diagnostic: e,
                })?;
        return Ok(FragmentSource {
            key: generic_key,
            source: compiled.source,
            metadata: Some(compiled.metadata),
        });
    }

    let Some((key, bytes)) = read_with_fallback(vfs, name, "fs") else {
        return Err(ShaderSourceError::Missing {
            message: format!(
                "no shaders/{name}.usagi.fs, shaders/{name}.fs, or shaders/{name}_es.fs found"
            ),
        });
    };
    let src = std::str::from_utf8(&bytes).map_err(|e| ShaderSourceError::InvalidUtf8 {
        key: key.clone(),
        detail: format!("fragment source not valid utf-8: {e}"),
    })?;
    Ok(FragmentSource {
        key,
        source: src.to_string(),
        metadata: None,
    })
}

fn generate_generic_fragment_with_metadata(
    src: &str,
    profile: ShaderProfile,
) -> Result<compiler::CompiledFragment, String> {
    compiler::compile_fragment_with_metadata(src, profile)
}

fn source_hash(src: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    src.hash(&mut hasher);
    hasher.finish()
}

fn shader_uniform_accepts_value(uniform_ty: &str, value: ShaderValue) -> bool {
    matches!(
        (uniform_ty, value),
        ("float", ShaderValue::Float(_))
            | ("vec2", ShaderValue::Vec2(_))
            | ("vec3", ShaderValue::Vec3(_))
            | ("vec4", ShaderValue::Vec4(_))
    )
}

fn shader_value_kind(value: ShaderValue) -> &'static str {
    match value {
        ShaderValue::Float(_) => "float",
        ShaderValue::Vec2(_) => "vec2",
        ShaderValue::Vec3(_) => "vec3",
        ShaderValue::Vec4(_) => "vec4",
    }
}

fn shader_uniform_type_map(metadata: &compiler::ShaderMetadata) -> HashMap<String, String> {
    let mut uniforms = HashMap::with_capacity(metadata.uniforms.len());
    for uniform in &metadata.uniforms {
        uniforms.insert(uniform.name.clone(), uniform.ty.clone());
    }
    uniforms
}

fn report_shader_compiler_warnings(shader_name: &str, fragment: &FragmentSource) {
    let Some(metadata) = fragment.metadata.as_ref() else {
        return;
    };
    for warning in &metadata.warnings {
        crate::msg::warn!(
            "shader '{shader_name}' [compiler-warning]: {}\n{}",
            fragment.key,
            warning.render()
        );
    }
}

fn gl_driver_failure_message(
    shader_name: &str,
    fragment: &FragmentSource,
    driver_log: &driver_log::RaylibShaderLog,
) -> String {
    let mut message = format!(
        "shader '{shader_name}' [gl-driver]: compile/link failed for {}",
        fragment.key
    );

    if let Some(metadata) = &fragment.metadata {
        message.push_str(" [");
        message.push_str(metadata.profile.label());
        message.push(']');
        if let (Some((generated_start, generated_end)), Some((source_start, source_end))) = (
            metadata.source_map.generated_source_line_range(),
            metadata.source_map.original_source_line_range(),
        ) {
            message.push_str(&format!(
                "; generated lines {generated_start}-{generated_end} map to {}:{source_start}-{source_end}",
                fragment.key
            ));
        }
        message.push_str("; inspect with `usagi shaders emit --format json`");
    }

    if driver_log.is_empty() {
        message.push_str("; no raylib driver log lines were captured");
    } else {
        message.push_str(&driver_log.format_remapped(&fragment.key, fragment.metadata.as_ref()));
    }
    message
}

/// Reads `shaders/<name>.<ext>` with the platform-preferred filename
/// first and the alt as fallback. Returns the key that hit and its
/// bytes so callers can stat / reload against the same file.
fn read_with_fallback(vfs: &dyn VirtualFs, name: &str, ext: &str) -> Option<(String, Vec<u8>)> {
    let primary = primary_key(name, ext);
    let alt = alt_key(name, ext);
    if let Some(bytes) = vfs.read_file(&primary) {
        return Some((primary, bytes));
    }
    if let Some(bytes) = vfs.read_file(&alt) {
        return Some((alt, bytes));
    }
    None
}

fn file_mtime(vfs: &dyn VirtualFs, key: &str) -> Option<SystemTime> {
    vfs.file_mtime(key)
}

fn generic_fragment_key(name: &str) -> String {
    format!("shaders/{name}.usagi.fs")
}

fn build_target() -> ShaderBuildTarget {
    if cfg!(target_family = "wasm") {
        ShaderBuildTarget::Web
    } else {
        ShaderBuildTarget::Desktop
    }
}

fn target_profile() -> ShaderProfile {
    ShaderProfile::for_build_target(build_target())
}

fn primary_key(name: &str, ext: &str) -> String {
    primary_key_for_target(build_target(), name, ext)
}

fn alt_key(name: &str, ext: &str) -> String {
    alt_key_for_target(build_target(), name, ext)
}

fn primary_key_for_target(target: ShaderBuildTarget, name: &str, ext: &str) -> String {
    match target {
        ShaderBuildTarget::Desktop => format!("shaders/{name}.{ext}"),
        ShaderBuildTarget::Web => format!("shaders/{name}_es.{ext}"),
    }
}

fn alt_key_for_target(target: ShaderBuildTarget, name: &str, ext: &str) -> String {
    match target {
        ShaderBuildTarget::Desktop => format!("shaders/{name}_es.{ext}"),
        ShaderBuildTarget::Web => format!("shaders/{name}.{ext}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generate_generic_fragment(src: &str, profile: ShaderProfile) -> Result<String, String> {
        generate_generic_fragment_with_metadata(src, profile).map(|compiled| compiled.source)
    }

    fn read_fragment_source_for_test(
        vfs: &dyn VirtualFs,
        name: &str,
    ) -> Result<FragmentSource, ShaderSourceError> {
        let mut cache = GenericShaderCompilerCache::default();
        read_fragment_source(vfs, name, &mut cache)
    }

    fn valid_generic_shader_with_value(value: &str) -> String {
        format!("vec4 usagi_main(vec2 uv, vec4 color) {{ return vec4({value}, 1.0); }}\n")
    }

    #[test]
    fn primary_then_alt_keys_pair_by_platform() {
        let p = primary_key("crt", "fs");
        let a = alt_key("crt", "fs");
        assert!(p == "shaders/crt.fs" || p == "shaders/crt_es.fs");
        assert!(a == "shaders/crt.fs" || a == "shaders/crt_es.fs");
        assert_ne!(p, a);
    }

    #[test]
    fn compiled_build_target_selects_expected_shader_profile() {
        if cfg!(target_family = "wasm") {
            assert_eq!(build_target(), ShaderBuildTarget::Web);
            assert_eq!(target_profile(), ShaderProfile::WebGlslEs100);
        } else {
            assert_eq!(build_target(), ShaderBuildTarget::Desktop);
            assert_eq!(target_profile(), ShaderProfile::DesktopGlsl330);
        }
    }

    #[test]
    fn shader_profiles_map_from_explicit_build_targets() {
        assert_eq!(
            ShaderProfile::for_build_target(ShaderBuildTarget::Desktop),
            ShaderProfile::DesktopGlsl330
        );
        assert_eq!(
            ShaderProfile::for_build_target(ShaderBuildTarget::Web),
            ShaderProfile::WebGlslEs100
        );
    }

    #[test]
    fn native_shader_fallback_order_matches_build_target() {
        assert_eq!(
            primary_key_for_target(ShaderBuildTarget::Desktop, "crt", "fs"),
            "shaders/crt.fs"
        );
        assert_eq!(
            alt_key_for_target(ShaderBuildTarget::Desktop, "crt", "fs"),
            "shaders/crt_es.fs"
        );
        assert_eq!(
            primary_key_for_target(ShaderBuildTarget::Web, "crt", "fs"),
            "shaders/crt_es.fs"
        );
        assert_eq!(
            alt_key_for_target(ShaderBuildTarget::Web, "crt", "fs"),
            "shaders/crt.fs"
        );
    }

    #[test]
    fn empty_manager_starts_inactive() {
        let m = ShaderManager::new();
        assert!(m.active.is_none());
    }

    #[test]
    fn clearing_request_drops_active_on_apply() {
        let mut m = ShaderManager::new();
        m.request_set(None);
        assert!(matches!(m.pending_shader, Some(None)));
    }

    #[test]
    fn generic_fragment_key_uses_usagi_suffix() {
        assert_eq!(generic_fragment_key("crt"), "shaders/crt.usagi.fs");
    }

    #[test]
    fn generic_fragment_source_wins_over_native_fallback() {
        let mut bundle = crate::bundle::Bundle::new();
        bundle.insert("main.lua", Vec::new());
        bundle.insert(
            "shaders/crt.usagi.fs",
            b"vec4 usagi_main(vec2 uv, vec4 color) { return vec4(0.1, 0.2, 0.3, 1.0); }\n".to_vec(),
        );
        bundle.insert(
            "shaders/crt.fs",
            b"#version 330\n// native_only_marker\nvoid main() {}\n".to_vec(),
        );
        let vfs = crate::vfs::BundleBacked::new(bundle);

        let fragment = read_fragment_source_for_test(&vfs, "crt").unwrap();

        assert_eq!(fragment.key, "shaders/crt.usagi.fs");
        assert!(fragment.source.contains("return vec4(0.1, 0.2, 0.3, 1.0);"));
        assert!(!fragment.source.contains("native_only_marker"));
        let metadata = fragment.metadata.expect("generic shader metadata");
        assert_eq!(metadata.profile, target_profile());
        assert!(metadata.uniforms.is_empty());
    }

    #[test]
    fn generic_fragment_errors_are_reported_as_compiler_errors() {
        let mut bundle = crate::bundle::Bundle::new();
        bundle.insert("main.lua", Vec::new());
        bundle.insert(
            "shaders/bad.usagi.fs",
            b"vec4 usagi_main(vec2 uv, vec4 color) { return texture(texture0, uv); }\n".to_vec(),
        );
        let vfs = crate::vfs::BundleBacked::new(bundle);

        let err = read_fragment_source_for_test(&vfs, "bad").unwrap_err();
        let message = err.render("bad");

        assert!(message.contains("shader 'bad' [compiler]: shaders/bad.usagi.fs"));
        assert!(message.contains("generic shaders must use usagi_texture"));
        assert!(message.contains("line 1, column"));
    }

    #[test]
    fn missing_fragment_errors_are_reported_as_source_errors() {
        let bundle = crate::bundle::Bundle::new();
        let vfs = crate::vfs::BundleBacked::new(bundle);

        let err = read_fragment_source_for_test(&vfs, "missing").unwrap_err();
        let message = err.render("missing");

        assert!(message.contains("shader 'missing' [source]:"));
        assert!(message.contains("no shaders/missing.usagi.fs"));
    }

    #[test]
    fn invalid_fragment_utf8_errors_are_reported_as_source_errors() {
        let mut bundle = crate::bundle::Bundle::new();
        bundle.insert("main.lua", Vec::new());
        bundle.insert("shaders/bad.usagi.fs", vec![0xff]);
        let vfs = crate::vfs::BundleBacked::new(bundle);

        let err = read_fragment_source_for_test(&vfs, "bad").unwrap_err();
        let message = err.render("bad");

        assert!(message.contains("shader 'bad' [source]: shaders/bad.usagi.fs"));
        assert!(message.contains("source not valid utf-8"));
    }

    #[test]
    fn generic_fragment_metadata_feeds_uniform_type_map() {
        let src = concat!(
            "#usagi shader 1\n\n",
            "uniform float u_time;\n",
            "uniform vec2 u_resolution, u_origin;\n\n",
            "vec4 usagi_main(vec2 uv, vec4 color) {\n",
            "    return color * u_time + ",
            "vec4(u_resolution / max(u_origin, vec2(1.0)), 0.0, 0.0);\n",
            "}\n",
        );
        let compiled =
            generate_generic_fragment_with_metadata(src, ShaderProfile::DesktopGlsl330).unwrap();

        let uniform_types = shader_uniform_type_map(&compiled.metadata);
        assert_eq!(
            uniform_types.get("u_time").map(String::as_str),
            Some("float")
        );
        assert_eq!(
            uniform_types.get("u_resolution").map(String::as_str),
            Some("vec2")
        );
        assert_eq!(
            compiled.metadata.source_map.generated_source_line_range(),
            Some((8, 13))
        );
        assert_eq!(
            compiled.metadata.source_map.original_source_line_range(),
            Some((3, 8))
        );
    }

    #[test]
    fn generic_compiler_cache_reuses_exact_source_and_profile() {
        let mut cache = GenericShaderCompilerCache::default();
        let src = valid_generic_shader_with_value("0.1, 0.2, 0.3");

        let first = cache
            .compile(&src, ShaderProfile::DesktopGlsl330)
            .expect("first compile");
        let second = cache
            .compile(&src, ShaderProfile::DesktopGlsl330)
            .expect("second compile");

        assert_eq!(cache.len(), 1);
        assert_eq!(first.source, second.source);
        assert_eq!(first.metadata, second.metadata);
    }

    #[test]
    fn generic_compiler_cache_separates_profiles_and_source_contents() {
        let mut cache = GenericShaderCompilerCache::default();
        let a = valid_generic_shader_with_value("0.1, 0.2, 0.3");
        let b = valid_generic_shader_with_value("0.3, 0.2, 0.1");

        cache.compile(&a, ShaderProfile::DesktopGlsl330).unwrap();
        cache.compile(&a, ShaderProfile::WebGlslEs100).unwrap();
        cache.compile(&b, ShaderProfile::DesktopGlsl330).unwrap();

        assert_eq!(cache.len(), 3);
    }

    #[test]
    fn generic_compiler_cache_is_bounded() {
        let mut cache = GenericShaderCompilerCache::default();

        for index in 0..(GENERIC_SHADER_COMPILER_CACHE_LIMIT + 4) {
            let src =
                valid_generic_shader_with_value(&format!("{:.1}, 0.0, 0.0", index as f32 / 10.0));
            cache.compile(&src, ShaderProfile::DesktopGlsl330).unwrap();
        }

        assert_eq!(cache.len(), GENERIC_SHADER_COMPILER_CACHE_LIMIT);
    }

    #[test]
    fn generic_gl_driver_failure_message_includes_source_map_hint() {
        let mut bundle = crate::bundle::Bundle::new();
        bundle.insert("main.lua", Vec::new());
        bundle.insert(
            "shaders/crt.usagi.fs",
            b"#usagi shader 1\n\nvec4 usagi_main(vec2 uv, vec4 color) {\n    return color;\n}\n"
                .to_vec(),
        );
        let vfs = crate::vfs::BundleBacked::new(bundle);
        let fragment = read_fragment_source_for_test(&vfs, "crt").unwrap();
        let driver_log = driver_log::RaylibShaderLog::from_entries(vec![(
            "SHADER: [ID 7] Compile error: 0(8) : error C0000",
            driver_log::RaylibLogLevel::Warning,
        )]);
        let message = gl_driver_failure_message("crt", &fragment, &driver_log);

        assert!(message.contains("shader 'crt' [gl-driver]"));
        assert!(message.contains("[GLSL 330]"));
        assert!(message.contains("generated lines"));
        assert!(message.contains("shaders/crt.usagi.fs:3-5"));
        assert!(message.contains("usagi shaders emit --format json"));
        assert!(message.contains("[driver-log] WARNING: SHADER: [ID 7] Compile error"));
        assert!(message.contains("generated line 8 -> shaders/crt.usagi.fs:3"));
    }

    #[test]
    fn shader_uniform_type_validation_accepts_lua_value_shapes() {
        assert!(shader_uniform_accepts_value(
            "float",
            ShaderValue::Float(1.0)
        ));
        assert!(shader_uniform_accepts_value(
            "vec2",
            ShaderValue::Vec2([1.0, 2.0])
        ));
        assert!(shader_uniform_accepts_value(
            "vec3",
            ShaderValue::Vec3([1.0, 2.0, 3.0])
        ));
        assert!(shader_uniform_accepts_value(
            "vec4",
            ShaderValue::Vec4([1.0, 2.0, 3.0, 4.0])
        ));

        assert!(!shader_uniform_accepts_value(
            "vec2",
            ShaderValue::Float(1.0)
        ));
        assert!(!shader_uniform_accepts_value(
            "sampler2D",
            ShaderValue::Float(1.0)
        ));
    }

    #[test]
    fn vertex_source_errors_render_as_source_errors() {
        let message = ShaderSourceError::InvalidUtf8 {
            key: "shaders/crt.vs".to_string(),
            detail: "vertex source not valid utf-8: bad byte".to_string(),
        }
        .render("crt");

        assert!(message.contains("shader 'crt' [source]: shaders/crt.vs"));
        assert!(message.contains("vertex source not valid utf-8"));
    }

    #[cfg(not(target_os = "emscripten"))]
    const VALID_RUNTIME_SHADER: &str = concat!(
        "#usagi shader 1\n\n",
        "vec4 usagi_main(vec2 uv, vec4 color) {\n",
        "    return usagi_texture(texture0, uv) * color;\n",
        "}\n",
    );

    #[cfg(not(target_os = "emscripten"))]
    const INVALID_RUNTIME_SHADER: &str =
        "vec4 usagi_main(vec2 uv, vec4 color) { return texture(texture0, uv); }\n";

    #[cfg(not(target_os = "emscripten"))]
    const DRIVER_INVALID_RUNTIME_SHADER: &str =
        "vec4 usagi_main(vec2 uv, vec4 color) { return vec4(no_such_symbol, 1.0); }\n";

    #[cfg(not(target_os = "emscripten"))]
    fn shader_runtime_project(shader_src: &str) -> (tempfile::TempDir, crate::vfs::FsBacked) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("main.lua"), "-- shader runtime test").unwrap();
        std::fs::create_dir(dir.path().join("shaders")).unwrap();
        std::fs::write(dir.path().join("shaders/crt.usagi.fs"), shader_src).unwrap();
        let vfs = crate::vfs::FsBacked::from_script_path(&dir.path().join("main.lua"));
        (dir, vfs)
    }

    #[cfg(not(target_os = "emscripten"))]
    fn shader_runtime_context(title: &str) -> (RaylibHandle, RaylibThread) {
        sola_raylib::init()
            .size(32, 32)
            .title(title)
            .log_level(TraceLogLevel::LOG_WARNING)
            .build()
    }

    #[cfg(not(target_os = "emscripten"))]
    fn shader_runtime_test_guard() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[cfg(not(target_os = "emscripten"))]
    #[test]
    #[ignore = "requires a desktop OpenGL context; run with `cargo test shader_runtime -- --ignored`"]
    fn shader_runtime_driver_error_log_maps_to_source_line() {
        let _guard = shader_runtime_test_guard();
        let (_dir, vfs) = shader_runtime_project(DRIVER_INVALID_RUNTIME_SHADER);
        let (mut rl, thread) = shader_runtime_context("usagi shader driver log remap test");
        let fragment = read_fragment_source_for_test(&vfs, "crt")
            .expect("fixture must pass Usagi compiler before GL driver compile");
        let (shader, driver_log) = driver_log::load_shader_from_memory_with_log(
            &mut rl,
            &thread,
            None,
            Some(&fragment.source),
        );

        assert!(!shader.is_shader_valid());
        let message = gl_driver_failure_message("crt", &fragment, &driver_log);
        assert!(message.contains("[driver-log]"));
        assert!(message.contains("no_such_symbol"));
        assert!(message.contains("generated line"));
        assert!(message.contains("shaders/crt.usagi.fs:1"));
    }

    #[cfg(not(target_os = "emscripten"))]
    #[test]
    #[ignore = "requires a desktop OpenGL context; run with `cargo test shader_runtime -- --ignored`"]
    fn shader_runtime_reload_compiler_error_keeps_previous_shader() {
        let _guard = shader_runtime_test_guard();
        let (_dir, vfs) = shader_runtime_project(VALID_RUNTIME_SHADER);
        let (mut rl, thread) =
            shader_runtime_context("usagi shader reload failure preservation test");
        let mut manager = ShaderManager::new();

        assert_eq!(
            manager.load(&mut rl, &thread, &vfs, "crt", LoadPolicy::Request),
            LoadOutcome::Loaded
        );
        std::fs::write(
            _dir.path().join("shaders/crt.usagi.fs"),
            INVALID_RUNTIME_SHADER,
        )
        .unwrap();
        manager.active.as_mut().unwrap().fs_mtime = Some(std::time::SystemTime::UNIX_EPOCH);

        assert!(!manager.reload_if_changed(&mut rl, &thread, &vfs));
        let active = manager
            .active
            .as_ref()
            .expect("reload failure must keep previous shader active");
        assert_eq!(active.name, "crt");
        assert_eq!(active.fs_key, "shaders/crt.usagi.fs");
    }

    #[cfg(not(target_os = "emscripten"))]
    #[test]
    #[ignore = "requires a desktop OpenGL context; run with `cargo test shader_runtime -- --ignored`"]
    fn shader_runtime_same_name_request_is_idempotent() {
        let _guard = shader_runtime_test_guard();
        let (_dir, vfs) = shader_runtime_project(VALID_RUNTIME_SHADER);
        let (mut rl, thread) = shader_runtime_context("usagi shader same-name request test");
        let mut manager = ShaderManager::new();

        assert_eq!(
            manager.load(&mut rl, &thread, &vfs, "crt", LoadPolicy::Request),
            LoadOutcome::Loaded
        );
        std::fs::write(
            _dir.path().join("shaders/crt.usagi.fs"),
            INVALID_RUNTIME_SHADER,
        )
        .unwrap();

        manager.request_set(Some("crt".to_string()));
        manager.apply_pending(&mut rl, &thread, &vfs);

        let active = manager
            .active
            .as_ref()
            .expect("same-name request must not recompile and clear the active shader");
        assert_eq!(active.name, "crt");
    }

    #[cfg(not(target_os = "emscripten"))]
    #[test]
    #[ignore = "requires a desktop OpenGL context; run with `cargo test shader_runtime -- --ignored`"]
    fn shader_runtime_examples_compile_and_gameboy_pixels_match() {
        let _guard = shader_runtime_test_guard();
        let (mut rl, thread) = sola_raylib::init()
            .size(32, 32)
            .title("usagi shader runtime test")
            .log_level(TraceLogLevel::LOG_WARNING)
            .build();
        for (name, src) in [
            (
                "crt",
                include_str!("../../examples/shader/shaders/crt.usagi.fs"),
            ),
            (
                "gameboy",
                include_str!("../../examples/shader/shaders/gameboy.usagi.fs"),
            ),
            (
                "notetris",
                include_str!("../../examples/notetris/shaders/notetris.usagi.fs"),
            ),
            (
                "playdate_palette",
                include_str!("../../examples/playdate/shaders/playdate_palette.usagi.fs"),
            ),
        ] {
            let generated = generate_generic_fragment(src, ShaderProfile::DesktopGlsl330)
                .unwrap_or_else(|e| panic!("{name}: generic shader generation failed: {e}"));
            let shader = rl.load_shader_from_memory(&thread, None, Some(&generated));
            assert!(
                shader.is_shader_valid(),
                "{name}: generated shader did not compile"
            );
        }

        let gameboy_src = include_str!("../../examples/shader/shaders/gameboy.usagi.fs");
        let generated = generate_generic_fragment(gameboy_src, ShaderProfile::DesktopGlsl330)
            .expect("gameboy shader generation failed");
        let mut shader = rl.load_shader_from_memory(&thread, None, Some(&generated));
        assert!(shader.is_shader_valid());

        const W: u32 = 4;
        const H: u32 = 4;
        let mut source = rl
            .load_render_texture(&thread, W, H)
            .expect("source render texture");
        let mut target = rl
            .load_render_texture(&thread, W, H)
            .expect("target render texture");
        let test_res = crate::config::Resolution {
            w: W as f32,
            h: H as f32,
        };

        {
            let mut d = rl.begin_texture_mode(&thread, &mut source);
            d.clear_background(Color::BLACK);
            d.draw_rectangle(0, 0, 1, H as i32, Color::new(0, 0, 0, 255));
            d.draw_rectangle(1, 0, 1, H as i32, Color::new(96, 96, 96, 255));
            d.draw_rectangle(2, 0, 1, H as i32, Color::new(160, 160, 160, 255));
            d.draw_rectangle(3, 0, 1, H as i32, Color::new(255, 255, 255, 255));
        }

        {
            let mut d = rl.begin_texture_mode(&thread, &mut target);
            d.clear_background(Color::BLACK);
            let mut s = d.begin_shader_mode(&mut shader);
            crate::render::draw_render_target_native(&mut s, &mut source, test_res);
        }

        let image = target.texture().load_image().expect("target readback");
        let pixels = image.get_image_data();
        assert_eq!(pixels.len(), (W * H) as usize);

        let y = 1usize;
        let expected = [
            Color::new(15, 56, 15, 255),
            Color::new(48, 98, 48, 255),
            Color::new(139, 172, 15, 255),
            Color::new(155, 188, 15, 255),
        ];
        for (x, want) in expected.into_iter().enumerate() {
            let got = pixels[y * W as usize + x];
            assert_color_near(got, want, 5, "gameboy palette stripe");
        }

        let captures = tempfile::tempdir().unwrap();
        let png =
            crate::capture::save_screenshot(&target, captures.path(), "shader", test_res).unwrap();
        assert!(std::fs::metadata(&png).unwrap().len() > 0);

        let mut recorder = crate::capture::Recorder::new();
        assert!(
            recorder
                .toggle(captures.path(), "shader", test_res)
                .unwrap()
                .is_none()
        );
        recorder.capture(&target, crate::capture::RecordingColorMode::AdaptivePalette);
        let gif = recorder
            .toggle(captures.path(), "shader", test_res)
            .unwrap()
            .expect("stopping recording should return saved gif path");
        assert!(std::fs::metadata(&gif).unwrap().len() > 0);
    }

    #[cfg(not(target_os = "emscripten"))]
    fn assert_color_near(got: Color, want: Color, tolerance: u8, label: &str) {
        let near = |a: u8, b: u8| a.abs_diff(b) <= tolerance;
        assert!(
            near(got.r, want.r)
                && near(got.g, want.g)
                && near(got.b, want.b)
                && near(got.a, want.a),
            "{label}: got rgba({}, {}, {}, {}), want near rgba({}, {}, {}, {})",
            got.r,
            got.g,
            got.b,
            got.a,
            want.r,
            want.g,
            want.b,
            want.a
        );
    }
}
