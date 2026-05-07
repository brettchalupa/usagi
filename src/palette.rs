//! Pico-8's 16-color palette. Values outside 0-15 return magenta as an
//! obvious "unknown color" sentinel.

use sola_raylib::prelude::*;

/// Typed palette entry for engine-side callers. Pass either a `Pal`
/// variant or a raw `i32` to `palette(...)` — the function accepts
/// anything that converts to `i32`. The Lua bridge keeps passing raw
/// integers since it has to validate untrusted user input anyway.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Pal {
    Black = 0,
    DarkBlue = 1,
    DarkPurple = 2,
    DarkGreen = 3,
    Brown = 4,
    DarkGray = 5,
    LightGray = 6,
    White = 7,
    Red = 8,
    Orange = 9,
    Yellow = 10,
    Green = 11,
    Blue = 12,
    Indigo = 13,
    Pink = 14,
    Peach = 15,
}

impl From<Pal> for i32 {
    #[inline]
    fn from(p: Pal) -> i32 {
        p as i32
    }
}

/// Maps a palette index (0-15) to an RGBA color. Accepts a `Pal`
/// variant or any `i32`. Out-of-range indices return magenta as an
/// obvious sentinel.
pub fn color(c: impl Into<i32>) -> Color {
    match c.into() {
        0 => Color::new(0, 0, 0, 255),        // black
        1 => Color::new(29, 43, 83, 255),     // dark blue
        2 => Color::new(126, 37, 83, 255),    // dark purple
        3 => Color::new(0, 135, 81, 255),     // dark green
        4 => Color::new(171, 82, 54, 255),    // brown
        5 => Color::new(95, 87, 79, 255),     // dark gray
        6 => Color::new(194, 195, 199, 255),  // light gray
        7 => Color::new(255, 241, 232, 255),  // white
        8 => Color::new(255, 0, 77, 255),     // red
        9 => Color::new(255, 163, 0, 255),    // orange
        10 => Color::new(255, 236, 39, 255),  // yellow
        11 => Color::new(0, 228, 54, 255),    // green
        12 => Color::new(41, 173, 255, 255),  // blue
        13 => Color::new(131, 118, 156, 255), // indigo
        14 => Color::new(255, 119, 168, 255), // pink
        15 => Color::new(255, 204, 170, 255), // peach
        _ => Color::new(255, 0, 255, 255),    // magenta (unknown)
    }
}

pub fn index_for_rgb(r: u8, g: u8, b: u8) -> Option<i32> {
    (0..=15).find(|&idx| {
        let c = color(idx);
        (c.r, c.g, c.b) == (r, g, b)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_rgb(c: Color, r: u8, g: u8, b: u8) {
        assert_eq!((c.r, c.g, c.b, c.a), (r, g, b, 255));
    }

    #[test]
    fn black() {
        assert_rgb(color(0), 0, 0, 0);
    }

    #[test]
    fn white() {
        assert_rgb(color(7), 255, 241, 232);
    }

    #[test]
    fn red() {
        assert_rgb(color(8), 255, 0, 77);
    }

    #[test]
    fn peach() {
        assert_rgb(color(15), 255, 204, 170);
    }

    #[test]
    fn every_palette_index_is_opaque() {
        for i in 0..=15 {
            assert_eq!(color(i).a, 255, "index {i} should be fully opaque");
        }
    }

    #[test]
    fn exact_rgb_maps_back_to_palette_index() {
        for i in 0..=15 {
            let c = color(i);
            assert_eq!(index_for_rgb(c.r, c.g, c.b), Some(i));
        }
    }

    #[test]
    fn non_palette_rgb_returns_none() {
        assert_eq!(index_for_rgb(1, 2, 3), None);
    }

    #[test]
    fn unknown_indices_return_magenta() {
        let magenta = Color::new(255, 0, 255, 255);
        for i in [-1, 16, 99, i32::MAX, i32::MIN] {
            let c = color(i);
            assert_eq!(
                (c.r, c.g, c.b, c.a),
                (magenta.r, magenta.g, magenta.b, magenta.a),
                "index {i} should return magenta"
            );
        }
    }
}
