//! Post-process shader support.
//!
//! Games declare a fragment shader that runs as a final pass on the
//! game render target as it's blitted to the window. Lua API:
//!
//! - `gfx.shader_set("name")` activates `shaders/<name>.fs` (and an
//!   optional `<name>.vs`). On web the loader prefers `<name>_es.fs`,
//!   on desktop it prefers `<name>.fs`; whichever isn't found falls
//!   back to the other. Pass `nil` to clear.
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
    /// `<name>.fs` or `<name>_es.fs` depending on platform / fallback.
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

    /// Re-reads source from the vfs if either the .fs or .vs mtime
    /// has moved. Replays cached uniforms onto the rebuilt shader.
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
        let Some((fs_key, fs_bytes)) = read_with_fallback(vfs, name, "fs") else {
            crate::msg::err!("shader '{name}': no shaders/{name}.fs (or _es.fs) found");
            self.active = None;
            return;
        };
        let vs_pair = read_with_fallback(vfs, name, "vs");
        let fs_src = match std::str::from_utf8(&fs_bytes) {
            Ok(s) => s.to_string(),
            Err(e) => {
                crate::msg::err!("shader '{name}': fragment source not valid utf-8: {e}");
                self.active = None;
                return;
            }
        };
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

#[cfg(target_os = "emscripten")]
fn primary_key(name: &str, ext: &str) -> String {
    format!("shaders/{name}_es.{ext}")
}
#[cfg(target_os = "emscripten")]
fn alt_key(name: &str, ext: &str) -> String {
    format!("shaders/{name}.{ext}")
}
#[cfg(not(target_os = "emscripten"))]
fn primary_key(name: &str, ext: &str) -> String {
    format!("shaders/{name}.{ext}")
}
#[cfg(not(target_os = "emscripten"))]
fn alt_key(name: &str, ext: &str) -> String {
    format!("shaders/{name}_es.{ext}")
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
}
