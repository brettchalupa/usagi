//! Pico-8's 16-color palette. Values outside 0-15 return magenta as an
//! obvious "unknown color" sentinel.

use sola_raylib::prelude::*;

/// Maps a palette index (0-15) to an RGBA color.
pub fn palette(c: i32) -> Color {
    match c {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_rgb(c: Color, r: u8, g: u8, b: u8) {
        assert_eq!((c.r, c.g, c.b, c.a), (r, g, b, 255));
    }

    #[test]
    fn black() {
        assert_rgb(palette(0), 0, 0, 0);
    }

    #[test]
    fn white() {
        assert_rgb(palette(7), 255, 241, 232);
    }

    #[test]
    fn red() {
        assert_rgb(palette(8), 255, 0, 77);
    }

    #[test]
    fn peach() {
        assert_rgb(palette(15), 255, 204, 170);
    }

    #[test]
    fn every_palette_index_is_opaque() {
        for i in 0..=15 {
            assert_eq!(palette(i).a, 255, "index {i} should be fully opaque");
        }
    }

    #[test]
    fn unknown_indices_return_magenta() {
        let magenta = Color::new(255, 0, 255, 255);
        for i in [-1, 16, 99, i32::MAX, i32::MIN] {
            let c = palette(i);
            assert_eq!(
                (c.r, c.g, c.b, c.a),
                (magenta.r, magenta.g, magenta.b, magenta.a),
                "index {i} should return magenta"
            );
        }
    }
}
