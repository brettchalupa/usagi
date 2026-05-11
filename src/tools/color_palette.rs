//! ColorPalette tool: swatches for every slot in the active project
//! palette (`palette.png` if present, otherwise the engine's Pico-8
//! default). Click a swatch to copy its `gfx.COLOR_*` constant (when
//! showing defaults) or its bare integer slot index (when showing a
//! custom palette, since the COLOR_* names don't carry semantic
//! weight against arbitrary palettes).
//!
//! The tool keeps the engine's default Pico-8 colors for its own UI
//! (panel borders, hover frame, label text) regardless of what
//! palette.png contains, so the tool chrome stays consistent across
//! projects. Only the swatches reflect the user's palette.

use super::{HINT_Y, PANEL_H, PANEL_W, PANEL_X, PANEL_Y};
use crate::palette::{Pal, Palette, color};
use crate::vfs::VirtualFs;
use sola_raylib::prelude::*;

/// Per-slot `gfx.COLOR_*` name for the 16 named palette indices. Slots
/// beyond 16 (when the user's palette is larger than 16) are labelled
/// by their bare integer index. `COLOR_NAMES[0]` corresponds to slot 1.
const COLOR_NAMES: [&str; 16] = [
    "COLOR_BLACK",
    "COLOR_DARK_BLUE",
    "COLOR_DARK_PURPLE",
    "COLOR_DARK_GREEN",
    "COLOR_BROWN",
    "COLOR_DARK_GRAY",
    "COLOR_LIGHT_GRAY",
    "COLOR_WHITE",
    "COLOR_RED",
    "COLOR_ORANGE",
    "COLOR_YELLOW",
    "COLOR_GREEN",
    "COLOR_BLUE",
    "COLOR_INDIGO",
    "COLOR_PINK",
    "COLOR_PEACH",
];

const COLS: usize = 4;

const GRID_TOP: f32 = PANEL_Y + 60.0;
const GRID_BOTTOM: f32 = HINT_Y - 16.0;
const GRID_PAD: f32 = 16.0;
const GRID_LEFT: f32 = PANEL_X + GRID_PAD;
const GRID_RIGHT: f32 = PANEL_X + PANEL_W - GRID_PAD;

const CELL_PAD: f32 = 8.0;
const LABEL_H: f32 = (crate::font::MONOGRAM_SIZE * 2) as f32 + 6.0;

pub(super) struct State {
    /// Palette being displayed. Either the project's `palette.png`
    /// (when present) or the engine default (Pico-8). Note this is
    /// distinct from the engine's globally-active palette, which the
    /// tools window deliberately leaves at the default for its own UI.
    palette: Palette,
    /// True when the project supplied a `palette.png` — toggles the
    /// label style (bare slot numbers vs. `N  COLOR_*` names).
    is_custom: bool,
}

impl State {
    pub fn new(vfs: Option<&dyn VirtualFs>) -> Self {
        let (palette, is_custom) = vfs
            .and_then(load_custom)
            .unwrap_or((Palette::pico8(), false));
        Self { palette, is_custom }
    }

    pub fn reload(&mut self, vfs: Option<&dyn VirtualFs>) {
        let (palette, is_custom) = vfs
            .and_then(load_custom)
            .unwrap_or((Palette::pico8(), false));
        self.palette = palette;
        self.is_custom = is_custom;
    }
}

/// Reads `palette.png` from the project vfs. Returns `Some((palette, true))`
/// on success, `None` when the file is absent. A malformed PNG falls
/// back to the default with a warning log so the tool window doesn't
/// refuse to open.
fn load_custom(vfs: &dyn VirtualFs) -> Option<(Palette, bool)> {
    let bytes = vfs.read_palette()?;
    match Palette::from_image_bytes(&bytes) {
        Ok(p) => Some((p, true)),
        Err(e) => {
            crate::msg::warn!("palette.png: {e}; using default palette in ColorPalette tool");
            None
        }
    }
}

fn rows_for(count: usize) -> usize {
    if count == 0 { 0 } else { count.div_ceil(COLS) }
}

fn cell_rect(grid_idx: usize, rows: usize) -> Rectangle {
    let row = (grid_idx / COLS) as f32;
    let col = (grid_idx % COLS) as f32;
    let cell_w = (GRID_RIGHT - GRID_LEFT) / COLS as f32;
    let cell_h = (GRID_BOTTOM - GRID_TOP) / rows.max(1) as f32;
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

/// Lua-side snippet to copy when slot `slot` (1-based) is clicked.
/// When the project ships a custom palette, only the bare integer is
/// meaningful (the `gfx.COLOR_*` names map to Pico-8 ordering, not the
/// user's palette). With the default palette the named constant reads
/// cleaner.
fn snippet_for(slot: usize, is_custom: bool) -> String {
    if is_custom {
        slot.to_string()
    } else if let Some(name) = COLOR_NAMES.get(slot - 1) {
        format!("gfx.{name}")
    } else {
        slot.to_string()
    }
}

/// Human-readable label drawn below each swatch.
fn label_for(slot: usize, is_custom: bool) -> String {
    if is_custom {
        slot.to_string()
    } else if let Some(name) = COLOR_NAMES.get(slot - 1) {
        format!("{slot}  {name}")
    } else {
        slot.to_string()
    }
}

pub(super) fn handle_input(rl: &mut RaylibHandle, state: &mut State) -> Option<String> {
    if !rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT) {
        return None;
    }
    let count = state.palette.len();
    let rows = rows_for(count);
    let mouse = rl.get_mouse_position();
    for slot in 1..=count {
        if rect_contains(cell_rect(slot - 1, rows), mouse) {
            let snippet = snippet_for(slot, state.is_custom);
            let ok = rl.set_clipboard_text(&snippet).is_ok();
            let msg = if ok {
                format!("copied {snippet} to clipboard")
            } else {
                format!("{snippet} (clipboard unavailable)")
            };
            crate::msg::info!("{msg}");
            return Some(msg);
        }
    }
    None
}

pub(super) fn draw(d: &mut RaylibDrawHandle, font: &Font, state: &State) {
    const SMALL: f32 = (crate::font::MONOGRAM_SIZE * 2) as f32;

    d.gui_panel(
        Rectangle::new(PANEL_X, PANEL_Y, PANEL_W, PANEL_H),
        "ColorPalette",
    );

    let count = state.palette.len();
    let header = if state.is_custom {
        format!("{count} colors from palette.png. Click a swatch to copy its 1-based slot index.")
    } else {
        "16 engine colors and their gfx.COLOR_* names. Click a swatch to copy.".to_owned()
    };
    d.draw_text_ex(
        font,
        &header,
        Vector2::new(PANEL_X + 10.0, PANEL_Y + 30.0),
        SMALL,
        0.0,
        color(Pal::DarkBlue),
    );

    let rows = rows_for(count);
    let mouse = d.get_mouse_position();

    for slot in 1..=count {
        let grid_idx = slot - 1;
        let cell = cell_rect(grid_idx, rows);
        let swatch = swatch_rect(cell);
        let hovered = rect_contains(cell, mouse);

        // Swatch reads from the LOCAL palette (the project's palette.png
        // when present), not the engine's globally-active palette.
        d.draw_rectangle_rec(swatch, state.palette.lookup(slot as i32));
        // Tool chrome (frame, hover accent) stays on engine defaults
        // via `palette::color` so the UI looks consistent regardless
        // of the project palette.
        let (border_color, border_thickness) = if hovered {
            (color(Pal::Pink), 3.0)
        } else {
            (color(Pal::DarkBlue), 1.0)
        };
        d.draw_rectangle_lines_ex(swatch, border_thickness, border_color);

        d.draw_text_ex(
            font,
            &label_for(slot, state.is_custom),
            Vector2::new(cell.x + CELL_PAD, cell.y + cell.height - LABEL_H + 2.0),
            SMALL,
            0.0,
            color(Pal::DarkBlue),
        );
    }

    let hint = if state.is_custom {
        "click: copy slot index to clipboard"
    } else {
        "click: copy gfx.COLOR_* to clipboard"
    };
    d.draw_text_ex(
        font,
        hint,
        Vector2::new(PANEL_X + 10.0, HINT_Y),
        SMALL,
        0.0,
        color(Pal::DarkGray),
    );
}
