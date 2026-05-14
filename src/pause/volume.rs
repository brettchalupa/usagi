//! Volume controls for the Top view's Music / SFX rows. Five-bar
//! meter with Pico-8-style 20% steps. Pure helpers so the bar layout
//! and step math can be unit-tested without a raylib handle.

use crate::palette;
use crate::palette::Pal;
use sola_raylib::prelude::*;

const VOLUME_STEPS: f32 = 5.0;
const VOLUME_STEP: f32 = 1.0 / VOLUME_STEPS;

/// Snaps `current` to the nearest step and bumps one position in
/// `dir` (-1 / +1). Snap-then-step keeps the meter coherent if
/// settings ever land between grid points (loaded from disk, etc.).
pub(super) fn step_volume(current: f32, dir: i32) -> f32 {
    let snapped = (current * VOLUME_STEPS).round() / VOLUME_STEPS;
    let next = snapped + dir as f32 * VOLUME_STEP;
    next.clamp(0.0, 1.0)
}

/// Number of bars to render filled (0..=5).
pub(super) fn volume_bars_filled(v: f32) -> usize {
    (v.clamp(0.0, 1.0) * VOLUME_STEPS).round() as usize
}

/// Renders the five-cell meter plus a percentage readout to the right.
/// Drops the percentage readout if it would clip the game's right edge
/// (matters at narrow widths like 128x128, where the label + bars +
/// `100%` would overflow). Bars themselves still render.
pub(super) fn draw_volume_bars<D: RaylibDraw>(
    d: &mut D,
    font: &Font,
    x: f32,
    y: f32,
    v: f32,
    res: crate::config::Resolution,
) {
    let cell_w = 6.0_f32;
    let cell_h = (crate::font::MONOGRAM_SIZE as f32 * 0.7).round();
    let gap = 2.0_f32;
    let cell_top = y + (crate::font::MONOGRAM_SIZE as f32 - cell_h) * 0.5;
    let filled = volume_bars_filled(v);
    let total = VOLUME_STEPS as usize;
    let color = palette::color(Pal::White);
    for i in 0..total {
        let cx = x + (i as f32) * (cell_w + gap);
        if i < filled {
            d.draw_rectangle(
                cx as i32,
                cell_top as i32,
                cell_w as i32,
                cell_h as i32,
                color,
            );
        } else {
            d.draw_rectangle_lines(
                cx as i32,
                cell_top as i32,
                cell_w as i32,
                cell_h as i32,
                color,
            );
        }
    }
    let pct = (v.clamp(0.0, 1.0) * 100.0).round() as i32;
    let pct_text = format!("{pct}%");
    let bars_w = (total as f32) * cell_w + ((total - 1) as f32) * gap;
    let pct_x = x + bars_w + 6.0;
    let pct_m = font.measure_text(&pct_text, crate::font::MONOGRAM_SIZE as f32, 0.0);
    let margin = 4.0;
    if pct_x + pct_m.x + margin <= res.w {
        d.draw_text_ex(
            font,
            &pct_text,
            Vector2::new(pct_x, y),
            crate::font::MONOGRAM_SIZE as f32,
            0.0,
            color,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_volume_walks_six_levels() {
        let mut v = 0.0;
        for expected in [0.2, 0.4, 0.6, 0.8, 1.0, 1.0] {
            v = step_volume(v, 1);
            assert!((v - expected).abs() < 1e-5, "got {v} expected {expected}");
        }
        for expected in [0.8, 0.6, 0.4, 0.2, 0.0, 0.0] {
            v = step_volume(v, -1);
            assert!((v - expected).abs() < 1e-5, "got {v} expected {expected}");
        }
    }

    #[test]
    fn step_volume_snaps_offgrid_value() {
        // 0.55 should snap to 0.6 before stepping; +1 → 0.8.
        let v = step_volume(0.55, 1);
        assert!((v - 0.8).abs() < 1e-5);
    }

    #[test]
    fn volume_bars_filled_maps_each_step() {
        assert_eq!(volume_bars_filled(0.0), 0);
        assert_eq!(volume_bars_filled(0.2), 1);
        assert_eq!(volume_bars_filled(0.4), 2);
        assert_eq!(volume_bars_filled(0.6), 3);
        assert_eq!(volume_bars_filled(0.8), 4);
        assert_eq!(volume_bars_filled(1.0), 5);
    }
}
