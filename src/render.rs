//! Rendering helpers that live outside the main game loop: final RT-to-
//! window blit and the on-screen error overlay.

use crate::config::Resolution;
use sola_raylib::prelude::*;

/// Maps the game's render target onto the window. Returned tuple is
/// `(scale, top_left_x, top_left_y)`: the integer-or-fractional upscale
/// applied to the RT, and the screen-space pixel where the upscaled
/// texture's top-left corner lands. Shared with `crate::input` so mouse
/// coords can be inverted back into game space using exactly the same
/// transform that drew the frame; if these drift, click positions will
/// fall offset from where the player visually aimed.
///
/// `res` is the configured render resolution (default 320x180;
/// overridable via `_config().game_width / game_height`).
pub fn game_view_transform(
    screen_w: i32,
    screen_h: i32,
    res: Resolution,
    pixel_perfect: bool,
) -> (f32, f32, f32) {
    let mut scale = (screen_w as f32 / res.w).min(screen_h as f32 / res.h);
    if pixel_perfect {
        scale = scale.floor();
    }
    if scale < 1.0 {
        scale = 1.0;
    }
    let scaled_w = res.w * scale;
    let scaled_h = res.h * scale;
    let top_left_x = (screen_w / 2) as f32 - scaled_w / 2.0;
    let top_left_y = (screen_h / 2) as f32 - scaled_h / 2.0;
    (scale, top_left_x, top_left_y)
}

/// Draws the game's render target to the screen, scaled to fit.
/// Generic over the draw handle so this composes with `RaylibShaderMode`
/// (the post-process wrapper used by `gfx.shader_set`) as well as the
/// plain `RaylibDrawHandle`.
///
/// `shake` is an offset in *game pixels* (not screen pixels) added to
/// the dest rect after upscaling, so `effect.screen_shake(t, 4)` looks
/// the same regardless of window size.
pub fn draw_render_target<D: RaylibDraw>(
    d: &mut D,
    rt: &mut RenderTexture2D,
    screen_w: i32,
    screen_h: i32,
    res: Resolution,
    pixel_perfect: bool,
    shake: (f32, f32),
) {
    let (scale, _, _) = game_view_transform(screen_w, screen_h, res, pixel_perfect);
    let scaled_w = res.w * scale;
    let scaled_h = res.h * scale;
    let (sx, sy) = shake;
    let dest_rect = Rectangle {
        x: (screen_w / 2) as f32 + sx * scale,
        y: (screen_h / 2) as f32 + sy * scale,
        width: scaled_w,
        height: scaled_h,
    };
    let origin = Vector2::new(scaled_w / 2.0, scaled_h / 2.0);

    d.draw_texture_pro(
        rt.texture(),
        Rectangle {
            x: 0.0,
            y: 0.0,
            width: res.w,
            height: -res.h,
        },
        dest_rect,
        origin,
        0.,
        Color::WHITE,
    );
}

/// Draws the game RT into another game-sized RT without window
/// letterboxing. Native captures use this to bake the active shader
/// into a clean game-resolution source before PNG/GIF readback.
#[cfg(not(target_os = "emscripten"))]
pub fn draw_render_target_native<D: RaylibDraw>(
    d: &mut D,
    rt: &mut RenderTexture2D,
    res: Resolution,
) {
    d.draw_texture_pro(
        rt.texture(),
        Rectangle {
            x: 0.0,
            y: 0.0,
            width: res.w,
            height: -res.h,
        },
        Rectangle {
            x: 0.0,
            y: 0.0,
            width: res.w,
            height: res.h,
        },
        Vector2::zero(),
        0.,
        Color::WHITE,
    );
}

/// Greedy word-wrap to `max_w` pixels using `measure`. Tokens longer
/// than `max_w` (Lua error chunk paths) are hard-broken per char so
/// nothing renders past the edge. Empty input lines pass through as
/// empty output lines. `measure` is a closure so tests can swap in a
/// fixed-width fake without a Font.
fn wrap_to_width<M: Fn(&str) -> f32>(measure: M, text: &str, max_w: f32) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            out.push(String::new());
            continue;
        }
        let mut current = String::new();
        // `split_inclusive` keeps trailing whitespace attached to each
        // token so inter-word spacing survives across the wrap boundary
        // (where it would otherwise get trimmed away).
        for token in line.split_inclusive(char::is_whitespace) {
            let candidate = format!("{current}{token}");
            if measure(&candidate) <= max_w {
                current = candidate;
                continue;
            }
            if !current.is_empty() {
                out.push(current.trim().to_string());
                current.clear();
            }
            if measure(token) <= max_w {
                current.push_str(token);
                continue;
            }
            for ch in token.chars() {
                let mut trial = current.clone();
                trial.push(ch);
                if measure(&trial) > max_w && !current.is_empty() {
                    out.push(current.trim().to_string());
                    current.clear();
                }
                current.push(ch);
            }
        }
        if !current.is_empty() {
            out.push(current.trim().to_string());
        }
    }
    out
}

/// Draws a full-width error banner at the bottom of the window. Shown only
/// when user Lua has errored; cleared on successful reload or F5 reset.
pub fn draw_error_overlay(
    d: &mut RaylibDrawHandle,
    font: &Font,
    err: &str,
    screen_w: i32,
    screen_h: i32,
) {
    const PADDING: i32 = 12;
    const TITLE_SIZE: f32 = 18.0;
    const MSG_SIZE: f32 = TITLE_SIZE;
    const FOOTER_SIZE: f32 = TITLE_SIZE;
    const LINE_H: f32 = MSG_SIZE + 4.0;
    const MAX_LINES: usize = 8;

    let max_w = ((screen_w - PADDING * 2).max(0)) as f32;
    let wrapped = wrap_to_width(|s| font.measure_text(s, MSG_SIZE, 0.0).x, err, max_w);
    let shown = wrapped.len().min(MAX_LINES) as f32;
    let truncated = wrapped.len() > MAX_LINES;
    let footer = "fix & save to reload   \u{00b7}   Ctrl+R or F5 to reset";

    let content_h = TITLE_SIZE
        + 8.0
        + shown * LINE_H
        + if truncated { LINE_H } else { 0.0 }
        + 10.0
        + FOOTER_SIZE;
    let box_h = content_h as i32 + PADDING * 2;
    let box_y = screen_h - box_h;

    d.draw_rectangle(0, box_y, screen_w, box_h, Color::new(30, 10, 10, 235));
    d.draw_rectangle(0, box_y, screen_w, 2, Color::new(220, 60, 60, 255));

    let mut y = (box_y + PADDING) as f32;
    d.draw_text_ex(
        font,
        "Lua error",
        Vector2::new(PADDING as f32, y),
        TITLE_SIZE,
        0.0,
        Color::new(220, 60, 60, 255),
    );
    y += TITLE_SIZE + 8.0;

    for line in wrapped.iter().take(MAX_LINES) {
        d.draw_text_ex(
            font,
            line,
            Vector2::new(PADDING as f32, y),
            MSG_SIZE,
            0.0,
            Color::WHITE,
        );
        y += LINE_H;
    }
    if truncated {
        d.draw_text_ex(
            font,
            "\u{2026}",
            Vector2::new(PADDING as f32, y),
            MSG_SIZE,
            0.0,
            Color::WHITE,
        );
        y += LINE_H;
    }

    y += 10.0;
    d.draw_text_ex(
        font,
        footer,
        Vector2::new(PADDING as f32, y),
        FOOTER_SIZE,
        0.0,
        Color::new(180, 180, 180, 255),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fixed-width measurer: every char counts as 1 unit. Lets tests
    /// use small integer widths and read the wrap behavior off the line
    /// lengths directly.
    fn ones(s: &str) -> f32 {
        s.chars().count() as f32
    }

    #[test]
    fn wraps_at_word_boundary_when_line_fits() {
        let out = wrap_to_width(ones, "the quick brown fox", 10.0);
        assert_eq!(out, vec!["the quick", "brown fox"]);
    }

    #[test]
    fn preserves_short_input_unchanged() {
        let out = wrap_to_width(ones, "hi there", 80.0);
        assert_eq!(out, vec!["hi there"]);
    }

    #[test]
    fn hard_breaks_token_longer_than_max_width() {
        // The chunk-name shape that motivates this: a single path-like
        // token wider than the box. Has to break mid-token or the box
        // can't contain it.
        let out = wrap_to_width(ones, "/very/long/path/main.lua:42", 10.0);
        assert_eq!(out, vec!["/very/long", "/path/main", ".lua:42"]);
    }

    #[test]
    fn long_token_after_short_word_starts_a_new_line_first() {
        // "ok " fits; the long token doesn't fit appended, so the short
        // word flushes first, then the long token hard-breaks on its own.
        let out = wrap_to_width(ones, "ok /very/long/path", 10.0);
        assert_eq!(out, vec!["ok", "/very/long", "/path"]);
    }

    #[test]
    fn preserves_empty_input_lines_as_blank_output_lines() {
        let out = wrap_to_width(ones, "first\n\nsecond", 80.0);
        assert_eq!(out, vec!["first", "", "second"]);
    }

    #[test]
    fn empty_input_produces_no_lines() {
        let out = wrap_to_width(ones, "", 80.0);
        assert!(out.is_empty());
    }

    #[test]
    fn honors_each_input_line_independently() {
        // Wrap is per-input-line: a newline in the source is a hard
        // break regardless of how much room is left on the current line.
        let out = wrap_to_width(ones, "ab\ncd", 80.0);
        assert_eq!(out, vec!["ab", "cd"]);
    }

    #[test]
    fn variable_width_measurer_drives_break_position() {
        // Mimic real fonts: 'm' is wider than 'i'. With max_w=4 and
        // i=1, m=3: "iiii" fits (4) but "iiiim" (4+3=7) doesn't, so
        // the wrap lands before the 'm'.
        let measure = |s: &str| {
            s.chars()
                .map(|c| if c == 'm' { 3.0 } else { 1.0 })
                .sum::<f32>()
        };
        let out = wrap_to_width(measure, "iiii m", 4.0);
        assert_eq!(out, vec!["iiii", "m"]);
    }
}
