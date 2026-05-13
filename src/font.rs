//! Engine font loader.
//!
//! Two fonts coexist at runtime:
//!
//! - The **bundled** monogram-extended atlas (CC0 by datagoblin,
//!   <https://datagoblin.itch.io/monogram>) powers engine UI overlays
//!   (FPS, REC indicator, pause menu, error overlay, tools window).
//!   It's always loaded and its metrics are stable, so engine layout
//!   doesn't depend on what the user dropped in their project.
//!
//! - The **user** font (`font.png` at project root, baked via
//!   `usagi font bake` so glyph metadata lives in a zTXt chunk) is
//!   loaded when present, and is used only for the Lua-facing API
//!   (`gfx.text`, `gfx.text_ex`, `usagi.measure_text`). If absent,
//!   those APIs fall back to the bundled font.
//!
//! Atlas + per-codepoint metadata are baked offline by `usagi font
//! bake`, which rasterizes TTF/OTF outlines via FreeType in mono mode
//! and emits a single PNG with the metadata in a zTXt chunk.

use crate::vfs::VirtualFs;
use sola_raylib::consts::TextureFilter;
use sola_raylib::ffi;
use sola_raylib::prelude::*;
use std::mem;

// Bundled atlas with glyph metadata embedded as a zTXt chunk inside
// the PNG. Baked from `assets/monogram-extended.ttf` via the Rust +
// FreeType pipeline. To regenerate after upgrading the TTF:
//   cargo run -- font bake assets/monogram-extended.ttf 15 --out assets/monogram.png
const BUNDLED_PNG: &[u8] = include_bytes!("../assets/monogram.png");

const USER_FONT_PNG: &str = "font.png";
/// Keyword for the zTXt chunk inside a baked font PNG. Must match
/// `font_bake::METADATA_KEYWORD`.
const FONT_METADATA_KEYWORD: &str = "usagi-font";

/// Bundled monogram font's line height. Used by engine code paths that
/// always render with the bundled font (tools, error overlay) and need
/// a compile-time constant. Lua-facing code should use the loaded
/// font's `base_size()` so it adapts to user fonts.
pub const MONOGRAM_SIZE: i32 = 12;

pub fn load_bundled(rl: &mut RaylibHandle, thread: &RaylibThread) -> Font {
    let json = extract_embedded_metadata(BUNDLED_PNG)
        .expect("bundled monogram.png should carry the zTXt metadata chunk");
    build_font(rl, thread, BUNDLED_PNG, json.as_bytes())
        .expect("bundled monogram font should always build")
}

pub fn load_user(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    vfs: &dyn VirtualFs,
) -> Option<Font> {
    let png = vfs.read_file(USER_FONT_PNG)?;
    let json = match extract_embedded_metadata(&png) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "font: {} missing the zTXt metadata chunk ({}), falling back to bundled. Re-bake with `usagi font bake`.",
                USER_FONT_PNG, e
            );
            return None;
        }
    };
    match build_font(rl, thread, &png, json.as_bytes()) {
        Ok(font) => {
            eprintln!(
                "font: loaded user {} (baseSize {})",
                USER_FONT_PNG,
                font.base_size()
            );
            Some(font)
        }
        Err(e) => {
            eprintln!(
                "font: failed to load user {}: {}, falling back to bundled",
                USER_FONT_PNG, e
            );
            None
        }
    }
}

/// Scan a PNG byte stream for our zTXt metadata chunk and return its
/// decompressed text. Errs if the PNG is malformed or carries no such
/// chunk.
fn extract_embedded_metadata(png_bytes: &[u8]) -> Result<String, String> {
    let decoder = png::Decoder::new(png_bytes);
    let reader = decoder
        .read_info()
        .map_err(|e| format!("png decode: {e}"))?;
    let info = reader.info();
    for chunk in &info.compressed_latin1_text {
        if chunk.keyword == FONT_METADATA_KEYWORD {
            let mut owned = chunk.clone();
            owned
                .decompress_text()
                .map_err(|e| format!("decompress zTXt: {e}"))?;
            return owned.get_text().map_err(|e| format!("read zTXt text: {e}"));
        }
    }
    Err(format!("no {} zTXt chunk", FONT_METADATA_KEYWORD))
}

fn build_font(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    png: &[u8],
    json: &[u8],
) -> Result<Font, String> {
    let meta: serde_json::Value =
        serde_json::from_slice(json).map_err(|e| format!("parse json: {e}"))?;
    let line_height = meta["line_height"]
        .as_i64()
        .ok_or("json missing line_height")? as i32;
    let glyphs_map = meta["glyphs"]
        .as_object()
        .ok_or("json missing glyphs object")?;
    let glyph_count = glyphs_map.len();
    if glyph_count == 0 {
        return Err("json has zero glyphs".to_owned());
    }

    // PNG is white-on-transparent already (the baker emits indexed
    // 1-bit with a tRNS chunk), so we can upload it as-is and tints
    // via per-channel multiply will work.
    let img = Image::load_image_from_mem(".png", png).map_err(|e| format!("decode png: {e}"))?;
    let texture = rl
        .load_texture_from_image(thread, &img)
        .map_err(|e| format!("upload texture: {e}"))?;
    texture.set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_POINT);

    // raylib's UnloadFont calls RL_FREE on these pointers, so they
    // must come from raylib's allocator. Construct the Font via ffi.
    unsafe {
        let glyphs_ptr = ffi::MemAlloc((glyph_count * mem::size_of::<ffi::GlyphInfo>()) as u32)
            as *mut ffi::GlyphInfo;
        let recs_ptr = ffi::MemAlloc((glyph_count * mem::size_of::<ffi::Rectangle>()) as u32)
            as *mut ffi::Rectangle;
        assert!(!glyphs_ptr.is_null() && !recs_ptr.is_null());

        for (i, (cp_str, g)) in glyphs_map.iter().enumerate() {
            let cp: i32 = cp_str
                .parse()
                .map_err(|_| format!("glyph key {cp_str:?} not an integer codepoint"))?;
            let x = g["x"].as_i64().ok_or("glyph.x")? as f32;
            let y = g["y"].as_i64().ok_or("glyph.y")? as f32;
            let w = g["w"].as_i64().ok_or("glyph.w")? as f32;
            let h = g["h"].as_i64().ok_or("glyph.h")? as f32;
            let adv = g["advance"].as_i64().ok_or("glyph.advance")? as i32;
            let ox = g["ox"].as_i64().ok_or("glyph.ox")? as i32;
            let oy = g["oy"].as_i64().ok_or("glyph.oy")? as i32;

            *recs_ptr.add(i) = ffi::Rectangle {
                x,
                y,
                width: w,
                height: h,
            };
            *glyphs_ptr.add(i) = ffi::GlyphInfo {
                value: cp,
                offsetX: ox,
                offsetY: oy,
                advanceX: adv,
                image: mem::zeroed(),
            };
        }

        let mut raw: ffi::Font = mem::zeroed();
        raw.baseSize = line_height;
        raw.glyphCount = glyph_count as i32;
        raw.glyphPadding = 0;
        raw.texture = texture.to_raw();
        raw.recs = recs_ptr;
        raw.glyphs = glyphs_ptr;
        Ok(Font::from_raw(raw))
    }
}
