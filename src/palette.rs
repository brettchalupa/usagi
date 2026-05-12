//! Color palette. Default is Pico-8's 16-color palette; user games can
//! override by dropping a `palette.png` at the project root (read in
//! row-major order, any rectangular size). Slot indices are **1-based**
//! to match `gfx.spr` and Lua's array convention — slot 1 is the
//! first color, slot N is the Nth. Values outside the active palette's
//! range (including 0) return magenta as an obvious "unknown color"
//! sentinel.

use sola_raylib::prelude::*;
use std::cell::RefCell;

/// Typed palette entry for engine-side callers. Pass either a `Pal`
/// variant or a raw `i32` to `color(...)` — the function accepts
/// anything that converts to `i32`. The Lua bridge keeps passing raw
/// integers since it has to validate untrusted user input anyway.
///
/// These names are **slot indices** keyed to the Pico-8 default
/// ordering. When a user supplies a custom `palette.png`, the slot at
/// index 1 still resolves through `Pal::Black` but its actual RGB is
/// whatever the user put there. Define your own constants in Lua if
/// you swap palettes and want names that match.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Pal {
    Black = 1,
    DarkBlue = 2,
    DarkPurple = 3,
    DarkGreen = 4,
    Brown = 5,
    DarkGray = 6,
    LightGray = 7,
    White = 8,
    Red = 9,
    Orange = 10,
    Yellow = 11,
    Green = 12,
    Blue = 13,
    Indigo = 14,
    Pink = 15,
    Peach = 16,
}

impl From<Pal> for i32 {
    #[inline]
    fn from(p: Pal) -> i32 {
        p as i32
    }
}

const MAGENTA_SENTINEL: Color = Color::new(255, 0, 255, 255);

const PICO8_COLORS: [(u8, u8, u8); 16] = [
    (0, 0, 0),       // black
    (29, 43, 83),    // dark blue
    (126, 37, 83),   // dark purple
    (0, 135, 81),    // dark green
    (171, 82, 54),   // brown
    (95, 87, 79),    // dark gray
    (194, 195, 199), // light gray
    (255, 241, 232), // white
    (255, 0, 77),    // red
    (255, 163, 0),   // orange
    (255, 236, 39),  // yellow
    (0, 228, 54),    // green
    (41, 173, 255),  // blue
    (131, 118, 156), // indigo
    (255, 119, 168), // pink
    (255, 204, 170), // peach
];

/// A user-loadable or built-in color palette. Stores raw colors in
/// order; index lookup returns magenta for out-of-range slots.
#[derive(Clone, Debug)]
pub struct Palette {
    colors: Vec<Color>,
}

impl Palette {
    /// The 16-color Pico-8 palette. Engine default.
    pub fn pico8() -> Self {
        Self {
            colors: PICO8_COLORS
                .iter()
                .map(|&(r, g, b)| Color::new(r, g, b, 255))
                .collect(),
        }
    }

    /// Parse `palette.png` bytes into a palette. Pixels are read in
    /// row-major order (left-to-right, top-to-bottom). Any rectangular
    /// shape works: a 16x1 strip, a 16x2 grid (32 colors), or a 4x4
    /// (16 colors) all produce a palette equal to width × height.
    /// Each pixel must be a distinct color you want at that slot —
    /// use lospec.com's "1px cells" export rather than the larger
    /// cell-block exports.
    pub fn from_image_bytes(bytes: &[u8]) -> Result<Self, String> {
        let image = Image::load_image_from_mem(".png", bytes)
            .map_err(|e| format!("decoding palette.png: {e}"))?;
        let w = image.width as usize;
        let h = image.height as usize;
        if w == 0 || h == 0 {
            return Err("palette.png is empty".to_owned());
        }
        let pixels = image.get_image_data();
        let colors: Vec<Color> = pixels
            .iter()
            .take(w * h)
            .map(|c| Color::new(c.r, c.g, c.b, c.a))
            .collect();
        if colors.is_empty() {
            return Err("palette.png decoded to zero pixels".to_owned());
        }
        Ok(Self { colors })
    }

    pub fn len(&self) -> usize {
        self.colors.len()
    }

    /// Look up a color by 1-based slot index. `0` and out-of-range
    /// values return magenta sentinel.
    pub fn lookup(&self, idx: i32) -> Color {
        if idx < 1 {
            return MAGENTA_SENTINEL;
        }
        self.colors
            .get((idx - 1) as usize)
            .copied()
            .unwrap_or(MAGENTA_SENTINEL)
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self::pico8()
    }
}

thread_local! {
    /// The active palette. Lookups via `palette::color(idx)` resolve
    /// through this. Session and tools entry points call `set_active`
    /// at startup (and on `palette.png` hot-reload) to override.
    /// raylib is single-threaded, so thread-local is effectively
    /// process-global here.
    static ACTIVE: RefCell<Palette> = RefCell::new(Palette::pico8());
}

/// Replaces the active palette. Subsequent `color()` calls resolve
/// through the new palette. Cheap; clones a Vec<Color>.
pub fn set_active(palette: Palette) {
    ACTIVE.with(|p| *p.borrow_mut() = palette);
}

/// Maps a palette index to an RGBA color via the active palette.
/// Accepts a `Pal` variant or any `i32`. Out-of-range indices return
/// magenta as an obvious sentinel.
pub fn color(c: impl Into<i32>) -> Color {
    let idx = c.into();
    ACTIVE.with(|p| p.borrow().lookup(idx))
}

/// Reverse lookup: returns the 1-based slot index of the active
/// palette's exact RGB match, or `None` if the color isn't in the
/// palette. Used by the screen and sprite pixel-read APIs so games
/// can branch on palette identity (e.g. "is this red?") rather than
/// on raw RGB triples. Alpha is ignored: palette entries are all
/// opaque, but sprite samples carry alpha that the caller already
/// handles separately.
pub fn index_of(r: u8, g: u8, b: u8) -> Option<i32> {
    ACTIVE.with(|p| {
        p.borrow()
            .colors
            .iter()
            .position(|c| c.r == r && c.g == g && c.b == b)
            .map(|i| (i + 1) as i32)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_rgb(c: Color, r: u8, g: u8, b: u8) {
        assert_eq!((c.r, c.g, c.b, c.a), (r, g, b, 255));
    }

    /// Reset the active palette before each test that relies on the
    /// default. Other tests in this module install custom palettes, so
    /// don't trust the thread-local state from previous tests.
    fn reset() {
        set_active(Palette::pico8());
    }

    #[test]
    fn black() {
        reset();
        assert_rgb(color(1), 0, 0, 0);
    }

    #[test]
    fn white() {
        reset();
        assert_rgb(color(8), 255, 241, 232);
    }

    #[test]
    fn red() {
        reset();
        assert_rgb(color(9), 255, 0, 77);
    }

    #[test]
    fn peach() {
        reset();
        assert_rgb(color(16), 255, 204, 170);
    }

    #[test]
    fn every_palette_index_is_opaque() {
        reset();
        for i in 1..=16 {
            assert_eq!(color(i).a, 255, "index {i} should be fully opaque");
        }
    }

    #[test]
    fn unknown_indices_return_magenta() {
        reset();
        let magenta = Color::new(255, 0, 255, 255);
        // 0 is now an out-of-range sentinel too (slots are 1-based).
        for i in [-1, 0, 17, 99, i32::MAX, i32::MIN] {
            let c = color(i);
            assert_eq!(
                (c.r, c.g, c.b, c.a),
                (magenta.r, magenta.g, magenta.b, magenta.a),
                "index {i} should return magenta"
            );
        }
    }

    #[test]
    fn pico8_palette_has_16_colors() {
        assert_eq!(Palette::pico8().len(), 16);
    }

    #[test]
    fn index_of_returns_one_based_slot_for_every_pico8_color() {
        reset();
        for slot in 1..=16i32 {
            let c = color(slot);
            assert_eq!(
                index_of(c.r, c.g, c.b),
                Some(slot),
                "slot {slot}'s RGB should round-trip"
            );
        }
    }

    #[test]
    fn index_of_returns_none_for_off_palette_rgb() {
        reset();
        // Pure 1-bit color isn't in the Pico-8 palette: black is (0,0,0)
        // and red is (255,0,77), so neither (1,1,1) nor (255,0,0) match.
        assert!(index_of(1, 1, 1).is_none());
        assert!(index_of(255, 0, 0).is_none());
    }

    #[test]
    fn index_of_ignores_alpha() {
        reset();
        let black = color(1);
        assert_eq!(index_of(black.r, black.g, black.b), Some(1));
    }

    #[test]
    fn index_of_uses_active_palette() {
        let custom = Palette {
            colors: vec![Color::new(10, 20, 30, 255), Color::new(40, 50, 60, 255)],
        };
        set_active(custom);
        assert_eq!(index_of(10, 20, 30), Some(1));
        assert_eq!(index_of(40, 50, 60), Some(2));
        // Black is in Pico-8 but not in this custom palette.
        assert!(index_of(0, 0, 0).is_none());
        reset();
    }

    #[test]
    fn custom_palette_replaces_lookups() {
        let custom = Palette {
            colors: vec![Color::new(10, 20, 30, 255), Color::new(40, 50, 60, 255)],
        };
        set_active(custom);
        // Slot 1 is the first color, slot 2 is the second. Slot 0 is
        // out-of-range (1-based indexing).
        assert_rgb(color(1), 10, 20, 30);
        assert_rgb(color(2), 40, 50, 60);
        let c0 = color(0);
        assert_eq!((c0.r, c0.g, c0.b), (255, 0, 255));
        // Beyond the custom palette's range -> magenta, not Pico-8's
        // dark purple. Slot indices honor the active palette length.
        let c3 = color(3);
        assert_eq!((c3.r, c3.g, c3.b), (255, 0, 255));
        reset();
    }
}
