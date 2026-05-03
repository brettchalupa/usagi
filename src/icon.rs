//! Embedded window icon. Applied to the game window and the
//! `usagi tools` window so the OS task switcher / dock / taskbar
//! shows Usagi's logo by default. Games can override at runtime via
//! `_config().icon = N` (1-based tile index into `sprites.png`,
//! same convention as `gfx.spr`); the same value is consumed at
//! `usagi export --target macos` time to generate the `.app`'s
//! `AppIcon.icns`.

use sola_raylib::prelude::*;

/// Default 16x16 PNG used when the game doesn't set `_config().icon`.
/// Public so the export path can hand the same bytes to
/// `png_to_icns` for the macOS bundle.
pub const ICON_PNG: &[u8] = include_bytes!("../assets/icon.png");

/// Width / height of every Usagi sprite tile and the canonical icon
/// source size. Larger render targets (macOS Dock at 1024x1024) come
/// from nearest-neighbor scaling, preserving the pixel-art look.
const ICON_SIZE: i32 = 16;

/// Decodes the embedded default PNG and hands it to raylib as the
/// current window icon. No-op on emscripten because the browser tab
/// uses an HTML favicon, not an OS window icon.
pub fn apply(rl: &mut RaylibHandle) {
    #[cfg(target_os = "emscripten")]
    let _ = rl;
    #[cfg(not(target_os = "emscripten"))]
    match Image::load_image_from_mem(".png", ICON_PNG) {
        Ok(image) => apply_multires(rl, &image),
        Err(e) => eprintln!("[usagi] failed to decode icon.png: {e}"),
    }
}

/// Slices tile `index` (1-based) out of `sprites_bytes` and applies
/// it as the window icon. Falls back silently to the embedded
/// default if the index is out of range, the sprite sheet can't be
/// decoded, or the sprite sheet isn't a multiple of `ICON_SIZE`
/// wide. No-op on emscripten.
pub fn apply_from_sprites(rl: &mut RaylibHandle, sprites_bytes: &[u8], index: u32) {
    #[cfg(target_os = "emscripten")]
    {
        let _ = (rl, sprites_bytes, index);
        return;
    }
    #[cfg(not(target_os = "emscripten"))]
    {
        let Some(tile) = slice_sprite_tile(sprites_bytes, index) else {
            apply(rl);
            return;
        };
        apply_multires(rl, &tile);
    }
}

/// Sizes shipped to the WM. Linux WMs (KDE ≈32, GNOME ≈48) and Windows
/// taskbars pick the closest match; nearest-neighbor upscales of the
/// 16x16 source keep pixel art crisp at every size.
#[cfg(not(target_os = "emscripten"))]
const ICON_RES_PX: &[u32] = &[16, 32, 48, 64, 128, 256];

/// GLFW copies the pixel data during `SetWindowIcons`, so the scaled
/// buffers can drop on return.
#[cfg(not(target_os = "emscripten"))]
fn apply_multires(rl: &mut RaylibHandle, src: &Image) {
    let src_w = src.width() as u32;
    let src_h = src.height() as u32;
    if src_w == 0 || src_h == 0 {
        return;
    }
    let src_rgba = image_to_rgba(src);
    let mut buffers: Vec<Vec<u8>> = ICON_RES_PX
        .iter()
        .map(|&size| scale_nearest(&src_rgba, src_w, src_h, size, size))
        .collect();
    let mut images: Vec<sola_raylib::ffi::Image> = buffers
        .iter_mut()
        .zip(ICON_RES_PX.iter())
        .map(|(buf, &size)| sola_raylib::ffi::Image {
            data: buf.as_mut_ptr().cast(),
            width: size as i32,
            height: size as i32,
            mipmaps: 1,
            format: sola_raylib::ffi::PixelFormat::PIXELFORMAT_UNCOMPRESSED_R8G8B8A8 as i32,
        })
        .collect();
    rl.set_window_icons(&mut images);
}

/// Loads `sprites_bytes` and extracts the 16x16 region at the
/// 1-based `index` (matches `gfx.spr` semantics: `1` = top-left
/// cell, then row-major). Returns `None` on any failure so the
/// caller can fall back to the default icon.
#[cfg(not(target_os = "emscripten"))]
fn slice_sprite_tile(sprites_bytes: &[u8], index: u32) -> Option<Image> {
    if index < 1 {
        return None;
    }
    let full = Image::load_image_from_mem(".png", sprites_bytes).ok()?;
    let cols = full.width() / ICON_SIZE;
    let rows = full.height() / ICON_SIZE;
    if cols <= 0 || rows <= 0 {
        return None;
    }
    let total = (cols * rows) as u32;
    if index > total {
        return None;
    }
    let zero = (index - 1) as i32;
    let x = (zero % cols) * ICON_SIZE;
    let y = (zero / cols) * ICON_SIZE;
    Some(full.from_image(Rectangle {
        x: x as f32,
        y: y as f32,
        width: ICON_SIZE as f32,
        height: ICON_SIZE as f32,
    }))
}

/// Resolves the icon source for `usagi export --target macos` and
/// packs it into an ICNS byte buffer. Uses the configured tile
/// from the project's `sprites.png` when `config.icon` is set and
/// the tile resolves cleanly; otherwise falls back to the embedded
/// default. Takes a pre-read `Config` so the export path doesn't
/// spin up its own throwaway Lua VM. `script_path` is still needed
/// for the vfs that reads `sprites.png`.
#[cfg(not(target_os = "emscripten"))]
pub fn resolve_icns_for_export(
    config: &crate::config::Config,
    script_path: &std::path::Path,
) -> std::io::Result<Vec<u8>> {
    let source = sprite_icon_source(config, script_path)
        .or_else(|| Image::load_image_from_mem(".png", ICON_PNG).ok())
        .ok_or_else(|| std::io::Error::other("no usable icon source"))?;
    image_to_icns(&source)
}

/// Tries to load the project's `config.icon` tile from
/// `sprites.png`. Returns `None` (no error) when any step doesn't
/// resolve, so the caller can fall through to the default.
#[cfg(not(target_os = "emscripten"))]
fn sprite_icon_source(
    config: &crate::config::Config,
    script_path: &std::path::Path,
) -> Option<Image> {
    let index = config.icon.filter(|&i| i >= 1)?;
    let vfs = crate::vfs::FsBacked::from_script_path(script_path);
    let sprites_bytes = crate::vfs::VirtualFs::read_sprites(&vfs)?;
    slice_sprite_tile(&sprites_bytes, index)
}

/// Encodes a raylib `Image` as a multi-resolution `.icns` container
/// suitable for an `.app`'s `Resources/AppIcon.icns`. Ships entries
/// at 256, 512, and 1024 pixels (Apple's `ic08`, `ic09`, `ic10`
/// slots) via nearest-neighbor scaling so pixel-art stays crisp;
/// smaller Dock sizes are synthesized by macOS from those.
///
/// `get_image_data` normalizes whatever pixel format the source is
/// in down to RGBA8888, so PNGs that decode as RGB or palette-
/// indexed still produce correct icns entries.
#[cfg(not(target_os = "emscripten"))]
fn image_to_icns(src: &Image) -> std::io::Result<Vec<u8>> {
    let src_w = src.width() as u32;
    let src_h = src.height() as u32;
    if src_w == 0 || src_h == 0 {
        return Err(std::io::Error::other("icon image has zero dimension"));
    }
    let src_rgba = image_to_rgba(src);

    let mut family = icns::IconFamily::new();
    for size in [256u32, 512, 1024] {
        let scaled = scale_nearest(&src_rgba, src_w, src_h, size, size);
        let img = icns::Image::from_data(icns::PixelFormat::RGBA, size, size, scaled)
            .map_err(|e| std::io::Error::other(format!("icns image: {e}")))?;
        family
            .add_icon(&img)
            .map_err(|e| std::io::Error::other(format!("icns add_icon: {e}")))?;
    }
    let mut out = Vec::new();
    family.write(&mut out)?;
    Ok(out)
}

/// Flattens a raylib `Image` into a `Vec<u8>` in RGBA8888 layout.
/// `get_image_data` does the format conversion internally; we
/// just need a contiguous byte buffer for `icns::Image::from_data`.
#[cfg(not(target_os = "emscripten"))]
fn image_to_rgba(img: &Image) -> Vec<u8> {
    let pixels = img.get_image_data();
    let mut out = Vec::with_capacity(pixels.len() * 4);
    for c in pixels.iter() {
        out.push(c.r);
        out.push(c.g);
        out.push(c.b);
        out.push(c.a);
    }
    out
}

/// Nearest-neighbor scale of a packed RGBA byte buffer. Integer
/// math so the destination grid samples line up exactly on the
/// source grid for pixel-art-friendly upscaling.
#[cfg(not(target_os = "emscripten"))]
fn scale_nearest(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    let mut dst = vec![0u8; (dst_w * dst_h * 4) as usize];
    for y in 0..dst_h {
        let sy = (y * src_h) / dst_h;
        for x in 0..dst_w {
            let sx = (x * src_w) / dst_w;
            let s = ((sy * src_w + sx) * 4) as usize;
            let d = ((y * dst_w + x) * 4) as usize;
            dst[d..d + 4].copy_from_slice(&src[s..s + 4]);
        }
    }
    dst
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_nearest_doubles_a_2x2_to_4x4() {
        // r g
        // b w
        #[rustfmt::skip]
        let src: Vec<u8> = vec![
            255, 0, 0, 255,   0, 255, 0, 255,
            0, 0, 255, 255,   255, 255, 255, 255,
        ];
        let dst = scale_nearest(&src, 2, 2, 4, 4);
        assert_eq!(dst.len(), 4 * 4 * 4);
        // Top-left quadrant should be all red.
        for y in 0..2 {
            for x in 0..2 {
                let i = (y * 4 + x) * 4;
                assert_eq!(&dst[i..i + 4], &[255, 0, 0, 255]);
            }
        }
        // Bottom-right should be white.
        for y in 2..4 {
            for x in 2..4 {
                let i = (y * 4 + x) * 4;
                assert_eq!(&dst[i..i + 4], &[255, 255, 255, 255]);
            }
        }
    }

    #[test]
    fn image_to_icns_returns_a_well_formed_container() {
        let src = Image::load_image_from_mem(".png", ICON_PNG).expect("decode embedded icon");
        let icns_bytes = image_to_icns(&src).expect("encode icns");
        // ICNS magic: "icns" header bytes.
        assert!(icns_bytes.len() > 8, "icns output too small");
        assert_eq!(&icns_bytes[..4], b"icns", "missing icns magic header");
        // Round-trip through the icns crate to confirm the entries are valid.
        let family =
            icns::IconFamily::read(std::io::Cursor::new(&icns_bytes)).expect("re-read icns");
        let icons = family.available_icons();
        assert!(
            icons.len() >= 3,
            "expected at least 3 icon entries, got {}",
            icons.len()
        );
    }
}
