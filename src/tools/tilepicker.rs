//! TilePicker tool: visualises `<project>/sprites.png` with a 1-based
//! index overlay; clicking a tile copies its index to the clipboard.

use super::{HINT_Y, PANEL_H, PANEL_W, PANEL_X, PANEL_Y};
use crate::palette::{Pal, color};
use sola_raylib::prelude::*;
use std::path::Path;

/// Matches `gfx.spr` in the runtime.
const TILE_SIZE: i32 = 16;
const PAN_SPEED: f32 = 400.0; // pixels/second, dt-scaled
const ZOOM_STEP: f32 = 0.5;
const ZOOM_MIN: f32 = 0.5;
const ZOOM_MAX: f32 = 20.;
const BG_COLORS: [Color; 3] = [Color::GRAY, Color::BLACK, Color::WHITE];

/// Viewport rect. The image + overlay are clipped to this so panning
/// doesn't bleed onto the surrounding UI.
const VIEW_X: f32 = PANEL_X + 2.0;
const VIEW_Y: f32 = PANEL_Y + 70.0;
const VIEW_W: f32 = PANEL_W - 4.0;
const VIEW_H: f32 = HINT_Y - VIEW_Y - 8.0;

pub(super) struct State {
    pub zoom: f32,
    pub pos: Vector2,
    pub show_overlay: bool,
    pub bg_idx: usize,
}

impl State {
    pub fn new() -> Self {
        Self {
            zoom: 3.0,
            pos: default_pos(),
            show_overlay: true,
            bg_idx: 0,
        }
    }
}

fn default_pos() -> Vector2 {
    Vector2::new(VIEW_X + 40.0, VIEW_Y + 40.0)
}

/// Returns an optional toast message (e.g. "copied sprite 7 to clipboard")
/// for the wrapper to display.
pub(super) fn handle_input(
    rl: &mut RaylibHandle,
    state: &mut State,
    texture: Option<&Texture2D>,
    dt: f32,
) -> Option<String> {
    // Pan (hold). WASD moves the camera, so the image translates opposite.
    let pan = PAN_SPEED * dt;
    if rl.is_key_down(KeyboardKey::KEY_A) {
        state.pos.x += pan;
    }
    if rl.is_key_down(KeyboardKey::KEY_D) {
        state.pos.x -= pan;
    }
    if rl.is_key_down(KeyboardKey::KEY_W) {
        state.pos.y += pan;
    }
    if rl.is_key_down(KeyboardKey::KEY_S) {
        state.pos.y -= pan;
    }
    if rl.is_key_pressed(KeyboardKey::KEY_Q) {
        state.zoom = (state.zoom - ZOOM_STEP).max(ZOOM_MIN);
    }
    if rl.is_key_pressed(KeyboardKey::KEY_E) {
        state.zoom = (state.zoom + ZOOM_STEP).min(ZOOM_MAX);
    }
    if rl.is_key_pressed(KeyboardKey::KEY_R) {
        state.show_overlay = !state.show_overlay;
    }
    if rl.is_key_pressed(KeyboardKey::KEY_B) {
        state.bg_idx = (state.bg_idx + 1) % BG_COLORS.len();
    }
    if rl.is_key_pressed(KeyboardKey::KEY_ZERO) {
        state.pos = default_pos();
        state.zoom = 3.0;
    }

    if rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT)
        && let Some(tex) = texture
    {
        let mouse = rl.get_mouse_position();
        let in_viewport = mouse.x >= VIEW_X
            && mouse.x < VIEW_X + VIEW_W
            && mouse.y >= VIEW_Y
            && mouse.y < VIEW_Y + VIEW_H;
        let cell = TILE_SIZE as f32 * state.zoom;
        let tex_w = tex.width as f32 * state.zoom;
        let tex_h = tex.height as f32 * state.zoom;
        let in_image = mouse.x >= state.pos.x
            && mouse.x < state.pos.x + tex_w
            && mouse.y >= state.pos.y
            && mouse.y < state.pos.y + tex_h;
        if in_viewport && in_image {
            let tile_x = ((mouse.x - state.pos.x) / cell).floor() as i32;
            let tile_y = ((mouse.y - state.pos.y) / cell).floor() as i32;
            let cols = tex.width / TILE_SIZE;
            if cols > 0 {
                let idx = tile_y * cols + tile_x + 1; // 1-based to match gfx.spr
                let s = idx.to_string();
                let ok = rl.set_clipboard_text(&s).is_ok();
                let msg = if ok {
                    format!("copied sprite {idx} to clipboard")
                } else {
                    format!("sprite {idx} (clipboard unavailable)")
                };
                crate::msg::info!("{msg}");
                return Some(msg);
            }
        }
    }

    None
}

pub(super) fn draw(
    d: &mut RaylibDrawHandle,
    font: &Font,
    state: &State,
    texture: Option<&Texture2D>,
    sprites_path: Option<&Path>,
) {
    // 2× the game-canvas font size for desktop-sized tools text. See
    // jukebox::draw for the rationale.
    const SMALL: f32 = (crate::font::MONOGRAM_SIZE * 2) as f32;

    d.gui_panel(
        Rectangle::new(PANEL_X, PANEL_Y, PANEL_W, PANEL_H),
        "TilePicker",
    );

    if let Some(tex) = texture {
        let tw = tex.width / TILE_SIZE;
        let th = tex.height / TILE_SIZE;
        d.draw_text_ex(
            font,
            &format!(
                "{}x{}px  |  {}x{} tiles ({} total)  |  zoom {:.1}x  overlay: {}",
                tex.width,
                tex.height,
                tw,
                th,
                tw * th,
                state.zoom,
                if state.show_overlay { "on" } else { "off" },
            ),
            Vector2::new(30.0, PANEL_Y + 30.0),
            SMALL,
            0.0,
            color(Pal::DarkBlue),
        );
    } else {
        let msg = match sprites_path {
            Some(p) => format!("no sprites.png at {}", p.display()),
            None => "no project loaded (pass a path: `usagi tools path/to/project`)".to_string(),
        };
        d.draw_text_ex(
            font,
            &msg,
            Vector2::new(30.0, PANEL_Y + 30.0),
            SMALL,
            0.0,
            color(Pal::DarkBlue),
        );
    }

    d.draw_rectangle(
        VIEW_X as i32,
        VIEW_Y as i32,
        VIEW_W as i32,
        VIEW_H as i32,
        BG_COLORS[state.bg_idx],
    );

    if let Some(tex) = texture {
        let mut clip =
            d.begin_scissor_mode(VIEW_X as i32, VIEW_Y as i32, VIEW_W as i32, VIEW_H as i32);
        clip.draw_texture_ex(tex, state.pos, 0., state.zoom, Color::WHITE);
        if state.show_overlay {
            draw_overlay(&mut clip, font, tex, state);
        }
    }

    d.draw_text_ex(
        font,
        "WASD: pan   QE: zoom   R: overlay   B: bg   0: reset   click: copy 1-based index",
        Vector2::new(30.0, HINT_Y),
        SMALL,
        0.0,
        color(Pal::DarkGray),
    );
}

fn draw_overlay<T: RaylibDraw>(d: &mut T, font: &Font, tex: &Texture2D, state: &State) {
    let cols = tex.width / TILE_SIZE;
    let rows = tex.height / TILE_SIZE;
    if cols <= 0 || rows <= 0 {
        return;
    }
    let cell = TILE_SIZE as f32 * state.zoom;
    // Semi-transparent cyan. Readable on any bg without a per-bg palette.
    let overlay = Color::new(0, 180, 200, 220);

    // 2× the design size — same crisp integer scale as the rest of
    // the tools panel text, and large enough to read at the default
    // 3× zoom (48 px tiles). monogram is bitmap with POINT filter so
    // any integer multiple stays crisp.
    let size = (crate::font::MONOGRAM_SIZE * 2) as f32;
    for row in 0..rows {
        for col in 0..cols {
            let idx = row * cols + col + 1;
            let x = state.pos.x + col as f32 * cell + 2.0;
            let y = state.pos.y + row as f32 * cell + 2.0;
            d.draw_text_ex(
                font,
                &idx.to_string(),
                Vector2::new(x, y),
                size,
                0.0,
                overlay,
            );
        }
    }

    let thick = (2.0 * state.zoom / 4.0).max(1.0);
    let w = tex.width as f32 * state.zoom;
    let h = tex.height as f32 * state.zoom;
    for r in 0..=rows {
        let y = state.pos.y + r as f32 * cell;
        d.draw_line_ex(
            Vector2::new(state.pos.x, y),
            Vector2::new(state.pos.x + w, y),
            thick,
            overlay,
        );
    }
    for c in 0..=cols {
        let x = state.pos.x + c as f32 * cell;
        d.draw_line_ex(
            Vector2::new(x, state.pos.y),
            Vector2::new(x, state.pos.y + h),
            thick,
            overlay,
        );
    }
}
