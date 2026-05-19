//! Verbose-mode runtime diagnostics. Active when `USAGI_VERBOSE=1`.
//!
//! Two surfaces:
//!
//! - One-shot **startup snapshot** logged from `run()` after session
//!   init: GC mode + params, resolution, palette/font source, etc.
//!   Pins the environment for bug reports without having to interrogate
//!   the user.
//! - Per-second **frame budget** + **Lua heap** summary driven by
//!   `Sampler` from inside the frame loop. Designed to catch the class
//!   of regression where everything still runs but slower — e.g. the
//!   `gc_inc(0, 0, 0)` bug where frame time went from ~16 ms to ~43 ms
//!   silently. avg / p50 / p99 / max plus an over-budget count makes
//!   spike-vs-steady-pressure visible.
//!
//! All output goes through `msg::dbg!`, which short-circuits before
//! `format_args!` runs when verbose mode is off, so the sampler's hot
//! path is just a vec push + an `Instant::elapsed` comparison.

use std::time::{Duration, Instant};

/// Frame budget for 60 FPS, in milliseconds. Frames exceeding this are
/// counted toward the per-second over-budget tally.
const FRAME_BUDGET_MS: f32 = 16.7;

/// How often to flush the rolling frame-time summary to the log. Long
/// enough to read in a terminal, short enough that a regression caught
/// during a 30-second test session produces ~30 data points.
const FLUSH_INTERVAL: Duration = Duration::from_secs(1);

/// Per-second frame-time + Lua-heap summary. Pushes a sample every
/// frame; emits a log line once `FLUSH_INTERVAL` elapses.
///
/// Enabled is captured at construction. Verbose mode is intended to be
/// a launch-time decision (env var), so we don't pay the env lookup on
/// every `record` call.
pub struct Sampler {
    enabled: bool,
    frames_ms: Vec<f32>,
    last_flush: Instant,
}

impl Sampler {
    pub fn new() -> Self {
        Self {
            enabled: crate::msg::dbg_enabled(),
            // Sized for a 1-second window at 120 FPS so the common case
            // never reallocates. Worst case (a stalled 1 FPS game)
            // grows the vec a handful of times and is irrelevant.
            frames_ms: Vec::with_capacity(120),
            last_flush: Instant::now(),
        }
    }

    /// Record one frame's wall-clock time. Cheap when verbose is off
    /// (`enabled` short-circuit); otherwise it pushes and maybe
    /// flushes. The `lua` handle is taken instead of a pre-fetched
    /// `used_memory()` value so that the mlua lock + userdata read are
    /// skipped entirely on the off path. Only the once-a-second flush
    /// actually consults it.
    pub fn record(&mut self, dt_seconds: f32, lua: &mlua::Lua) {
        if !self.enabled {
            return;
        }
        self.frames_ms.push(dt_seconds * 1000.0);
        if self.last_flush.elapsed() >= FLUSH_INTERVAL {
            self.flush(lua.used_memory());
        }
    }

    fn flush(&mut self, lua_heap_bytes: usize) {
        if self.frames_ms.is_empty() {
            self.last_flush = Instant::now();
            return;
        }
        // Copy + sort for percentiles. With ~60-120 entries this is
        // microseconds; not worth a heap-based selection algorithm.
        let mut sorted = self.frames_ms.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let n = sorted.len();
        let avg = sorted.iter().sum::<f32>() / n as f32;
        let p50 = sorted[n / 2];
        let p99 = sorted[((n * 99) / 100).min(n - 1)];
        let max = sorted[n - 1];
        let over_budget = self
            .frames_ms
            .iter()
            .filter(|&&t| t > FRAME_BUDGET_MS)
            .count();
        let heap_kb = lua_heap_bytes / 1024;
        crate::msg::dbg!(
            "frame avg {avg:.2}ms (p50 {p50:.2} / p99 {p99:.2} / max {max:.2}); over-budget {over_budget}/{n}; lua heap {heap_kb} KB"
        );
        self.frames_ms.clear();
        self.last_flush = Instant::now();
    }
}

/// One-shot environment snapshot emitted at startup when verbose is
/// on. Fields are kept narrow on purpose: each entry should help
/// reproduce a bug report or rule out a configuration as the cause.
/// raylib's own init chatter covers GL/audio details.
pub struct StartupSnapshot<'a> {
    pub build_profile: &'static str,
    pub platform: &'a str,
    pub gc_pause: i32,
    pub gc_stepmul: i32,
    pub gc_stepsize: i32,
    pub game_w: f32,
    pub game_h: f32,
    pub pixel_perfect: bool,
    pub sprite_size: i32,
    pub pause_menu: bool,
    pub palette_custom: bool,
    pub font_custom: bool,
    pub script_name: &'a str,
    pub lua_heap_bytes: usize,
}

impl StartupSnapshot<'_> {
    /// Compile-time profile string. Helps catch "user accidentally ran
    /// a debug build and is reporting perf numbers" cases.
    pub fn build_profile() -> &'static str {
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    }

    pub fn emit(&self) {
        if !crate::msg::dbg_enabled() {
            return;
        }
        let palette = if self.palette_custom {
            "custom"
        } else {
            "pico-8"
        };
        let font = if self.font_custom {
            "custom"
        } else {
            "bundled"
        };
        let pixel_perfect = if self.pixel_perfect { "on" } else { "off" };
        let pause_menu = if self.pause_menu { "on" } else { "off" };
        let heap_kb = self.lua_heap_bytes / 1024;
        crate::msg::dbg!("-- startup snapshot --");
        crate::msg::dbg!(
            "build {profile} on {platform}",
            profile = self.build_profile,
            platform = self.platform
        );
        crate::msg::dbg!(
            "gc inc pause={pause} stepmul={stepmul} stepsize={stepsize}",
            pause = self.gc_pause,
            stepmul = self.gc_stepmul,
            stepsize = self.gc_stepsize
        );
        crate::msg::dbg!(
            "resolution {w}x{h} pixel-perfect={pp} sprite-size={sz}",
            w = self.game_w as i32,
            h = self.game_h as i32,
            pp = pixel_perfect,
            sz = self.sprite_size
        );
        crate::msg::dbg!(
            "pause-menu={pm} palette={pal} font={font}",
            pm = pause_menu,
            pal = palette,
            font = font
        );
        crate::msg::dbg!(
            "script={script} lua-heap-after-init={kb} KB",
            script = self.script_name,
            kb = heap_kb
        );
        crate::msg::dbg!("-- end startup snapshot --");
    }
}
