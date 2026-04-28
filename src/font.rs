//! Bundled monogram font (CC0 by datagoblin).
//! <https://datagoblin.itch.io/monogram>
//!
//! Use the pre-rasterized bitmap atlas (`assets/monogram.png`)
//! rather than the TTF for crisp, scaled text.
//!
//! Atlas layout: 96×96 RGBA, 16 cells × 8 cells, each cell 6×12 px.
//! Cell index = codepoint − 32 (so cell 0 is space, cell 33 is 'A',
//! and the last printable cell is 94 = '~'). Each cell has 1 px of
//! left padding and uses the right 5 px for the glyph; vertically the
//! 12 px contains 2 px ascender + 5 px lowercase + 2 px descender +
//! 3 px of layout slack.
//!
//! Per-glyph advance widths come from the upstream JSON glyph data;
//! they're baked here as a fixed table since the font is frozen.

use sola_raylib::consts::TextureFilter;
use sola_raylib::ffi;
use sola_raylib::prelude::*;
use std::mem;

const MONOGRAM_PNG: &[u8] = include_bytes!("../assets/monogram.png");

/// Cell dimensions in the atlas.
const CELL_W: i32 = 6;
const CELL_H: i32 = 12;
/// Cells per row in the atlas (96 / 6).
const ATLAS_COLS: i32 = 16;
/// First codepoint represented in the atlas. Cell 0 is space (0x20).
const FIRST_CODEPOINT: i32 = 32;
/// Number of codepoints in the bundled atlas: 32..=126 (95 glyphs).
const GLYPH_COUNT: usize = 95;

/// Logical line height. Use `usagi.measure_text(text)` from Lua to get
/// the same value at runtime; this constant exists for engine code
/// (FPS overlay, error overlay) that doesn't go through the Lua API.
pub const MONOGRAM_SIZE: i32 = CELL_H;

/// Per-glyph advance widths in pixels, indexed by `codepoint - 32`.
/// Computed from the upstream `monogram-bitmap.json` as
/// `max(row.bit_length() across rows) + 1`, with space ' ' set to 4
/// (its bitmap is all-zero so the formula gives 0). Frozen at the
/// monogram release shipped: `assets/monogram.png`. When upgrading the bundled
/// font, regenerate this table from the matching
/// JSON.
const ADVANCES: [i32; GLYPH_COUNT] = [
    4, 4, 5, 6, 6, 6, 6, 4, 5, 4, 6, 6, 4, 6, 4, 6, // 32..47
    6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 4, 4, 6, 6, 6, 6, // 48..63
    6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, // 64..79
    6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 5, 6, 4, 6, 6, // 80..95
    4, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, // 96..111
    6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 5, 4, 5, 6, // 112..126
];

pub fn load(rl: &mut RaylibHandle, thread: &RaylibThread) -> Font {
    // Decode the bundled PNG into a CPU-side Image. Safe wrapper —
    // raylib copies MONOGRAM_PNG into its own buffer, so the source
    // slice doesn't need to outlive this call.
    let mut img = Image::load_image_from_mem(".png", MONOGRAM_PNG)
        .expect("monogram.png bundled at compile time should always decode");

    // The upstream PNG stores "on" pixels as black-opaque (RGB=0,0,0
    // + A=255) and "off" as fully transparent. raylib tints text via
    // per-channel multiply, so a black texel zeroes out any color tint
    // and every glyph would render black. Whiten the opaque pixels so
    // `texel × tint = tint` and `gfx.text` honors the requested color.
    img.color_replace(Color::new(0, 0, 0, 255), Color::new(255, 255, 255, 255));

    // Upload to GPU; CPU-side `img` drops at end of scope.
    let texture = rl
        .load_texture_from_image(thread, &img)
        .expect("monogram texture upload should succeed");
    // POINT (nearest-neighbor) sampling on the atlas — pixel art;
    // bilinear would soften every glyph edge.
    texture.set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_POINT);

    // Construct the Font from raw FFI: raylib's `UnloadFont` will
    // call `RL_FREE` on `glyphs` and `recs`, so they MUST come from
    // raylib's allocator (`MemAlloc`). The `mem::zeroed` + manual
    // field assignment + `Font::from_raw` dance is unsafe — there's
    // no safe sola-raylib API that builds a Font from a pre-existing
    // texture + glyph metadata. Once Font is constructed, the rest of
    // the engine treats it as a normal owned `Font`.
    unsafe {
        let glyphs_ptr = ffi::MemAlloc((GLYPH_COUNT * mem::size_of::<ffi::GlyphInfo>()) as u32)
            as *mut ffi::GlyphInfo;
        let recs_ptr = ffi::MemAlloc((GLYPH_COUNT * mem::size_of::<ffi::Rectangle>()) as u32)
            as *mut ffi::Rectangle;
        assert!(!glyphs_ptr.is_null() && !recs_ptr.is_null());

        for (i, advance) in ADVANCES.iter().enumerate() {
            let codepoint = FIRST_CODEPOINT + i as i32;
            let cell_idx = i as i32;
            let col = cell_idx % ATLAS_COLS;
            let row = cell_idx / ATLAS_COLS;
            *recs_ptr.add(i) = ffi::Rectangle {
                x: (col * CELL_W) as f32,
                y: (row * CELL_H) as f32,
                width: CELL_W as f32,
                height: CELL_H as f32,
            };
            // image.data = NULL is safe: UnloadImage no-ops on NULL.
            // We keep the per-glyph image empty because the GPU atlas
            // is what actually gets sampled when drawing; the per-
            // glyph image field is only used by `ImageDrawText` (CPU
            // text into an Image), which we never invoke.
            *glyphs_ptr.add(i) = ffi::GlyphInfo {
                value: codepoint,
                offsetX: 0,
                offsetY: 0,
                advanceX: *advance,
                image: mem::zeroed(),
            };
        }

        let mut raw: ffi::Font = mem::zeroed();
        raw.baseSize = CELL_H;
        raw.glyphCount = GLYPH_COUNT as i32;
        raw.glyphPadding = 0;
        // `to_raw` consumes the Texture2D wrapper without dropping it
        // (no `UnloadTexture`), transferring ownership of the GPU
        // resource into the Font. raylib's `UnloadFont` will free the
        // texture along with glyphs and recs.
        raw.texture = texture.to_raw();
        raw.recs = recs_ptr;
        raw.glyphs = glyphs_ptr;
        Font::from_raw(raw)
    }
}
