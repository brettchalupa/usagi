//! CPU-side pixel buffers backing the `gfx.px` (screen) and
//! `gfx.spr_px` (sprite sheet) read APIs. Each holds a copy of the
//! pixels in top-down row-major order so callers can sample by (x, y)
//! without allocating per call and without thinking about OpenGL's
//! bottom-up framebuffer convention.

use sola_raylib::prelude::*;

/// A read-only snapshot of a 2D pixel buffer.
pub struct Pixels {
    pub width: i32,
    pub height: i32,
    data: Vec<Color>,
}

impl Pixels {
    /// Reads a render texture back from the GPU. Flips vertically so
    /// `(0, 0)` is the top-left in caller coordinates (raylib stores
    /// render targets bottom-up under OpenGL). Returns `None` if the
    /// readback fails.
    pub fn from_render_texture(rt: &RenderTexture2D) -> Option<Self> {
        let mut image = rt.texture().load_image().ok()?;
        image.flip_vertical();
        Some(Self::from_image(&image))
    }

    /// Copies pixels out of an existing CPU-side `Image`. Used for the
    /// sprite sheet, which is decoded once at load time and kept in
    /// CPU memory for the lifetime of the sheet.
    pub fn from_image(image: &Image) -> Self {
        Self {
            width: image.width,
            height: image.height,
            data: image.get_image_data().to_vec(),
        }
    }

    /// Samples the pixel at `(x, y)`. Returns `None` for out-of-bounds
    /// coordinates so callers can map that to a Lua `nil`.
    pub fn get(&self, x: i32, y: i32) -> Option<Color> {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return None;
        }
        let idx = (y as usize) * (self.width as usize) + (x as usize);
        self.data.get(idx).copied()
    }
}

/// Shape returned to Lua by `gfx.px` and `gfx.spr_px`: r, g, b, and the
/// 1-based palette index of an exact RGB match (or `None` when the
/// color isn't in the active palette). All four slots are `None` when
/// the read fails outright (no snapshot yet, out-of-bounds, unknown
/// sprite index), which the Lua bridge surfaces as four `nil` returns.
pub type LuaPixel = (Option<i32>, Option<i32>, Option<i32>, Option<i32>);

/// All-nil read result. Reused as the fallback for every miss path
/// (no snapshot, out-of-bounds, invalid sprite index) so the four
/// branches stay readable.
pub const NIL_PIXEL: LuaPixel = (None, None, None, None);

fn color_to_lua(c: Color) -> LuaPixel {
    (
        Some(c.r as i32),
        Some(c.g as i32),
        Some(c.b as i32),
        crate::palette::index_of(c.r, c.g, c.b),
    )
}

/// `gfx.px(x, y)` body: reads a screen pixel from the most recent
/// frame snapshot, or returns all `None` when no snapshot exists yet
/// (first frame of the session) or the coordinates fall outside the
/// game-space resolution. Floats are rounded to the nearest int.
pub fn read_screen(snapshot: Option<&Pixels>, x: f32, y: f32) -> LuaPixel {
    let xi = x.round() as i32;
    let yi = y.round() as i32;
    snapshot
        .and_then(|p| p.get(xi, yi))
        .map(color_to_lua)
        .unwrap_or(NIL_PIXEL)
}

/// `gfx.spr_px(idx, x, y)` body: resolves a 1-based sprite index plus
/// `(x, y)` inside the cell into a sheet pixel coordinate and reads
/// from the CPU-side sheet mirror. Returns all `None` on any miss
/// (no sheet loaded, idx out of range, (x, y) outside the cell) or
/// when the sampled pixel is fully transparent. `gfx.spr` draws
/// alpha-keyed sprites, so a fully transparent pixel reads as "no
/// pixel here" rather than as its (typically black) backing RGB.
pub fn read_sprite(snapshot: Option<&Pixels>, cell: i32, idx: i32, x: f32, y: f32) -> LuaPixel {
    let Some(pixels) = snapshot else {
        return NIL_PIXEL;
    };
    if idx < 1 || cell < 1 {
        return NIL_PIXEL;
    }
    let xi = x.round() as i32;
    let yi = y.round() as i32;
    if xi < 0 || yi < 0 || xi >= cell || yi >= cell {
        return NIL_PIXEL;
    }
    let cols = pixels.width / cell;
    if cols <= 0 {
        return NIL_PIXEL;
    }
    let idx0 = idx - 1;
    let col = idx0 % cols;
    let row = idx0 / cols;
    if row * cell >= pixels.height {
        return NIL_PIXEL;
    }
    match pixels.get(col * cell + xi, row * cell + yi) {
        Some(c) if c.a > 0 => color_to_lua(c),
        _ => NIL_PIXEL,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(width: i32, height: i32, pixels: Vec<Color>) -> Pixels {
        assert_eq!(pixels.len(), (width * height) as usize);
        Pixels {
            width,
            height,
            data: pixels,
        }
    }

    #[test]
    fn get_returns_color_at_coordinates() {
        let red = Color::new(255, 0, 0, 255);
        let green = Color::new(0, 255, 0, 255);
        let blue = Color::new(0, 0, 255, 255);
        let white = Color::new(255, 255, 255, 255);
        // 2x2: row 0 = [red, green], row 1 = [blue, white].
        let p = buf(2, 2, vec![red, green, blue, white]);
        assert_eq!(p.get(0, 0).map(|c| (c.r, c.g, c.b)), Some((255, 0, 0)));
        assert_eq!(p.get(1, 0).map(|c| (c.r, c.g, c.b)), Some((0, 255, 0)));
        assert_eq!(p.get(0, 1).map(|c| (c.r, c.g, c.b)), Some((0, 0, 255)));
        assert_eq!(p.get(1, 1).map(|c| (c.r, c.g, c.b)), Some((255, 255, 255)));
    }

    #[test]
    fn get_returns_none_for_negative_coords() {
        let p = buf(2, 2, vec![Color::WHITE; 4]);
        assert!(p.get(-1, 0).is_none());
        assert!(p.get(0, -1).is_none());
        assert!(p.get(-1, -1).is_none());
    }

    #[test]
    fn read_sprite_returns_nils_for_fully_transparent_pixel() {
        // 2x2 sheet, treated as a single 2x2 cell. Pixel (0,0) is a
        // fully transparent black; the rest are opaque white. The
        // transparent pixel should read as four nils so callers using
        // sprite reads for alpha-keyed scans skip it instead of
        // plotting its backing RGB (which would render as black).
        let transparent_black = Color::new(0, 0, 0, 0);
        let opaque_white = Color::new(255, 255, 255, 255);
        let sheet = Pixels {
            width: 2,
            height: 2,
            data: vec![transparent_black, opaque_white, opaque_white, opaque_white],
        };
        let (r, g, b, idx) = read_sprite(Some(&sheet), 2, 1, 0.0, 0.0);
        assert!(r.is_none() && g.is_none() && b.is_none() && idx.is_none());
        // Sanity: an opaque cell-mate at (1, 0) reads through normally.
        let (r2, _, _, _) = read_sprite(Some(&sheet), 2, 1, 1.0, 0.0);
        assert_eq!(r2, Some(255));
    }

    #[test]
    fn get_returns_none_outside_width_or_height() {
        let p = buf(2, 2, vec![Color::WHITE; 4]);
        assert!(p.get(2, 0).is_none(), "x == width is out of bounds");
        assert!(p.get(0, 2).is_none(), "y == height is out of bounds");
        assert!(p.get(99, 99).is_none());
    }
}
