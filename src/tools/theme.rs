//! Dark theme for the `usagi tools` window. Single source of truth so
//! the four tool panes (tilepicker, jukebox, save inspector, color
//! palette) read consistently, and so swapping the overall feel later
//! is one file. The `mod.rs::apply_theme` function pumps these colors
//! into raygui's style table; the per-tool draw code reads them
//! directly for text and highlights.
//!
//! Color palette tool is deliberately left alone: it visualizes the
//! game's own palette, not the tool theme.

use sola_raylib::prelude::Color;

/// Window backdrop. The darkest layer behind every panel.
pub const BG: Color = Color {
    r: 0x1e,
    g: 0x1e,
    b: 0x1e,
    a: 0xff,
};

/// Panel and button base. One step lighter than `BG` so panels
/// register as raised content.
pub const SURFACE: Color = Color {
    r: 0x2a,
    g: 0x2a,
    b: 0x2a,
    a: 0xff,
};

/// Slightly lighter than `SURFACE`. Used for the panel header strip
/// so it reads as a title bar without needing a different hue.
pub const SURFACE_HIGH: Color = Color {
    r: 0x33,
    g: 0x33,
    b: 0x33,
    a: 0xff,
};

/// Subtle outline. Borders / dividers / disabled element edges.
pub const BORDER: Color = Color {
    r: 0x44,
    g: 0x44,
    b: 0x44,
    a: 0xff,
};

/// Primary text. Off-white so it doesn't burn against the dark
/// backdrop the way pure white does.
pub const TEXT: Color = Color {
    r: 0xd4,
    g: 0xd4,
    b: 0xd4,
    a: 0xff,
};

/// Secondary text. For supporting info (e.g. status readouts that
/// aren't the main thing the user is looking at). Tuned bright enough
/// to read comfortably on the panel surface; #a0 was too dim against
/// the gray bg.
pub const TEXT_DIM: Color = Color {
    r: 0xbc,
    g: 0xbc,
    b: 0xbc,
    a: 0xff,
};

/// Tertiary text. Hints, disabled labels, low-priority instructions.
/// Still meant to read as a degraded step from primary, but pushed up
/// from the original #80 so it doesn't fade out against the surface.
pub const TEXT_MUTED: Color = Color {
    r: 0x9c,
    g: 0x9c,
    b: 0x9c,
    a: 0xff,
};

/// Error / destructive accent. Used sparingly: save inspector's
/// "couldn't read save" message, etc.
pub const DANGER: Color = Color {
    r: 0xe0,
    g: 0x6c,
    b: 0x6c,
    a: 0xff,
};

/// Focus / interactive highlight. Calm muted blue, the one pop of
/// color in the otherwise gray theme.
pub const ACCENT: Color = Color {
    r: 0x56,
    g: 0x9c,
    b: 0xd6,
    a: 0xff,
};

/// Text drawn directly on top of an `ACCENT` fill (e.g. inside a
/// pressed button).
pub const ON_ACCENT: Color = BG;

/// Committed selection in the tilepicker. Warm enough to pop against
/// the dark panel without competing with `ACCENT`.
pub const SELECTION: Color = Color {
    r: 0xe0,
    g: 0xc0,
    b: 0x60,
    a: 0xff,
};
