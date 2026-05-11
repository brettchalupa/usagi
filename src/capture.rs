//! In-game capture: GIF recording (F9 / Cmd+G) and PNG screenshots
//! (F8 / Cmd+F). Both write to `<cwd>/captures/` so games have a
//! single bucket to gitignore and players have one place to find their
//! shareable artifacts.
//!
//! GIF pipeline: each frame while recording, read the game render
//! target's pixel data back from the GPU, map each pixel to the
//! palette index via a fixed lookup, and stream a `gif::Frame`
//! straight to disk through `gif::Encoder`. Memory stays bounded
//! regardless of recording length because the encoder owns a
//! `BufWriter<File>` and writes LZW blocks as it goes.
//!
//! Screenshot pipeline: same RT readback, but one-shot. Flip
//! vertically (RTs are stored bottom-up under OpenGL), upscale 2x via
//! nearest-neighbor, and hand to raylib's `ExportImage` which picks
//! PNG by file extension.
//!
//! Both paths produce 2x-upscaled output (640×360).
//!
//! Native-only, doesn't work in web; `cfg(not(target_os = "emscripten"))` in `main.rs`.

use sola_raylib::prelude::*;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use crate::palette;

/// Centiseconds per frame written to the GIF. The game runs at 60 fps
/// (16.67 ms), but GIF delays are integer centiseconds and most viewers
/// clamp anything under 2 cs to ~10 cs anyway. 2 cs gives 50 fps
/// playback. That's a small slowdown vs. real-time, but viewers
/// render it smoothly. Going to 1 cs would technically map to 60 fps
/// but only a few viewers honor it.
const FRAME_DELAY_CS: u16 = 2;

/// Nearest-neighbor upscale applied to every recorded frame. 2x reads
/// well when sharing.
const RECORDING_SCALE: u16 = 2;

/// Resolved RGB triples for the 16 palette entries, in palette-index
/// order. Built once and reused for every captured frame. GIF palette
/// slots are 0-based by spec, so usagi slots 1..=16 fill GIF positions
/// 0..15.
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
/// recorder start. Pixels that don't match any palette color (e.g.
/// `gfx`-rendered antialiased edges from font glyphs) fall through to
/// the closest palette entry by squared-distance lookup.
///
/// Public so it can sit inside the public `Recorder::Recording`
/// variant; not part of the recorder's external interface (callers
/// only use `Recorder::{toggle, capture, is_recording}`).
pub struct PaletteIndex {
    /// Fast path: exact RGB → index match. Hits for every pixel a game
    /// draws via a `COLOR_*` constant, which is ~all of them.
    exact: HashMap<(u8, u8, u8), u8>,
    /// Slow-path fallback for unrecognized RGB. Stored as a contiguous
    /// `[r, g, b, ...]` so the closest-match search vectorizes
    /// reasonably well.
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
        // Closest-match fallback. The bundled font is monochrome and
        // gfx draws are flat fills, so this branch is rare; the cost
        // doesn't show up in profiles for typical Usagi games.
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

/// Active-recording payload. Held inside `Recorder::Recording` via a
/// `Box` so the enum's idle variant doesn't carry the ~200 bytes of
/// encoder + LUT around for the entire session. Clippy's
/// `large_enum_variant` (denied in CI) would otherwise fire because
/// `Idle` carries no data while this struct is hefty.
pub struct RecordingState {
    encoder: gif::Encoder<BufWriter<File>>,
    path: PathBuf,
    palette_index: PaletteIndex,
    frames: u32,
    /// Source RT dims at the moment recording started. Captured on
    /// start so `capture()` always matches the encoder's frame size,
    /// independent of any later config reload.
    game_w: u16,
    game_h: u16,
}

/// Top-level recorder state machine. Idle by default; user hotkey
/// toggles in and out of `Recording`. The active variant carries a
/// boxed `RecordingState` (streaming encoder, destination path,
/// palette LUT, frame counter) so the enum stays small while idle.
pub enum Recorder {
    Idle,
    Recording(Box<RecordingState>),
}

impl Recorder {
    pub fn new() -> Self {
        Recorder::Idle
    }

    pub fn is_recording(&self) -> bool {
        matches!(self, Recorder::Recording(_))
    }

    /// Toggles between recording and idle. Returns the saved path on
    /// stop so the caller can show / log it. On start, returns `None`
    /// (or an error if the file can't be opened).
    ///
    /// `dest_dir` is the directory `.gif` files land in. The recorder
    /// creates it if it doesn't exist. `prefix` is the filename
    /// prefix (typically the game's short name).
    pub fn toggle(
        &mut self,
        dest_dir: &Path,
        prefix: &str,
        res: crate::config::Resolution,
    ) -> std::io::Result<Option<PathBuf>> {
        match std::mem::replace(self, Recorder::Idle) {
            Recorder::Idle => {
                let path = next_capture_path(dest_dir, prefix, "gif")?;
                let file = File::create(&path)?;
                let writer = BufWriter::new(file);
                let palette = palette_rgb();
                let game_w = res.w as u16;
                let game_h = res.h as u16;
                let gif_w = game_w.saturating_mul(RECORDING_SCALE);
                let gif_h = game_h.saturating_mul(RECORDING_SCALE);
                let mut encoder =
                    gif::Encoder::new(writer, gif_w, gif_h, &palette).map_err(io_err)?;
                encoder.set_repeat(gif::Repeat::Infinite).map_err(io_err)?;
                crate::msg::info!("recording started: {}", path.display());
                *self = Recorder::Recording(Box::new(RecordingState {
                    encoder,
                    path,
                    palette_index: PaletteIndex::new(),
                    frames: 0,
                    game_w,
                    game_h,
                }));
                Ok(None)
            }
            Recorder::Recording(state) => {
                // Move out of the box so the GIF trailer is written and
                // the BufWriter flushed when `encoder` drops at the end
                // of this scope, before we report the save.
                let RecordingState {
                    encoder,
                    path,
                    frames,
                    ..
                } = *state;
                drop(encoder);
                crate::msg::info!("recording saved: {} ({} frame(s))", path.display(), frames);
                Ok(Some(path))
            }
        }
    }

    /// Pulls the latest RT pixels off the GPU and appends them as a
    /// frame. No-op if not currently recording. Errors are logged and
    /// dropped: a single bad frame shouldn't tear the session down,
    /// but losing the recording mid-flight to a disk-full error
    /// matters less than the user's game continuing to run.
    pub fn capture(&mut self, rt: &RenderTexture2D) {
        let Recorder::Recording(state) = self else {
            return;
        };
        let RecordingState {
            encoder,
            palette_index,
            frames,
            game_w,
            game_h,
            ..
        } = state.as_mut();
        let Ok(image) = rt.texture().load_image() else {
            crate::msg::err!("recorder: failed to read RT pixels");
            return;
        };
        let pixels = image.get_image_data();
        // Source dims are the game RT size at recording start; the GIF
        // dims (game_w * RECORDING_SCALE, game_h * RECORDING_SCALE) are
        // already baked into the encoder. Both the row-flip (RTs are
        // stored bottom-up in OpenGL) and the nearest-neighbor 2x
        // upscale happen in this single pass so we only walk the
        // indexed buffer once.
        let src_w = *game_w as usize;
        let src_h = *game_h as usize;
        let scale = RECORDING_SCALE as usize;
        let out_w = src_w * scale;
        let out_h = src_h * scale;
        let expected_src = src_w * src_h;
        if pixels.len() != expected_src {
            crate::msg::err!(
                "recorder: unexpected RT size: got {}, expected {}",
                pixels.len(),
                expected_src
            );
            return;
        }
        let mut indexed = vec![0u8; out_w * out_h];
        for sy in 0..src_h {
            let flipped = src_h - 1 - sy;
            let src_off = flipped * src_w;
            for sx in 0..src_w {
                let p = pixels[src_off + sx];
                let idx = palette_index.lookup(p.r, p.g, p.b);
                let dy0 = sy * scale;
                let dx0 = sx * scale;
                for dy in 0..scale {
                    let row_off = (dy0 + dy) * out_w;
                    for dx in 0..scale {
                        indexed[row_off + dx0 + dx] = idx;
                    }
                }
            }
        }
        let gif_w = (*game_w).saturating_mul(RECORDING_SCALE);
        let gif_h = (*game_h).saturating_mul(RECORDING_SCALE);
        let mut frame = gif::Frame::from_indexed_pixels(gif_w, gif_h, indexed, None);
        frame.delay = FRAME_DELAY_CS;
        if let Err(e) = encoder.write_frame(&frame) {
            crate::msg::err!("recorder: write_frame failed: {e}");
            return;
        }
        *frames = frames.saturating_add(1);
    }
}

fn io_err(e: gif::EncodingError) -> std::io::Error {
    std::io::Error::other(format!("gif encoder: {e}"))
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
    // UTC keeps the time crate dep light: `now_local` would need the
    // `local-offset` feature, and the timestamp here only needs to
    // produce a unique, monotonic-ish filename, not a wall clock.
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
    while candidate.exists() {
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
        // GIF palette slots are 0..15; they correspond to usagi slots
        // 1..16. The lookup returns the GIF-side index.
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
        // Black is usagi slot 1 / GIF index 0. A near-black (1,1,1)
        // should snap to it.
        assert_eq!(p.lookup(1, 1, 1), 0);
        // Bright red (255, 0, 0) is closest to palette red (255,0,77).
        // Should not pick yellow / orange / pink at this distance.
        // Red is usagi slot 9 / GIF index 8.
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
}
