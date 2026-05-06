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

use crate::vfs::VirtualFs;
use sola_raylib::prelude::*;
use std::collections::HashMap;
use std::time::SystemTime;

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
}

#[derive(Default)]
pub struct ShaderManager {
    /// Outer Some = a request is pending. Inner Some(name) = activate
    /// that shader; inner None = clear.
    pending_shader: Option<Option<String>>,
    pending_uniforms: Vec<(String, ShaderValue)>,
    active: Option<Active>,
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
                Some(name) => self.load(rl, thread, vfs, &name),
                None => self.active = None,
            }
        }
        if let Some(active) = self.active.as_mut() {
            for (uname, value) in self.pending_uniforms.drain(..) {
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
        self.load(rl, thread, vfs, &name);
        if let Some(active) = self.active.as_mut() {
            for (uname, value) in &cached {
                let loc = active.shader.get_shader_location(uname);
                value.apply(&mut active.shader, loc);
            }
            active.last_uniforms = cached;
        }
        true
    }

    /// Returns the currently-bound shader, if any. Used by the blit
    /// path to wrap the RT draw in `begin_shader_mode`.
    pub fn active_shader_mut(&mut self) -> Option<&mut Shader> {
        self.active.as_mut().map(|a| &mut a.shader)
    }

    fn load(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        vfs: &dyn VirtualFs,
        name: &str,
    ) {
        let (fs_key, fs_src) = match read_fragment_source(vfs, name) {
            Ok(src) => src,
            Err(e) => {
                crate::msg::err!("shader '{name}': {e}");
                self.active = None;
                return;
            }
        };
        let vs_pair = read_with_fallback(vfs, name, "vs");
        let vs_src = match vs_pair.as_ref() {
            Some((_, bytes)) => match std::str::from_utf8(bytes) {
                Ok(s) => Some(s.to_string()),
                Err(e) => {
                    crate::msg::err!("shader '{name}': vertex source not valid utf-8: {e}");
                    self.active = None;
                    return;
                }
            },
            None => None,
        };

        let shader = rl.load_shader_from_memory(thread, vs_src.as_deref(), Some(&fs_src));
        if !shader.is_shader_valid() {
            crate::msg::err!("shader '{name}': compile/link failed (see GL log above)");
            // Drop the bad shader and keep whatever was active before
            // unset. Returning early here means `active` isn't
            // overwritten with a broken handle.
            return;
        }

        let fs_mtime = file_mtime(vfs, &fs_key);
        let vs_key = vs_pair.as_ref().map(|(k, _)| k.clone());
        let vs_mtime = vs_key.as_deref().and_then(|k| file_mtime(vfs, k));
        self.active = Some(Active {
            name: name.to_string(),
            shader,
            fs_key,
            vs_key,
            fs_mtime,
            vs_mtime,
            last_uniforms: HashMap::new(),
        });
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShaderBuildTarget {
    Desktop,
    Web,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShaderProfile {
    DesktopGlsl330,
    WebGlslEs100,
}

impl ShaderProfile {
    fn for_build_target(target: ShaderBuildTarget) -> Self {
        match target {
            ShaderBuildTarget::Desktop => Self::DesktopGlsl330,
            ShaderBuildTarget::Web => Self::WebGlslEs100,
        }
    }
}

fn read_fragment_source(vfs: &dyn VirtualFs, name: &str) -> Result<(String, String), String> {
    let generic_key = generic_fragment_key(name);
    if let Some(bytes) = vfs.read_file(&generic_key) {
        let src = std::str::from_utf8(&bytes)
            .map_err(|e| format!("{generic_key}: source not valid utf-8: {e}"))?;
        let generated = generate_generic_fragment(src, target_profile())
            .map_err(|e| format!("{generic_key}: {e}"))?;
        return Ok((generic_key, generated));
    }

    let Some((key, bytes)) = read_with_fallback(vfs, name, "fs") else {
        return Err(format!(
            "no shaders/{name}.usagi.fs, shaders/{name}.fs, or shaders/{name}_es.fs found"
        ));
    };
    let src = std::str::from_utf8(&bytes)
        .map_err(|e| format!("{key}: fragment source not valid utf-8: {e}"))?;
    Ok((key, src.to_string()))
}

fn generate_generic_fragment(src: &str, profile: ShaderProfile) -> Result<String, String> {
    let body = strip_usagi_shader_directive(src).trim_start_matches('\n');
    if body.contains("#version") {
        return Err("generic shaders must not declare #version".to_string());
    }
    if !body.contains("usagi_main") {
        return Err("generic shaders must define vec4 usagi_main(vec2 uv, vec4 color)".to_string());
    }

    let header = match profile {
        ShaderProfile::DesktopGlsl330 => {
            "#version 330\n\nin vec2 fragTexCoord;\nin vec4 fragColor;\nuniform sampler2D texture0;\nout vec4 finalColor;\n#define usagi_texture(texture_name, uv) texture(texture_name, uv)\n\n"
        }
        ShaderProfile::WebGlslEs100 => {
            "#version 100\n\nprecision mediump float;\n\nvarying vec2 fragTexCoord;\nvarying vec4 fragColor;\nuniform sampler2D texture0;\n#define usagi_texture(texture_name, uv) texture2D(texture_name, uv)\n\n"
        }
    };
    let footer = match profile {
        ShaderProfile::DesktopGlsl330 => {
            "\n\nvoid main() {\n    finalColor = usagi_main(fragTexCoord, fragColor);\n}\n"
        }
        ShaderProfile::WebGlslEs100 => {
            "\n\nvoid main() {\n    gl_FragColor = usagi_main(fragTexCoord, fragColor);\n}\n"
        }
    };

    let mut out = String::with_capacity(header.len() + body.len() + footer.len());
    out.push_str(header);
    out.push_str(body);
    out.push_str(footer);
    Ok(out)
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

        let (key, src) = read_fragment_source(&vfs, "crt").unwrap();

        assert_eq!(key, "shaders/crt.usagi.fs");
        assert!(src.contains("return vec4(0.1, 0.2, 0.3, 1.0);"));
        assert!(!src.contains("native_only_marker"));
    }

    #[test]
    fn generic_fragment_generates_desktop_shader() {
        let src = "#usagi shader 1\n\nvec4 usagi_main(vec2 uv, vec4 color) {\n    return usagi_texture(texture0, uv) * color;\n}\n";
        let out = generate_generic_fragment(src, ShaderProfile::DesktopGlsl330).unwrap();

        assert!(out.contains("#version 330"));
        assert!(out.contains("in vec2 fragTexCoord;"));
        assert!(out.contains("out vec4 finalColor;"));
        assert!(out.contains("#define usagi_texture(texture_name, uv) texture(texture_name, uv)"));
        assert!(out.contains("finalColor = usagi_main(fragTexCoord, fragColor);"));
        assert!(!out.contains("#usagi shader 1"));
    }

    #[test]
    fn generic_fragment_generates_web_shader() {
        let src = "vec4 usagi_main(vec2 uv, vec4 color) {\n    return usagi_texture(texture0, uv) * color;\n}\n";
        let out = generate_generic_fragment(src, ShaderProfile::WebGlslEs100).unwrap();

        assert!(out.contains("#version 100"));
        assert!(out.contains("precision mediump float;"));
        assert!(out.contains("varying vec2 fragTexCoord;"));
        assert!(
            out.contains("#define usagi_texture(texture_name, uv) texture2D(texture_name, uv)")
        );
        assert!(out.contains("gl_FragColor = usagi_main(fragTexCoord, fragColor);"));
    }

    #[test]
    fn generic_fragment_rejects_version_directive() {
        let err = generate_generic_fragment(
            "#version 330\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("must not declare #version"));
    }

    #[test]
    fn generic_fragment_rejects_missing_entrypoint() {
        let err = generate_generic_fragment(
            "vec4 tint(vec4 color) { return color; }\n",
            target_profile(),
        )
        .unwrap_err();

        assert!(err.contains("usagi_main"));
    }

    #[cfg(not(target_os = "emscripten"))]
    #[test]
    #[ignore = "requires a desktop OpenGL context; run with `cargo test shader_runtime -- --ignored`"]
    fn shader_runtime_examples_compile_and_gameboy_pixels_match() {
        let (mut rl, thread) = sola_raylib::init()
            .size(32, 32)
            .title("usagi shader runtime test")
            .log_level(TraceLogLevel::LOG_WARNING)
            .build();
        for (name, src) in [
            (
                "crt",
                include_str!("../examples/shader/shaders/crt.usagi.fs"),
            ),
            (
                "gameboy",
                include_str!("../examples/shader/shaders/gameboy.usagi.fs"),
            ),
            (
                "notetris",
                include_str!("../examples/notetris/shaders/notetris.usagi.fs"),
            ),
            (
                "playdate_palette",
                include_str!("../examples/playdate/shaders/playdate_palette.usagi.fs"),
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

        let gameboy_src = include_str!("../examples/shader/shaders/gameboy.usagi.fs");
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
            s.draw_texture_pro(
                source.texture(),
                Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: W as f32,
                    height: -(H as f32),
                },
                Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: W as f32,
                    height: H as f32,
                },
                Vector2::new(0.0, 0.0),
                0.0,
                Color::WHITE,
            );
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
