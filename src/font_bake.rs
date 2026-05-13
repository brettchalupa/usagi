//! `usagi font bake` implementation. Rasterizes a TTF/OTF into a single
//! PNG that `src/font.rs` can load as a custom font (the atlas image
//! plus glyph metadata embedded as a compressed `zTXt` chunk).

use freetype::Library;
use freetype::face::LoadFlag;
use std::collections::BTreeMap;
use std::fs;
use std::io::BufWriter;
use std::path::Path;

/// Inclusive Unicode ranges always included in the bake. Codepoints
/// the font doesn't cover are filtered via FreeType's cmap lookup.
const RANGES: &[(u32, u32)] = &[
    (0x0020, 0x007E), // Basic Latin
    (0x00A0, 0x00FF), // Latin-1 Supplement
    (0x0100, 0x017F), // Latin Extended-A
    (0x0180, 0x024F), // Latin Extended-B
    (0x0370, 0x03FF), // Greek and Coptic
    (0x0400, 0x04FF), // Cyrillic
    (0x2010, 0x205E), // General Punctuation
    (0x3000, 0x303F), // CJK Symbols and Punctuation
    (0x3040, 0x309F), // Hiragana
    (0x30A0, 0x30FF), // Katakana
    (0xFF00, 0xFFEF), // Halfwidth and Fullwidth Forms
];

/// Included by default; pass `--no-cjk` to skip. Adds the full CJK
/// Unified Ideographs block; the cmap filter drops codepoints the
/// font doesn't cover, so this is safe to enable for any font.
const CJK_UNIFIED: (u32, u32) = (0x4E00, 0x9FFF);

const ATLAS_MAX_WIDTH: u32 = 512;

pub fn run(ttf_path: &Path, size: u32, out_path: &Path, include_cjk: bool) -> Result<(), String> {
    let bytes = fs::read(ttf_path).map_err(|e| format!("reading {}: {e}", ttf_path.display()))?;
    let lib = Library::init().map_err(|e| format!("freetype init: {e}"))?;
    let face = lib
        .new_memory_face(bytes, 0)
        .map_err(|e| format!("loading {}: {e}", ttf_path.display()))?;
    face.set_pixel_sizes(0, size)
        .map_err(|e| format!("set_pixel_sizes({size}): {e}"))?;

    // Size metrics fields are FT_F26Dot6 (1/64 pixel); shift to int px.
    let size_metrics = face
        .size_metrics()
        .ok_or("font has no size metrics after set_pixel_sizes")?;
    let ascent = (size_metrics.ascender >> 6) as i32;
    let descent = (-size_metrics.descender >> 6) as i32;
    let line_height = ascent + descent;

    let mut glyphs: Vec<GlyphData> = Vec::new();
    let mut bitmaps: Vec<Bitmap> = Vec::new();

    for cp in iter_codepoints(include_cjk) {
        // Filter unmapped codepoints up front so the atlas doesn't fill
        // with .notdef placeholders.
        if face.get_char_index(cp as usize).is_none() {
            continue;
        }
        face.load_char(
            cp as usize,
            LoadFlag::RENDER | LoadFlag::MONOCHROME | LoadFlag::TARGET_MONO,
        )
        .map_err(|e| format!("load_char U+{cp:04X}: {e}"))?;
        let slot = face.glyph();
        let bitmap = slot.bitmap();
        let w = bitmap.width() as u32;
        let h = bitmap.rows() as u32;
        // advance.x is 26.6 fixed-point pixels.
        let advance = (slot.advance().x >> 6) as i32;
        let ox = slot.bitmap_left();
        // FreeType's bitmap_top is the y-distance from baseline UP to
        // the top of the bitmap (positive for ascenders). Convert to
        // our line-top-relative y-down convention.
        let oy = ascent - slot.bitmap_top();

        if w == 0 || h == 0 {
            glyphs.push(GlyphData {
                cp,
                x: 0,
                y: 0,
                w: 0,
                h: 0,
                advance,
                ox,
                oy,
            });
            continue;
        }

        // Unpack FT_PIXEL_MODE_MONO. Each row is `pitch` bytes,
        // packed MSB-first (bit 7 = leftmost pixel). Pitch can include
        // padding bytes past width/8 rounded up.
        let pitch = bitmap.pitch() as i32;
        let buf = bitmap.buffer();
        let abs_pitch = pitch.unsigned_abs() as usize;
        let mut pixels: Vec<bool> = Vec::with_capacity((w * h) as usize);
        for row in 0..h {
            let row_offset = if pitch >= 0 {
                row as usize * abs_pitch
            } else {
                (h - 1 - row) as usize * abs_pitch
            };
            for col in 0..w {
                let byte = buf[row_offset + (col / 8) as usize];
                let bit = byte & (0x80u8 >> (col % 8));
                pixels.push(bit != 0);
            }
        }

        // Some glyphs (space, NBSP) come back with a non-zero
        // width/rows but no lit pixels. Record advance only and don't
        // place a phantom 1×1 cell in the atlas.
        if pixels.iter().all(|&p| !p) {
            glyphs.push(GlyphData {
                cp,
                x: 0,
                y: 0,
                w: 0,
                h: 0,
                advance,
                ox: 0,
                oy: 0,
            });
            continue;
        }

        bitmaps.push(Bitmap { cp, w, h, pixels });
        glyphs.push(GlyphData {
            cp,
            x: 0, // filled in by packer
            y: 0,
            w,
            h,
            advance,
            ox,
            oy,
        });
    }

    if bitmaps.is_empty() {
        return Err("font produced no glyphs".to_string());
    }

    // Shelf-pack tallest first to keep shelves short.
    let mut order: Vec<usize> = (0..bitmaps.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(bitmaps[i].h));
    let mut placements: Vec<(usize, u32, u32)> = Vec::with_capacity(bitmaps.len());
    let mut x = 0u32;
    let mut y = 0u32;
    let mut shelf_h = 0u32;
    let mut atlas_w = 0u32;
    for idx in order {
        let b = &bitmaps[idx];
        if x + b.w > ATLAS_MAX_WIDTH {
            y += shelf_h;
            x = 0;
            shelf_h = 0;
        }
        placements.push((idx, x, y));
        x += b.w;
        if x > atlas_w {
            atlas_w = x;
        }
        if b.h > shelf_h {
            shelf_h = b.h;
        }
    }
    let atlas_h = y + shelf_h;

    let mut atlas: Vec<bool> = vec![false; (atlas_w * atlas_h) as usize];
    let mut cp_to_idx: BTreeMap<u32, usize> = BTreeMap::new();
    for (i, g) in glyphs.iter().enumerate() {
        cp_to_idx.insert(g.cp, i);
    }
    for (idx, px, py) in &placements {
        let b = &bitmaps[*idx];
        let gi = cp_to_idx[&b.cp];
        glyphs[gi].x = *px as i32;
        glyphs[gi].y = *py as i32;
        for row in 0..b.h {
            for col in 0..b.w {
                if b.pixels[(row * b.w + col) as usize] {
                    let dst = ((py + row) * atlas_w + (px + col)) as usize;
                    atlas[dst] = true;
                }
            }
        }
    }

    let png_path = out_path.to_path_buf();
    if let Some(parent) = png_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|e| format!("creating output dir {}: {e}", parent.display()))?;
    }

    let metadata_json =
        build_metadata_json(&png_path, atlas_w, atlas_h, line_height, ascent, &glyphs);
    write_png(&png_path, atlas_w, atlas_h, &atlas, &metadata_json)
        .map_err(|e| format!("writing {}: {e}", png_path.display()))?;

    let png_size = fs::metadata(&png_path).map(|m| m.len()).unwrap_or(0);
    println!(
        "baked {} glyphs from {} at size={}",
        glyphs.len(),
        ttf_path.display(),
        size
    );
    println!(
        "  {} ({atlas_w}x{atlas_h}, {png_size} bytes, metadata in zTXt chunk)",
        png_path.display()
    );
    Ok(())
}

struct Bitmap {
    cp: u32,
    w: u32,
    h: u32,
    pixels: Vec<bool>,
}

struct GlyphData {
    cp: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    advance: i32,
    ox: i32,
    oy: i32,
}

fn iter_codepoints(include_cjk: bool) -> impl Iterator<Item = u32> {
    let mut ranges: Vec<(u32, u32)> = RANGES.to_vec();
    if include_cjk {
        ranges.push(CJK_UNIFIED);
    }
    ranges.into_iter().flat_map(|(lo, hi)| lo..=hi)
}

/// Keyword for the zTXt chunk that holds the glyph metadata JSON.
/// `src/font.rs::FONT_METADATA_KEYWORD` must match.
pub const METADATA_KEYWORD: &str = "usagi-font";

fn write_png(
    path: &Path,
    w: u32,
    h: u32,
    mono: &[bool],
    metadata_json: &str,
) -> std::io::Result<()> {
    let file = fs::File::create(path)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), w, h);
    // 1-bit indexed: palette[0]=transparent, palette[1]=opaque white.
    // tRNS gives index 0 alpha 0, leaving index 1 fully opaque. raylib
    // decodes indexed+tRNS into RGBA via stb_image, so the engine path
    // doesn't change. 32× less raw pixel data than RGBA before deflate.
    encoder.set_color(png::ColorType::Indexed);
    encoder.set_depth(png::BitDepth::One);
    encoder.set_palette(vec![0u8, 0, 0, 255, 255, 255]);
    encoder.set_trns(vec![0u8, 255]);
    encoder.set_compression(png::Compression::Best);
    // Embed glyph metadata as a compressed zTXt chunk. Image viewers
    // ignore unknown text chunks, so the atlas stays inspectable, but
    // the engine can extract this and skip the separate JSON file.
    // Using zTXt (deflate) rather than iTXt because png 0.17's
    // `add_itxt_chunk` writes uncompressed; zTXt is always compressed
    // and the JSON is pure ASCII so Latin-1 isn't a constraint.
    encoder
        .add_ztxt_chunk(METADATA_KEYWORD.to_string(), metadata_json.to_string())
        .map_err(std::io::Error::other)?;
    let mut writer = encoder.write_header()?;

    // Pack 1 bit per pixel, MSB-first within each byte, rows padded to
    // byte boundaries, per PNG spec for indexed bit-depth-1 images.
    let bytes_per_row = w.div_ceil(8) as usize;
    let mut packed = vec![0u8; bytes_per_row * h as usize];
    for y in 0..h {
        for x in 0..w {
            if mono[(y * w + x) as usize] {
                let bit = 7 - (x % 8);
                packed[y as usize * bytes_per_row + (x / 8) as usize] |= 1 << bit;
            }
        }
    }
    writer.write_image_data(&packed)?;
    Ok(())
}

fn build_metadata_json(
    png_path: &Path,
    atlas_w: u32,
    atlas_h: u32,
    line_height: i32,
    ascent: i32,
    glyphs: &[GlyphData],
) -> String {
    let mut s = String::new();
    s.push_str(r#"{"name":""#);
    s.push_str(
        png_path
            .file_stem()
            .and_then(|x| x.to_str())
            .unwrap_or("font"),
    );
    s.push_str(r#"","size":0,"#);
    s.push_str(&format!(r#""line_height":{line_height},"#));
    s.push_str(&format!(r#""ascent":{ascent},"#));
    s.push_str(&format!(r#""atlas_w":{atlas_w},"#));
    s.push_str(&format!(r#""atlas_h":{atlas_h},"#));
    s.push_str(r#""glyphs":{"#);
    let mut sorted: Vec<&GlyphData> = glyphs.iter().collect();
    sorted.sort_by_key(|g| g.cp);
    for (i, g) in sorted.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            r#""{}":{{"w":{},"h":{},"advance":{},"ox":{},"oy":{},"x":{},"y":{}}}"#,
            g.cp, g.w, g.h, g.advance, g.ox, g.oy, g.x, g.y
        ));
    }
    s.push_str("}}");
    s
}
