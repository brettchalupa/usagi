//! In-game capture: rolling GIF buffer (F9 / Cmd+G / Ctrl+G) and PNG
//! screenshots (F8 / Cmd+F / Ctrl+F). Both write to the user's
//! Downloads dir by default (see `default_captures_dir`) so shipped
//! binaries land artifacts somewhere the player can find regardless of
//! cwd.
//!
//! GIF pipeline (rolling buffer): every frame, accumulate real elapsed
//! time. Once at least one frame's worth of time at the 30fps floor has
//! passed, read the game render target's pixel data back from the GPU,
//! map each pixel to a palette index via a fixed lookup, and push the
//! indexed buffer + actual elapsed centiseconds onto a ring sized to
//! hold the last ~5 seconds. The expensive work (2x upscale + LZW +
//! disk write) is deferred to the hotkey moment AND moved to a
//! background thread, so the per-frame cost while idle is just the
//! cheap palette quantize and the save itself doesn't stall the main
//! loop (no visible frame hitch, no audio underrun). The per-frame
//! `Arc<[u8]>` makes the snapshot handed to the worker free of any
//! large copies.
//!
//! Screenshot pipeline: same RT readback, but one-shot. Flip
//! vertically (RTs are stored bottom-up under OpenGL), upscale 2x via
//! nearest-neighbor, and hand to raylib's `ExportImage` which picks
//! PNG by file extension.
//!
//! Both paths produce 2x-upscaled output (640×360 at the default
//! resolution).
//!
//! Native-only, doesn't work in web; `cfg(not(target_os = "emscripten"))` in `main.rs`.

use sola_raylib::prelude::*;
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::palette;

/// Lower bound on the per-frame delay written to the GIF, in centiseconds.
/// Frames captured within this window are coalesced (accumulated dt
/// carries forward into the next capture). Picked to be roughly a 30fps
/// floor on the output, which both halves the disk-side work versus
/// recording every game frame and keeps playback inside the range that
/// most GIF viewers honor cleanly.
const MIN_DELAY_CS: u16 = 3;

/// Target retained-buffer duration in seconds. At ~30fps after the
/// frame-skip and 320×180 indexed pre-upscale storage, this works out to
/// roughly 9MB of resident memory, regardless of how long the session
/// has been running.
const BUFFER_SECONDS: f32 = 5.0;

/// Nearest-neighbor upscale applied at save time so the resulting GIF
/// reads well when shared.
const RECORDING_SCALE: u16 = 2;

/// Resolved RGB triples for the 16 palette entries, in palette-index
/// order. GIF palette slots are 0-based by spec, so usagi slots 1..=16
/// fill GIF positions 0..15.
fn palette_rgb() -> [u8; 48] {
    let mut out = [0u8; 48];
    for i in 0..16 {
        let c = palette::color((i + 1) as i32);
        out[i * 3] = c.r;
        out[i * 3 + 1] = c.g;
        out[i * 3 + 2] = c.b;
    }
    out
}

/// Maps an exact RGB triple back to its palette index. Built once at
/// recorder construction. Pixels that don't match any palette color
/// (e.g. antialiased font edges) fall through to the closest palette
/// entry by squared-distance lookup.
struct PaletteIndex {
    /// Fast path: exact RGB to index match. Hits for every pixel a game
    /// draws via a `COLOR_*` constant, which is ~all of them.
    exact: HashMap<(u8, u8, u8), u8>,
    /// Slow-path fallback for unrecognized RGB. Stored as a contiguous
    /// `[r, g, b, ...]` so the closest-match scan stays cache-friendly
    /// across the 16-iter loop.
    palette: [u8; 48],
}

impl PaletteIndex {
    fn new() -> Self {
        let palette = palette_rgb();
        let mut exact = HashMap::with_capacity(16);
        for i in 0..16u8 {
            let r = palette[i as usize * 3];
            let g = palette[i as usize * 3 + 1];
            let b = palette[i as usize * 3 + 2];
            exact.insert((r, g, b), i);
        }
        Self { exact, palette }
    }

    fn lookup(&self, r: u8, g: u8, b: u8) -> u8 {
        if let Some(&idx) = self.exact.get(&(r, g, b)) {
            return idx;
        }
        let mut best = 0u8;
        let mut best_dist = i32::MAX;
        for i in 0..16usize {
            let pr = self.palette[i * 3] as i32;
            let pg = self.palette[i * 3 + 1] as i32;
            let pb = self.palette[i * 3 + 2] as i32;
            let dr = pr - r as i32;
            let dg = pg - g as i32;
            let db = pb - b as i32;
            let dist = dr * dr + dg * dg + db * db;
            if dist < best_dist {
                best_dist = dist;
                best = i as u8;
            }
        }
        best
    }
}

/// One captured frame in the ring. Stores pixels at game resolution
/// (pre-upscale) as palette indices, and the actual elapsed time since
/// the previous kept frame so playback timing reflects reality even
/// across stutters. `Arc<[u8]>` so the save-on-hotkey path can snapshot
/// the buffer for a background worker with only atomic refcount bumps
/// per frame instead of copying ~10MB of pixels.
#[derive(Clone)]
struct CapturedFrame {
    indexed: Arc<[u8]>,
    delay_cs: u16,
}

/// Adds `dt` (seconds, may be slightly negative from prior carry) to
/// the accumulator and decides whether enough time has passed to keep
/// a frame. Returns `Some(delay_cs)` for the kept frame's GIF delay
/// (decrementing the accumulator by exactly that amount, with the
/// remainder carried forward so playback rate tracks real time), or
/// `None` when the floor hasn't been reached yet. Pure: the unit tests
/// drive it without a render target.
fn tick_timing(accumulated_dt: &mut f32, dt: f32) -> Option<u16> {
    *accumulated_dt += dt.max(0.0);
    let min_delay_seconds = MIN_DELAY_CS as f32 / 100.0;
    if *accumulated_dt < min_delay_seconds {
        return None;
    }
    let delay_cs_raw = (*accumulated_dt * 100.0).round() as i32;
    let delay_cs = delay_cs_raw.clamp(MIN_DELAY_CS as i32, u16::MAX as i32) as u16;
    // Signed carry: a delay rounded UP (e.g. 4cs written for a 3.67cs
    // real frame) leaves a tiny negative debt that the next
    // accumulation pays back. Without the signed carry a 60fps game
    // drifts ~10% slow because every kept frame loses ~0.3cs to
    // rounding.
    *accumulated_dt -= delay_cs as f32 / 100.0;
    Some(delay_cs)
}

/// Rolling buffer GIF recorder. Holds the last ~5 seconds of game
/// frames in memory and writes a GIF on demand. Always live in the
/// session; there is no start/stop toggle.
pub struct Recorder {
    frames: VecDeque<CapturedFrame>,
    /// Total real time held in the ring, in seconds. Used to evict old
    /// frames and to log "how much we saved" on hotkey.
    total_seconds: f32,
    /// Pending elapsed real time since the previous kept frame.
    /// Accumulates across skipped frames so a coalesced delay still
    /// reflects real time.
    accumulated_dt: f32,
    palette_index: PaletteIndex,
    /// Source frame dimensions of the buffered frames. Set on the first
    /// captured frame. If the game's render resolution changes
    /// mid-session the buffer is discarded so the encoder never sees
    /// mixed dims.
    width: u16,
    height: u16,
}

impl Recorder {
    pub fn new() -> Self {
        Self {
            frames: VecDeque::new(),
            total_seconds: 0.0,
            accumulated_dt: 0.0,
            palette_index: PaletteIndex::new(),
            width: 0,
            height: 0,
        }
    }

    /// Per-frame entry point. Called every game frame after `_draw`.
    /// Accumulates `dt` (seconds, from raylib's frame time); if enough
    /// has passed to clear the 30fps floor, reads the RT back from the
    /// GPU, quantizes to palette indices, and appends to the ring.
    /// Evicts the oldest frames once total duration exceeds
    /// `BUFFER_SECONDS`.
    pub fn capture(&mut self, rt: &RenderTexture2D, dt: f32, res: crate::config::Resolution) {
        let Some(delay_cs) = tick_timing(&mut self.accumulated_dt, dt) else {
            return;
        };
        let used_seconds = delay_cs as f32 / 100.0;

        let new_w = res.w as u16;
        let new_h = res.h as u16;
        if self.width != new_w || self.height != new_h {
            // Resolution changed (live `_config` reload, etc.). The
            // GIF encoder needs uniform dims, so drop what we have.
            self.frames.clear();
            self.total_seconds = 0.0;
            self.width = new_w;
            self.height = new_h;
        }

        let Ok(image) = rt.texture().load_image() else {
            crate::msg::err!("recorder: failed to read RT pixels");
            return;
        };
        let pixels = image.get_image_data();
        let src_w = self.width as usize;
        let src_h = self.height as usize;
        if pixels.len() != src_w * src_h {
            crate::msg::err!(
                "recorder: unexpected RT size: got {}, expected {}",
                pixels.len(),
                src_w * src_h
            );
            return;
        }
        // RTs are bottom-up under OpenGL; flip during the palette walk
        // so the buffered indexed bytes are already top-down. Save time
        // can then upscale linearly without re-flipping.
        let mut indexed = vec![0u8; src_w * src_h];
        for sy in 0..src_h {
            let flipped = src_h - 1 - sy;
            let src_off = flipped * src_w;
            let dst_off = sy * src_w;
            for sx in 0..src_w {
                let p = pixels[src_off + sx];
                indexed[dst_off + sx] = self.palette_index.lookup(p.r, p.g, p.b);
            }
        }

        self.frames.push_back(CapturedFrame {
            indexed: Arc::from(indexed.into_boxed_slice()),
            delay_cs,
        });
        self.total_seconds += used_seconds;
        while self.total_seconds > BUFFER_SECONDS {
            match self.frames.pop_front() {
                Some(front) => self.total_seconds -= front.delay_cs as f32 / 100.0,
                None => {
                    self.total_seconds = 0.0;
                    break;
                }
            }
        }
    }

    /// Spawns a background thread to encode the current ring as a GIF
    /// at `dest_dir/<prefix>-...gif`. Returns immediately so the main
    /// loop never stalls on the LZW pass or the disk write (which is
    /// what used to cause the visible frame hitch and audio underrun
    /// when the recorder was synchronous).
    ///
    /// The buffer is left intact so a player can hit the hotkey again
    /// for another copy of overlapping or subsequent moments. The
    /// snapshot taken for the worker is a cheap `Arc` clone per frame,
    /// so the main thread pays only refcount bumps for ~150 frames at
    /// 30fps × 5s.
    ///
    /// Returns the chosen output path on success (so the caller can
    /// log "saved to X" immediately) or `Ok(None)` if the buffer is
    /// empty. The actual write happens on the worker; its success or
    /// failure logs via `msg::info!` / `msg::err!` when done.
    pub fn save(&self, dest_dir: &Path, prefix: &str) -> std::io::Result<Option<PathBuf>> {
        if self.frames.is_empty() {
            crate::msg::warn!("recorder: nothing buffered yet, save skipped");
            return Ok(None);
        }
        let path = next_capture_path(dest_dir, prefix, "gif")?;
        // Reserve the filename on the main thread so a rapid second
        // hotkey press doesn't collide with this still-pending write:
        // `next_capture_path` checks `.exists()` and would otherwise
        // hand the same name to two workers in flight. The worker
        // re-opens and truncates this empty placeholder when it gets
        // around to encoding.
        File::create(&path)?;
        let width = self.width;
        let height = self.height;
        let total_seconds = self.total_seconds;
        let frames: Vec<CapturedFrame> = self.frames.iter().cloned().collect();
        let frame_count = frames.len();
        let path_for_thread = path.clone();
        std::thread::spawn(
            move || match write_gif(&path_for_thread, width, height, &frames) {
                Ok(()) => crate::msg::info!(
                    "recording saved: {} ({} frame(s), {:.1}s)",
                    path_for_thread.display(),
                    frame_count,
                    total_seconds,
                ),
                Err(e) => crate::msg::err!(
                    "recording write failed for {}: {e}",
                    path_for_thread.display(),
                ),
            },
        );
        Ok(Some(path))
    }
}

/// Encodes a snapshot of captured frames into a GIF at `path`. Pure;
/// invoked from the save worker thread so the main loop stays free.
fn write_gif(
    path: &Path,
    width: u16,
    height: u16,
    frames: &[CapturedFrame],
) -> std::io::Result<()> {
    let file = File::create(path)?;
    let writer = BufWriter::new(file);
    let palette = palette_rgb();
    let scale = RECORDING_SCALE;
    let gif_w = width.saturating_mul(scale);
    let gif_h = height.saturating_mul(scale);
    let mut encoder = gif::Encoder::new(writer, gif_w, gif_h, &palette).map_err(io_err)?;
    encoder.set_repeat(gif::Repeat::Infinite).map_err(io_err)?;
    let src_w = width as usize;
    let src_h = height as usize;
    let s = scale as usize;
    let out_w = src_w * s;
    let out_h = src_h * s;
    for f in frames {
        let mut upscaled = vec![0u8; out_w * out_h];
        for sy in 0..src_h {
            let src_row = sy * src_w;
            let dst_y0 = sy * s;
            for sx in 0..src_w {
                let idx = f.indexed[src_row + sx];
                let dst_x0 = sx * s;
                for dy in 0..s {
                    let row_off = (dst_y0 + dy) * out_w;
                    for dx in 0..s {
                        upscaled[row_off + dst_x0 + dx] = idx;
                    }
                }
            }
        }
        let mut frame = gif::Frame::from_indexed_pixels(gif_w, gif_h, upscaled, None);
        frame.delay = f.delay_cs;
        encoder.write_frame(&frame).map_err(io_err)?;
    }
    drop(encoder);
    Ok(())
}

impl Default for Recorder {
    fn default() -> Self {
        Self::new()
    }
}

fn io_err(e: gif::EncodingError) -> std::io::Error {
    std::io::Error::other(format!("gif encoder: {e}"))
}

/// Where GIFs and screenshots land by default. The user's Downloads
/// directory on all three desktop platforms: easy to find, universally
/// understood as the "stuff I just got" bucket, and writable from
/// shipped binaries even when the launch cwd is weird (macOS .app
/// bundles cwd to `/`). Falls back to `<cwd>/captures` if the OS
/// doesn't expose a Downloads dir, which keeps `usagi dev` / `usagi
/// run` working in odd shells and CI.
pub fn default_captures_dir() -> PathBuf {
    if let Some(dirs) = directories::UserDirs::new()
        && let Some(dl) = dirs.download_dir()
    {
        return dl.to_path_buf();
    }
    let fallback = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("captures");
    crate::msg::warn!(
        "capture: no Downloads dir from the OS; falling back to {}",
        fallback.display(),
    );
    fallback
}

/// Builds a unique timestamped path inside `dest_dir` with the given
/// `prefix` and `ext`. Creates the directory if missing. Format:
/// `<prefix>-YYYYMMDD-HHMMSS.<ext>`. If two captures start in the
/// same second, a `-N` suffix is appended. The prefix is the game's
/// short name (typically derived from `_config().game_id`) so users
/// can tell different projects' captures apart at a glance.
/// Shared between the GIF recorder and the PNG screenshot helper.
pub(crate) fn next_capture_path(
    dest_dir: &Path,
    prefix: &str,
    ext: &str,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dest_dir)?;
    let now = time::OffsetDateTime::now_utc();
    let stem = format!(
        "{prefix}-{:04}{:02}{:02}-{:02}{:02}{:02}",
        now.year(),
        now.month() as u8,
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
    );
    let mut candidate = dest_dir.join(format!("{stem}.{ext}"));
    let mut n: u32 = 1;
    // Cap defensively: if a polluted dir somehow holds thousands of
    // matching files we'd rather error out than spin on `.exists()`.
    const MAX_SUFFIX: u32 = 1000;
    while candidate.exists() {
        if n > MAX_SUFFIX {
            return Err(std::io::Error::other(format!(
                "no free capture filename in {} after {MAX_SUFFIX} attempts",
                dest_dir.display(),
            )));
        }
        candidate = dest_dir.join(format!("{stem}-{n}.{ext}"));
        n += 1;
    }
    Ok(candidate)
}

/// Reads the game render target back from the GPU and writes it to a
/// timestamped PNG inside `dest_dir` at `RECORDING_SCALE` × game size.
/// Returns the saved path on success. The screenshot pipeline reuses
/// the same upscale factor as the GIF recorder so both file types
/// land at matching pixel dimensions (640×360 by default), and goes
/// through `next_capture_path` so file naming and the `captures/`
/// dir creation behave identically across both kinds of capture.
pub fn save_screenshot(
    rt: &RenderTexture2D,
    dest_dir: &Path,
    prefix: &str,
    res: crate::config::Resolution,
) -> std::io::Result<PathBuf> {
    let mut image = rt
        .texture()
        .load_image()
        .map_err(|e| std::io::Error::other(format!("read RT pixels: {e}")))?;
    image.flip_vertical();
    let scale = RECORDING_SCALE as i32;
    image.resize_nn((res.w as i32) * scale, (res.h as i32) * scale);
    let path = next_capture_path(dest_dir, prefix, "png")?;
    let path_str = path
        .to_str()
        .ok_or_else(|| std::io::Error::other("screenshot path is not valid UTF-8"))?;
    image.export_image(path_str);
    crate::msg::info!("screenshot saved: {}", path.display());
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_lookup_returns_exact_index_for_each_palette_color() {
        let p = PaletteIndex::new();
        for gif_idx in 0..16u8 {
            let c = palette::color((gif_idx + 1) as i32);
            assert_eq!(
                p.lookup(c.r, c.g, c.b),
                gif_idx,
                "gif idx {gif_idx} (usagi slot {}) should round-trip",
                gif_idx + 1
            );
        }
    }

    #[test]
    fn palette_lookup_picks_nearest_for_off_palette_rgb() {
        let p = PaletteIndex::new();
        assert_eq!(p.lookup(1, 1, 1), 0);
        assert_eq!(p.lookup(255, 0, 0), (palette::Pal::Red as i32 - 1) as u8);
    }

    #[test]
    fn next_capture_path_creates_dir_and_uses_prefix() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("captures");
        let path = next_capture_path(&dest, "snake", "gif").unwrap();
        assert!(dest.exists(), "dest dir should be created");
        assert_eq!(
            path.extension().and_then(|s: &std::ffi::OsStr| s.to_str()),
            Some("gif")
        );
        let stem = path.file_stem().unwrap().to_str().unwrap();
        assert!(stem.starts_with("snake-"), "got: {stem}");
    }

    #[test]
    fn next_capture_path_honors_extension_arg() {
        let tmp = tempfile::tempdir().unwrap();
        let png = next_capture_path(tmp.path(), "usagi", "png").unwrap();
        assert_eq!(
            png.extension().and_then(|s: &std::ffi::OsStr| s.to_str()),
            Some("png")
        );
    }

    #[test]
    fn next_capture_path_avoids_collision_with_suffix() {
        let tmp = tempfile::tempdir().unwrap();
        let first = next_capture_path(tmp.path(), "usagi", "gif").unwrap();
        std::fs::write(&first, b"").unwrap();
        let second = next_capture_path(tmp.path(), "usagi", "gif").unwrap();
        assert_ne!(first, second, "should not return the same path twice");
    }

    /// Direct timing-math test helper. Calls the same `tick_timing`
    /// the real `capture` uses, but stubs the GPU readback with a
    /// zeroed indexed buffer so eviction can be exercised without a
    /// raylib context.
    fn push_synthetic_frame(rec: &mut Recorder, dt: f32, w: u16, h: u16) -> Option<u16> {
        let delay_cs = tick_timing(&mut rec.accumulated_dt, dt)?;
        rec.width = w;
        rec.height = h;
        rec.frames.push_back(CapturedFrame {
            indexed: Arc::from(vec![0u8; w as usize * h as usize].into_boxed_slice()),
            delay_cs,
        });
        rec.total_seconds += delay_cs as f32 / 100.0;
        while rec.total_seconds > BUFFER_SECONDS {
            match rec.frames.pop_front() {
                Some(front) => rec.total_seconds -= front.delay_cs as f32 / 100.0,
                None => {
                    rec.total_seconds = 0.0;
                    break;
                }
            }
        }
        Some(delay_cs)
    }

    #[test]
    fn sub_floor_dts_coalesce_into_one_kept_frame() {
        // Two 60fps game frames (each 1/60s ~= 16.67ms) should produce
        // one captured GIF frame at the 30fps floor. The frame's delay
        // should round to ~3 cs.
        let mut rec = Recorder::new();
        let dt = 1.0 / 60.0;
        assert_eq!(push_synthetic_frame(&mut rec, dt, 8, 8), None);
        let kept = push_synthetic_frame(&mut rec, dt, 8, 8);
        assert_eq!(kept, Some(3));
        assert_eq!(rec.frames.len(), 1);
    }

    #[test]
    fn long_dt_is_kept_as_is_above_the_floor() {
        // A 100ms frame (game stuttered) is well above the floor and
        // should be kept with the full delay so playback reflects the
        // real elapsed time.
        let mut rec = Recorder::new();
        let kept = push_synthetic_frame(&mut rec, 0.1, 8, 8);
        assert_eq!(kept, Some(10));
        assert_eq!(rec.frames.len(), 1);
    }

    #[test]
    fn ring_evicts_oldest_once_total_exceeds_buffer_seconds() {
        // Push enough 100ms frames to overshoot the 5s buffer and
        // confirm the front gets popped while the back keeps growing.
        let mut rec = Recorder::new();
        for _ in 0..60 {
            push_synthetic_frame(&mut rec, 0.1, 8, 8);
        }
        assert!(
            rec.total_seconds <= BUFFER_SECONDS + 0.01,
            "ring should keep total ~5s, got {}",
            rec.total_seconds
        );
        assert!(rec.frames.len() <= 50);
    }

    #[test]
    fn carry_remainder_keeps_average_close_to_real_time() {
        // At 60fps the rounded 3cs floor would naively drop ~10% per
        // captured frame; the carry-remainder logic compensates so the
        // total accumulated GIF time tracks the wall-clock dt.
        let mut rec = Recorder::new();
        let dt = 1.0 / 60.0;
        let mut total_kept_cs: u32 = 0;
        let frame_count = 600; // 10 seconds of game time at 60fps
        for _ in 0..frame_count {
            if let Some(delay) = push_synthetic_frame(&mut rec, dt, 8, 8) {
                total_kept_cs += delay as u32;
            }
        }
        let real_seconds = frame_count as f32 * dt;
        let kept_seconds = total_kept_cs as f32 / 100.0;
        // Allow generous tolerance because the ring keeps only the
        // last 5s; the eviction subtracts what fell off too. Test that
        // total_seconds (kept after eviction) tracks 5s closely.
        assert!(
            (rec.total_seconds - BUFFER_SECONDS).abs() < 0.05,
            "ring should hold ~{BUFFER_SECONDS}s, got {}",
            rec.total_seconds
        );
        // And confirm the accumulated kept cs comes close to the real
        // elapsed time without an obvious 10% drop.
        assert!(
            (kept_seconds - real_seconds).abs() < 0.1,
            "kept ~{kept_seconds}s vs real {real_seconds}s",
        );
    }
}
