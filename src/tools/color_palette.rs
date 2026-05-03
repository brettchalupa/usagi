//! ColorPalette tool: lays out the engine's 16-color palette as
//! clickable swatches paired with their `gfx.COLOR_*` constant names.
//! Clicking a swatch copies the prefixed constant (e.g. `gfx.COLOR_RED`)
//! to the clipboard so it can be pasted directly into game code.

use super::{HINT_Y, PANEL_H, PANEL_W, PANEL_X, PANEL_Y};
use crate::palette::{Pal, color};
use sola_raylib::prelude::*;

/// (palette entry, Lua constant name without the `gfx.` prefix) in
/// palette-index order. Names are kept in sync with `api.rs` where
/// `gfx.COLOR_*` is registered for Lua.
const ENTRIES: [(Pal, &str); 16] = [
    (Pal::Black, "COLOR_BLACK"),
    (Pal::DarkBlue, "COLOR_DARK_BLUE"),
    (Pal::DarkPurple, "COLOR_DARK_PURPLE"),
    (Pal::DarkGreen, "COLOR_DARK_GREEN"),
    (Pal::Brown, "COLOR_BROWN"),
    (Pal::DarkGray, "COLOR_DARK_GRAY"),
    (Pal::LightGray, "COLOR_LIGHT_GRAY"),
    (Pal::White, "COLOR_WHITE"),
    (Pal::Red, "COLOR_RED"),
    (Pal::Orange, "COLOR_ORANGE"),
    (Pal::Yellow, "COLOR_YELLOW"),
    (Pal::Green, "COLOR_GREEN"),
    (Pal::Blue, "COLOR_BLUE"),
    (Pal::Indigo, "COLOR_INDIGO"),
    (Pal::Pink, "COLOR_PINK"),
    (Pal::Peach, "COLOR_PEACH"),
];

const COLS: usize = 4;
const ROWS: usize = 4;

const GRID_TOP: f32 = PANEL_Y + 60.0;
const GRID_BOTTOM: f32 = HINT_Y - 16.0;
const GRID_PAD: f32 = 16.0;
const GRID_LEFT: f32 = PANEL_X + GRID_PAD;
const GRID_RIGHT: f32 = PANEL_X + PANEL_W - GRID_PAD;

const CELL_PAD: f32 = 8.0;
const LABEL_H: f32 = (crate::font::MONOGRAM_SIZE * 2) as f32 + 6.0;

pub(super) struct State {}

impl State {
    pub fn new() -> Self {
        Self {}
    }
}

fn cell_rect(idx: usize) -> Rectangle {
    let row = (idx / COLS) as f32;
    let col = (idx % COLS) as f32;
    let cell_w = (GRID_RIGHT - GRID_LEFT) / COLS as f32;
    let cell_h = (GRID_BOTTOM - GRID_TOP) / ROWS as f32;
    Rectangle::new(
        GRID_LEFT + col * cell_w,
        GRID_TOP + row * cell_h,
        cell_w,
        cell_h,
    )
}

fn swatch_rect(cell: Rectangle) -> Rectangle {
    Rectangle::new(
        cell.x + CELL_PAD,
        cell.y + CELL_PAD,
        cell.width - 2.0 * CELL_PAD,
        cell.height - 2.0 * CELL_PAD - LABEL_H,
    )
}

fn rect_contains(r: Rectangle, p: Vector2) -> bool {
    p.x >= r.x && p.x < r.x + r.width && p.y >= r.y && p.y < r.y + r.height
}

pub(super) fn handle_input(rl: &mut RaylibHandle, _state: &mut State) -> Option<String> {
    if !rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT) {
        return None;
    }
    let mouse = rl.get_mouse_position();
    for (i, (_, name)) in ENTRIES.iter().enumerate() {
        if rect_contains(cell_rect(i), mouse) {
            let snippet = format!("gfx.{name}");
            let ok = rl.set_clipboard_text(&snippet).is_ok();
            let msg = if ok {
                format!("copied {snippet} to clipboard")
            } else {
                format!("{snippet} (clipboard unavailable)")
            };
            println!("[usagi] {msg}");
            return Some(msg);
        }
    }
    None
}

pub(super) fn draw(d: &mut RaylibDrawHandle, font: &Font, _state: &State) {
    const SMALL: f32 = (crate::font::MONOGRAM_SIZE * 2) as f32;

    d.gui_panel(
        Rectangle::new(PANEL_X, PANEL_Y, PANEL_W, PANEL_H),
        "ColorPalette",
    );

    d.draw_text_ex(
        font,
        "16 engine colors and their gfx.COLOR_* names. Click a swatch to copy.",
        Vector2::new(PANEL_X + 10.0, PANEL_Y + 30.0),
        SMALL,
        0.0,
        color(Pal::DarkBlue),
    );

    let mouse = d.get_mouse_position();

    for (i, (pal, name)) in ENTRIES.iter().enumerate() {
        let cell = cell_rect(i);
        let swatch = swatch_rect(cell);
        let hovered = rect_contains(cell, mouse);

        d.draw_rectangle_rec(swatch, color(*pal));
        // Highlight on hover: pink frame matches the FOCUSED button
        // accent already used elsewhere in the tools UI.
        let (border_color, border_thickness) = if hovered {
            (color(Pal::Pink), 3.0)
        } else {
            (color(Pal::DarkBlue), 1.0)
        };
        d.draw_rectangle_lines_ex(swatch, border_thickness, border_color);

        let label = format!("{i}  {name}");
        d.draw_text_ex(
            font,
            &label,
            Vector2::new(cell.x + CELL_PAD, cell.y + cell.height - LABEL_H + 2.0),
            SMALL,
            0.0,
            color(Pal::DarkBlue),
        );
    }

    d.draw_text_ex(
        font,
        "click: copy gfx.COLOR_* to clipboard",
        Vector2::new(PANEL_X + 10.0, HINT_Y),
        SMALL,
        0.0,
        color(Pal::DarkGray),
    );
}
