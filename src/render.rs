//! Rendering helpers that live outside the main game loop: final RT-to-
//! window blit and the on-screen error overlay.

use crate::{GAME_HEIGHT, GAME_WIDTH};
use sola_raylib::prelude::*;

/// Draws the game's render target to the screen, scaled to fit.
pub fn draw_render_target(
    d: &mut RaylibDrawHandle,
    rt: &mut RenderTexture2D,
    screen_w: i32,
    screen_h: i32,
    pixel_perfect: bool,
) {
    let game_w = GAME_WIDTH;
    let game_h = GAME_HEIGHT;
    let mut scale = (screen_w as f32 / game_w).min(screen_h as f32 / game_h);
    if pixel_perfect {
        scale = scale.floor();
    }
    if scale < 1.0 {
        scale = 1.0;
    }
    let scaled_w = game_w * scale;
    let scaled_h = game_h * scale;
    let dest_rect = Rectangle {
        x: (screen_w / 2) as f32,
        y: (screen_h / 2) as f32,
        width: scaled_w,
        height: scaled_h,
    };
    let origin = Vector2::new(scaled_w / 2.0, scaled_h / 2.0);

    d.draw_texture_pro(
        rt.texture(),
        Rectangle {
            x: 0.0,
            y: 0.0,
            width: game_w,
            height: -game_h,
        },
        dest_rect,
        origin,
        0.,
        Color::WHITE,
    );
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
    // Everything renders at monogram's 16px design size: it's the only
    // size we can draw at without scaling the atlas. The previous
    // bigger-title look fought monogram's pixel-font aesthetic anyway.
    const TITLE_SIZE: f32 = 16.0;
    const MSG_SIZE: f32 = 16.0;
    const LINE_H: f32 = MSG_SIZE + 4.0;
    const FOOTER_SIZE: f32 = 16.0;
    const MAX_LINES: usize = 8;

    let lines: Vec<&str> = err.lines().collect();
    let shown = lines.len().min(MAX_LINES) as f32;
    let truncated = lines.len() > MAX_LINES;
    let footer = "fix & save to reload   \u{00b7}   F5 to reset";

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

    for line in lines.iter().take(MAX_LINES) {
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
