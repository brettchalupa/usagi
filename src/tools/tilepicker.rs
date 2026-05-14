//! TilePicker tool: visualises `<project>/sprites.png` with a 1-based
//! index overlay. LMB picks a single tile and copies its `spr` index;
//! RMB drag selects a tile-aligned rect and copies `sx,sy,sw,sh` ready
//! for `sspr`. The current selection is shown persistently in the
//! header and highlighted on the sheet.

use super::{HINT_Y, PANEL_H, PANEL_W, PANEL_X, PANEL_Y};
use crate::tools::theme;
use sola_raylib::prelude::*;
use std::path::Path;

const PAN_SPEED: f32 = 400.0; // pixels/second, dt-scaled
const ZOOM_STEP: f32 = 0.5;
const ZOOM_MIN: f32 = 0.5;
const ZOOM_MAX: f32 = 20.;
const BG_COLORS: [Color; 3] = [Color::GRAY, Color::BLACK, Color::WHITE];

/// Viewport rect. The image + overlay are clipped to this so panning
/// doesn't bleed onto the surrounding UI. VIEW_Y sits below two header
/// text lines (meta + cursor readout) at the top of the panel.
const VIEW_X: f32 = PANEL_X + 2.0;
const VIEW_Y: f32 = PANEL_Y + 90.0;
const VIEW_W: f32 = PANEL_W - 4.0;
const VIEW_H: f32 = HINT_Y - VIEW_Y - 8.0;

/// A finalized pick on the sprite sheet. `spr_idx` is `Some` only when
/// the selection covers exactly one sprite_size×sprite_size cell. Multi-tile
/// drag rects leave it `None` because no single `spr` index maps to them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Selection {
    pub sx: i32,
    pub sy: i32,
    pub sw: i32,
    pub sh: i32,
    pub spr_idx: Option<i32>,
}

/// Initial zoom factor. Picked so a typical 128x128 sprite sheet reads
/// at a comfortable size in the viewport on first open; user pans/zooms
/// from there. The `0` key reset returns to this zoom and re-centers.
const DEFAULT_ZOOM: f32 = 4.0;

pub(super) struct State {
    pub zoom: f32,
    pub pos: Vector2,
    pub show_overlay: bool,
    pub bg_idx: usize,
    /// Cell size, in pixels, of the loaded `sprites.png`. Read from
    /// the project's `_config().sprite_size` at tools startup
    /// (defaulting to 16). Drives the grid overlay and the
    /// 1-based-index click-to-copy.
    pub sprite_size: i32,
    pub selection: Option<Selection>,
    /// (col, row) of the tile where an RMB drag started. `Some` while
    /// the right button is held after pressing inside the image.
    pub drag_start_cell: Option<(i32, i32)>,
    /// True while a MMB pan is in progress (press started inside the
    /// viewport, button still held). Pressing MMB on panel chrome or
    /// outside the panel never starts a pan.
    pub mmb_panning: bool,
    /// True while a space+LMB pan is in progress. Latched on the LMB
    /// press frame when space is held; cleared when either is released.
    /// While set, LMB selection is suppressed so the same drag doesn't
    /// also pick a tile.
    pub space_panning: bool,
    /// Flipped to true after the first frame the tilepicker sees a
    /// loaded texture and centers the camera on it. `State::new` runs
    /// before the sprite sheet is loaded, so the centering has to
    /// happen lazily once dims are known. Reset (`0` key) clears this
    /// flag so the next frame re-centers.
    pub auto_centered: bool,
}

impl State {
    pub fn new(sprite_size: i32) -> Self {
        Self {
            zoom: DEFAULT_ZOOM,
            pos: default_pos(),
            show_overlay: true,
            bg_idx: 0,
            sprite_size,
            selection: None,
            drag_start_cell: None,
            mmb_panning: false,
            space_panning: false,
            auto_centered: false,
        }
    }
}

/// Fallback camera position used until the first texture frame lands
/// (and after, only if there's no sprites.png to center on).
fn default_pos() -> Vector2 {
    Vector2::new(VIEW_X + 40.0, VIEW_Y + 40.0)
}

/// Centers a `tex_w × tex_h` sprite sheet at the given zoom inside the
/// viewport. Result is the camera position (top-left of the rendered
/// image) such that the image's center sits at the viewport's center.
fn centered_pos(tex_w: i32, tex_h: i32, zoom: f32) -> Vector2 {
    let img_w = tex_w as f32 * zoom;
    let img_h = tex_h as f32 * zoom;
    Vector2::new(
        VIEW_X + (VIEW_W - img_w) * 0.5,
        VIEW_Y + (VIEW_H - img_h) * 0.5,
    )
}

/// Returns an optional toast message (e.g. "copied spr 7 to clipboard")
/// for the wrapper to display. The persistent selection readout in the
/// header is the durable channel; the toast is a transient
/// confirmation that something hit the clipboard.
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
        state.auto_centered = false;
    }

    let tex = texture?;
    if state.sprite_size <= 0 {
        return None;
    }
    // Lazy center: State::new ran before the sheet existed, so the
    // first frame with a texture (and any frame after a `0` reset)
    // snaps to a centered camera at the default zoom.
    if !state.auto_centered {
        state.zoom = DEFAULT_ZOOM;
        state.pos = centered_pos(tex.width, tex.height, state.zoom);
        state.auto_centered = true;
    }
    let mouse = rl.get_mouse_position();
    let in_viewport = mouse.x >= VIEW_X
        && mouse.x < VIEW_X + VIEW_W
        && mouse.y >= VIEW_Y
        && mouse.y < VIEW_Y + VIEW_H;

    let space_held = rl.is_key_down(KeyboardKey::KEY_SPACE);

    // MMB drag-pan. Latch on press-in-viewport so a press starting
    // outside the panel can't grab the image when the mouse wanders in.
    if rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_MIDDLE) && in_viewport {
        state.mmb_panning = true;
    }
    if state.mmb_panning {
        if rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_MIDDLE) {
            let delta = rl.get_mouse_delta();
            state.pos.x += delta.x;
            state.pos.y += delta.y;
        } else {
            state.mmb_panning = false;
        }
    }

    // Space + LMB drag-pan. Standard image-editor gesture: hold space,
    // drag with the left mouse to push the sheet around. Latched at
    // press time so the same drag can't double as a tile selection
    // (the LMB selection block below checks `space_panning`).
    if rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT) && space_held && in_viewport {
        state.space_panning = true;
    }
    if state.space_panning {
        if space_held && rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT) {
            let delta = rl.get_mouse_delta();
            state.pos.x += delta.x;
            state.pos.y += delta.y;
        } else {
            state.space_panning = false;
        }
    }

    // Wheel zoom, anchored on the cursor: the sheet pixel under the
    // mouse stays put as zoom changes, so users can dial in on the
    // tile they're looking at without re-panning afterward.
    let wheel = rl.get_mouse_wheel_move();
    if wheel != 0.0 && in_viewport {
        let new_zoom = (state.zoom + wheel * ZOOM_STEP).clamp(ZOOM_MIN, ZOOM_MAX);
        if new_zoom != state.zoom {
            (state.pos, state.zoom) = zoom_to_cursor(mouse, state.pos, state.zoom, new_zoom);
        }
    }

    if rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT) && !space_held && in_viewport {
        match mouse_to_cell(
            mouse,
            state.pos,
            state.zoom,
            state.sprite_size,
            tex.width,
            tex.height,
        ) {
            Some((col, row)) => {
                let cols = tex.width / state.sprite_size;
                if cols <= 0 {
                    return None;
                }
                let idx = row * cols + col + 1;
                let sz = state.sprite_size;
                state.selection = Some(Selection {
                    sx: col * sz,
                    sy: row * sz,
                    sw: sz,
                    sh: sz,
                    spr_idx: Some(idx),
                });
                let s = idx.to_string();
                let ok = rl.set_clipboard_text(&s).is_ok();
                let msg = if ok {
                    format!("copied spr {idx} to clipboard")
                } else {
                    format!("spr {idx} (clipboard unavailable)")
                };
                crate::msg::info!("{msg}");
                return Some(msg);
            }
            None => {
                // Click landed inside the viewport but off the sheet.
                // Treat it as "clear the active selection" so the
                // yellow box doesn't linger over a tile the user
                // already noted down.
                if state.selection.is_some() {
                    state.selection = None;
                }
            }
        }
    }

    // RMB press starts a drag; the rect isn't finalized until release.
    // Press must land inside the image so a stray right-click on the
    // panel chrome doesn't start a phantom drag.
    if rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_RIGHT)
        && in_viewport
        && let Some(cell) = mouse_to_cell(
            mouse,
            state.pos,
            state.zoom,
            state.sprite_size,
            tex.width,
            tex.height,
        )
    {
        state.drag_start_cell = Some(cell);
    }

    // RMB release finalizes the rect, regardless of where the mouse
    // ended up. The end cell is clamped so a drag that wanders past
    // the sheet still produces a valid selection.
    if rl.is_mouse_button_released(MouseButton::MOUSE_BUTTON_RIGHT)
        && let Some(start) = state.drag_start_cell.take()
    {
        let end = mouse_to_cell_clamped(
            mouse,
            state.pos,
            state.zoom,
            state.sprite_size,
            tex.width,
            tex.height,
        );
        let (col, row, cw, ch) = normalize_cells(start, end);
        let sz = state.sprite_size;
        let sx = col * sz;
        let sy = row * sz;
        let sw = cw * sz;
        let sh = ch * sz;
        let cols = tex.width / sz;
        let spr_idx = if cw == 1 && ch == 1 && cols > 0 {
            Some(row * cols + col + 1)
        } else {
            None
        };
        state.selection = Some(Selection {
            sx,
            sy,
            sw,
            sh,
            spr_idx,
        });
        let snippet = format!("{sx},{sy},{sw},{sh}");
        let ok = rl.set_clipboard_text(&snippet).is_ok();
        let msg = if ok {
            format!("copied sspr {snippet} to clipboard")
        } else {
            format!("sspr {snippet} (clipboard unavailable)")
        };
        crate::msg::info!("{msg}");
        return Some(msg);
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
        let tw = tex.width / state.sprite_size;
        let th = tex.height / state.sprite_size;
        d.draw_text_ex(
            font,
            &format!(
                "{}x{}px  |  {}x{} tiles ({} total)  |  zoom {:.1}x  overlay: {}  |  {}",
                tex.width,
                tex.height,
                tw,
                th,
                tw * th,
                state.zoom,
                if state.show_overlay { "on" } else { "off" },
                selection_readout(state.selection),
            ),
            Vector2::new(30.0, PANEL_Y + 30.0),
            SMALL,
            0.0,
            theme::TEXT,
        );
        let mouse = d.get_mouse_position();
        let in_viewport = mouse.x >= VIEW_X
            && mouse.x < VIEW_X + VIEW_W
            && mouse.y >= VIEW_Y
            && mouse.y < VIEW_Y + VIEW_H;
        d.draw_text_ex(
            font,
            &cursor_readout(
                mouse,
                state.pos,
                state.zoom,
                tex.width,
                tex.height,
                in_viewport,
            ),
            Vector2::new(30.0, PANEL_Y + 56.0),
            SMALL,
            0.0,
            theme::TEXT,
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
            theme::TEXT,
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
        let mouse = d.get_mouse_position();
        let mut clip =
            d.begin_scissor_mode(VIEW_X as i32, VIEW_Y as i32, VIEW_W as i32, VIEW_H as i32);
        clip.draw_texture_ex(tex, state.pos, 0., state.zoom, Color::WHITE);
        if state.show_overlay {
            draw_overlay(&mut clip, font, tex, state);
        }
        // Finalized selection sits behind the in-progress drag preview
        // so both are visible when the user is mid-drag over an old
        // selection.
        if let Some(sel) = state.selection {
            draw_selection_box(
                &mut clip,
                state,
                sel.sx,
                sel.sy,
                sel.sw,
                sel.sh,
                theme::SELECTION,
            );
        }
        if let Some(start) = state.drag_start_cell
            && state.sprite_size > 0
        {
            let end = mouse_to_cell_clamped(
                mouse,
                state.pos,
                state.zoom,
                state.sprite_size,
                tex.width,
                tex.height,
            );
            let (col, row, cw, ch) = normalize_cells(start, end);
            let sz = state.sprite_size;
            draw_selection_box(
                &mut clip,
                state,
                col * sz,
                row * sz,
                cw * sz,
                ch * sz,
                theme::ACCENT,
            );
        }
    }

    d.draw_text_ex(
        font,
        "WASD/MMB/space+drag: pan  QE/wheel: zoom  R: overlay  B: bg  0: reset  LMB: spr  RMB drag: sspr",
        Vector2::new(30.0, HINT_Y),
        SMALL,
        0.0,
        theme::TEXT_MUTED,
    );
}

/// Format the current selection for the header readout. Single-tile
/// selections show both the `spr` index and the `sspr` source rect so
/// the user can grab either; multi-tile rects show only the source rect.
fn selection_readout(sel: Option<Selection>) -> String {
    match sel {
        None => "selection: (LMB tile, RMB drag rect)".to_owned(),
        Some(Selection {
            sx,
            sy,
            sw,
            sh,
            spr_idx: Some(idx),
        }) => format!("selected: spr {idx} = sspr {sx},{sy},{sw},{sh}"),
        Some(Selection {
            sx,
            sy,
            sw,
            sh,
            spr_idx: None,
        }) => format!("selected: sspr {sx},{sy},{sw},{sh}"),
    }
}

/// Draw a rectangular outline on the sprite sheet at sheet-local pixel
/// coords (sx, sy, sw, sh). Maps through zoom + pos so it tracks the
/// image as the user pans / zooms.
fn draw_selection_box<T: RaylibDraw>(
    d: &mut T,
    state: &State,
    sx: i32,
    sy: i32,
    sw: i32,
    sh: i32,
    color: Color,
) {
    let x = state.pos.x + sx as f32 * state.zoom;
    let y = state.pos.y + sy as f32 * state.zoom;
    let w = sw as f32 * state.zoom;
    let h = sh as f32 * state.zoom;
    let thick = (state.zoom * 0.75).max(2.0);
    d.draw_rectangle_lines_ex(Rectangle::new(x, y, w, h), thick, color);
}

fn draw_overlay<T: RaylibDraw>(d: &mut T, font: &Font, tex: &Texture2D, state: &State) {
    let cols = tex.width / state.sprite_size;
    let rows = tex.height / state.sprite_size;
    if cols <= 0 || rows <= 0 {
        return;
    }
    let cell = state.sprite_size as f32 * state.zoom;
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

/// Mouse position → (col, row) on the sprite sheet, or `None` when the
/// mouse is outside the image. Splitting this out from `handle_input`
/// keeps the rect math testable without raylib state.
fn mouse_to_cell(
    mouse: Vector2,
    pos: Vector2,
    zoom: f32,
    sprite_size: i32,
    sheet_w: i32,
    sheet_h: i32,
) -> Option<(i32, i32)> {
    if sprite_size <= 0 || zoom <= 0.0 {
        return None;
    }
    let cell = sprite_size as f32 * zoom;
    let col = ((mouse.x - pos.x) / cell).floor() as i32;
    let row = ((mouse.y - pos.y) / cell).floor() as i32;
    let cols = sheet_w / sprite_size;
    let rows = sheet_h / sprite_size;
    if col < 0 || row < 0 || col >= cols || row >= rows {
        return None;
    }
    Some((col, row))
}

/// Like `mouse_to_cell` but clamps to the sheet's last valid cell
/// instead of returning `None`. Used for the end cell of an
/// in-progress drag so the user can release outside the image.
fn mouse_to_cell_clamped(
    mouse: Vector2,
    pos: Vector2,
    zoom: f32,
    sprite_size: i32,
    sheet_w: i32,
    sheet_h: i32,
) -> (i32, i32) {
    let cell = (sprite_size as f32 * zoom).max(f32::EPSILON);
    let col = ((mouse.x - pos.x) / cell).floor() as i32;
    let row = ((mouse.y - pos.y) / cell).floor() as i32;
    let cols = (sheet_w / sprite_size).max(1);
    let rows = (sheet_h / sprite_size).max(1);
    (col.clamp(0, cols - 1), row.clamp(0, rows - 1))
}

/// Two cell coords (start, end) → top-left + size in cells, normalized
/// so width/height are always positive and the rect is inclusive of
/// both endpoints (a click-and-release on the same cell yields a 1×1
/// rect, not 0×0).
fn normalize_cells(a: (i32, i32), b: (i32, i32)) -> (i32, i32, i32, i32) {
    let col_min = a.0.min(b.0);
    let col_max = a.0.max(b.0);
    let row_min = a.1.min(b.1);
    let row_max = a.1.max(b.1);
    (
        col_min,
        row_min,
        col_max - col_min + 1,
        row_max - row_min + 1,
    )
}

/// Mouse position → sheet pixel (x, y), or `None` when the mouse is
/// outside the image. Independent of `sprite_size` since the readout is
/// for arbitrary pixel coords, not tile coords.
fn mouse_to_pixel(
    mouse: Vector2,
    pos: Vector2,
    zoom: f32,
    sheet_w: i32,
    sheet_h: i32,
) -> Option<(i32, i32)> {
    if zoom <= 0.0 {
        return None;
    }
    let x = ((mouse.x - pos.x) / zoom).floor() as i32;
    let y = ((mouse.y - pos.y) / zoom).floor() as i32;
    if x < 0 || y < 0 || x >= sheet_w || y >= sheet_h {
        return None;
    }
    Some((x, y))
}

/// Header text for the cursor's current position. Three states: outside
/// the panel viewport entirely, inside the panel but off the image, or
/// over a real sheet pixel.
fn cursor_readout(
    mouse: Vector2,
    pos: Vector2,
    zoom: f32,
    sheet_w: i32,
    sheet_h: i32,
    in_viewport: bool,
) -> String {
    if !in_viewport {
        return "cursor: -".to_owned();
    }
    match mouse_to_pixel(mouse, pos, zoom, sheet_w, sheet_h) {
        Some((x, y)) => format!("cursor: ({x}, {y})"),
        None => "cursor: (off sheet)".to_owned(),
    }
}

/// Compute the new (pos, zoom) for a zoom transition that keeps the
/// sheet pixel currently under the cursor stationary on screen. Pulled
/// out so the anchor math is unit-testable without raylib state.
fn zoom_to_cursor(mouse: Vector2, pos: Vector2, old_zoom: f32, new_zoom: f32) -> (Vector2, f32) {
    let wx = (mouse.x - pos.x) / old_zoom;
    let wy = (mouse.y - pos.y) / old_zoom;
    let new_pos = Vector2::new(mouse.x - wx * new_zoom, mouse.y - wy * new_zoom);
    (new_pos, new_zoom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_to_cell_inside_image_at_default_zoom() {
        let pos = Vector2::new(100.0, 50.0);
        // 16-px cells at zoom=2 → 32 screen-px per cell. Mouse one
        // pixel inside cell (1, 0): pos.x + 32 + 1, pos.y + 1.
        let cell = mouse_to_cell(
            Vector2::new(pos.x + 33.0, pos.y + 1.0),
            pos,
            2.0,
            16,
            128,
            64,
        );
        assert_eq!(cell, Some((1, 0)));
    }

    #[test]
    fn mouse_to_cell_returns_none_outside_image() {
        let pos = Vector2::new(100.0, 50.0);
        assert!(mouse_to_cell(Vector2::new(50.0, 50.0), pos, 1.0, 16, 128, 64).is_none());
        // Just past the right edge of an 8-cell-wide sheet (128 / 16 = 8
        // cols at 16 px each = 128 px wide at zoom 1).
        assert!(
            mouse_to_cell(
                Vector2::new(pos.x + 129.0, pos.y + 5.0),
                pos,
                1.0,
                16,
                128,
                64
            )
            .is_none()
        );
    }

    #[test]
    fn mouse_to_cell_respects_custom_sprite_size() {
        let pos = Vector2::zero();
        // 8-px cells at zoom 1: pixel (15, 0) is column 1.
        let cell = mouse_to_cell(Vector2::new(15.0, 0.0), pos, 1.0, 8, 64, 32);
        assert_eq!(cell, Some((1, 0)));
        // 32-px cells at zoom 1: same pixel is column 0.
        let cell = mouse_to_cell(Vector2::new(15.0, 0.0), pos, 1.0, 32, 128, 64);
        assert_eq!(cell, Some((0, 0)));
    }

    #[test]
    fn mouse_to_cell_clamped_pulls_back_when_outside() {
        let pos = Vector2::zero();
        // 16-px cells × zoom 2 = 32 screen-px. 4×2 cell sheet (64×32).
        let clamped = mouse_to_cell_clamped(Vector2::new(-50.0, -50.0), pos, 2.0, 16, 64, 32);
        assert_eq!(clamped, (0, 0));
        let clamped = mouse_to_cell_clamped(Vector2::new(9999.0, 9999.0), pos, 2.0, 16, 64, 32);
        assert_eq!(clamped, (3, 1));
    }

    #[test]
    fn normalize_cells_handles_any_drag_direction() {
        // top-left → bottom-right.
        assert_eq!(normalize_cells((1, 1), (3, 4)), (1, 1, 3, 4));
        // bottom-right → top-left: same rect.
        assert_eq!(normalize_cells((3, 4), (1, 1)), (1, 1, 3, 4));
        // top-right → bottom-left.
        assert_eq!(normalize_cells((5, 0), (2, 2)), (2, 0, 4, 3));
        // bottom-left → top-right.
        assert_eq!(normalize_cells((0, 5), (4, 1)), (0, 1, 5, 5));
    }

    #[test]
    fn normalize_cells_single_cell_is_one_by_one() {
        // A click without dragging produces a 1×1 rect, not 0×0.
        assert_eq!(normalize_cells((2, 3), (2, 3)), (2, 3, 1, 1));
    }

    #[test]
    fn selection_readout_no_selection_hints_at_controls() {
        let s = selection_readout(None);
        assert!(s.contains("LMB"), "got: {s}");
        assert!(s.contains("RMB"), "got: {s}");
    }

    #[test]
    fn selection_readout_single_tile_shows_spr_and_sspr() {
        let s = selection_readout(Some(Selection {
            sx: 16,
            sy: 0,
            sw: 16,
            sh: 16,
            spr_idx: Some(2),
        }));
        assert!(s.contains("spr 2"), "got: {s}");
        assert!(s.contains("sspr 16,0,16,16"), "got: {s}");
    }

    #[test]
    fn selection_readout_multi_tile_shows_only_sspr() {
        let s = selection_readout(Some(Selection {
            sx: 0,
            sy: 16,
            sw: 48,
            sh: 32,
            spr_idx: None,
        }));
        assert!(s.contains("sspr 0,16,48,32"), "got: {s}");
        // The "spr N = sspr ..." form is for single-tile picks; the
        // multi-tile readout must omit both the spr index and the
        // equals sign.
        assert!(!s.contains(" = "), "should not show a spr index: {s}");
        assert!(s.starts_with("selected: sspr"), "got: {s}");
    }

    #[test]
    fn mouse_to_pixel_inside_image() {
        let pos = Vector2::new(100.0, 50.0);
        // At zoom 4 each sheet pixel is 4 screen px. Mouse at (pos.x+10,
        // pos.y+6) → floor(10/4), floor(6/4) → (2, 1).
        let px = mouse_to_pixel(Vector2::new(110.0, 56.0), pos, 4.0, 32, 32);
        assert_eq!(px, Some((2, 1)));
    }

    #[test]
    fn mouse_to_pixel_outside_image_returns_none() {
        let pos = Vector2::new(0.0, 0.0);
        assert!(mouse_to_pixel(Vector2::new(-1.0, 0.0), pos, 1.0, 32, 32).is_none());
        assert!(mouse_to_pixel(Vector2::new(32.0, 0.0), pos, 1.0, 32, 32).is_none());
        assert!(mouse_to_pixel(Vector2::new(0.0, 32.0), pos, 1.0, 32, 32).is_none());
    }

    #[test]
    fn cursor_readout_outside_viewport_shows_dash() {
        let s = cursor_readout(Vector2::zero(), Vector2::zero(), 1.0, 32, 32, false);
        assert_eq!(s, "cursor: -");
    }

    #[test]
    fn cursor_readout_off_sheet_says_so() {
        let pos = Vector2::new(100.0, 100.0);
        // In viewport but the cursor isn't on the image.
        let s = cursor_readout(Vector2::new(0.0, 0.0), pos, 1.0, 32, 32, true);
        assert!(s.contains("off sheet"), "got: {s}");
    }

    #[test]
    fn cursor_readout_on_image_shows_pixel_coords() {
        let s = cursor_readout(Vector2::new(48.0, 24.0), Vector2::zero(), 2.0, 64, 64, true);
        // 48/2=24, 24/2=12.
        assert_eq!(s, "cursor: (24, 12)");
    }

    #[test]
    fn zoom_to_cursor_keeps_pixel_under_cursor_anchored() {
        // Sheet pixel (10, 5) is currently under the cursor at screen
        // (110, 55) with pos=(100, 50), zoom=1. After a 1→2 zoom the
        // same sheet pixel should still sit under (110, 55).
        let mouse = Vector2::new(110.0, 55.0);
        let (new_pos, new_zoom) = zoom_to_cursor(mouse, Vector2::new(100.0, 50.0), 1.0, 2.0);
        assert_eq!(new_zoom, 2.0);
        // (mouse - new_pos) / new_zoom should equal the original sheet pixel (10, 5).
        let wx = (mouse.x - new_pos.x) / new_zoom;
        let wy = (mouse.y - new_pos.y) / new_zoom;
        assert!((wx - 10.0).abs() < 1e-4, "wx={wx}");
        assert!((wy - 5.0).abs() < 1e-4, "wy={wy}");
    }

    #[test]
    fn zoom_to_cursor_works_zooming_out() {
        // Symmetric: zooming out (2→1) should also keep the cursor anchored.
        let mouse = Vector2::new(200.0, 120.0);
        let pos = Vector2::new(40.0, 30.0);
        let (new_pos, _) = zoom_to_cursor(mouse, pos, 2.0, 1.0);
        let wx_old = (mouse.x - pos.x) / 2.0;
        let wx_new = (mouse.x - new_pos.x) / 1.0;
        assert!((wx_old - wx_new).abs() < 1e-4, "old={wx_old} new={wx_new}");
    }
}
