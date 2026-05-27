//! Usagi tools window. Hosts the shell (fixed 1280x720 window, tab bar,
//! shared toast, asset loading + live reload); individual tools live in
//! sibling modules and expose a small `State` + `handle_input` + `draw`
//! API.

mod color_palette;
mod jukebox;
mod save_inspector;
pub(super) mod theme;
mod tilepicker;

use crate::assets::{MusicLibrary, SfxLibrary, SpriteSheet};
use crate::vfs::{FsBacked, VirtualFs};
use sola_raylib::prelude::*;
use std::path::{Path, PathBuf};

/// Tools UI is drawn at fixed canvas dimensions into a render texture and
/// scaled (with letterboxing) to whatever size the user resizes the window
/// to. `.highdpi()` is intentionally off; the per-frame letterbox blit
/// handles scaling across DPIs uniformly.
pub(super) const CANVAS_W: f32 = 1280.;
pub(super) const CANVAS_H: f32 = 720.;

/// Shared panel geometry. Each tool draws into this panel.
pub(super) const PANEL_X: f32 = 20.;
pub(super) const PANEL_Y: f32 = 70.;
pub(super) const PANEL_W: f32 = CANVAS_W - 2.0 * PANEL_X;
pub(super) const PANEL_H: f32 = CANVAS_H - PANEL_Y - 20.0;
pub(super) const HINT_Y: f32 = PANEL_Y + PANEL_H - 24.0;

const TOAST_SECS: f32 = 2.5;

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Jukebox,
    TilePicker,
    SaveInspector,
    ColorPalette,
}

pub(super) struct Toast {
    pub timer: f32,
    pub message: String,
}

impl Toast {
    pub fn new(message: String) -> Self {
        Self {
            timer: TOAST_SECS,
            message,
        }
    }
}

struct State {
    active: Tool,
    jukebox: jukebox::State,
    tilepicker: tilepicker::State,
    save_inspector: save_inspector::State,
    color_palette: color_palette::State,
    toast: Option<Toast>,
}

pub fn run(project_path: Option<&str>) -> crate::Result<()> {
    let project_dir = project_path.and_then(resolve_project_dir);
    let vfs = project_dir
        .as_ref()
        .map(|d| FsBacked::from_project_dir(d.clone()));
    // Read the project's `_config()` once at startup so tools that
    // depend on its values (currently the tilepicker's grid size)
    // pick up overrides like `sprite_size`. Falls back to defaults
    // when there's no project path.
    let project_config = match project_dir.as_ref() {
        Some(dir) => crate::config::Config::read_for_export(&dir.join("main.lua")),
        None => crate::config::Config::default(),
    };
    let sfx_dir_display = project_dir.as_ref().map(|d| d.join("sfx"));
    let music_dir_display = project_dir.as_ref().map(|d| d.join("music"));
    let sprites_path_display = project_dir.as_ref().map(|d| d.join("sprites.png"));
    // The tools UI uses its own dark theme (see `theme.rs`) for chrome,
    // independent of whatever palette the project ships. The
    // ColorPalette tool still reads `palette.png` itself (see
    // `color_palette::State::new`) to show the user's custom palette
    // in its swatches.

    // Same log-level handling as the game session: raylib defaults
    // to LOG_INFO and floods the terminal with GLFW/GL/audio init
    // chatter. Drop to LOG_WARNING so real failures still surface.
    // `USAGI_RAYLIB_VERBOSE=1` brings the full raylib log back.
    let log_level = if std::env::var_os("USAGI_RAYLIB_VERBOSE").is_some() {
        TraceLogLevel::LOG_INFO
    } else {
        TraceLogLevel::LOG_WARNING
    };
    let (mut rl, thread) = sola_raylib::init()
        .size(CANVAS_W as i32, CANVAS_H as i32)
        .title("Usagi Tools")
        .log_level(log_level)
        .vsync()
        .resizable()
        .build();
    crate::icon::apply(&mut rl);
    rl.set_target_fps(60);

    // Without highdpi the window opens at the literal canvas size, which
    // is tiny on a 4K display. Scale up to roughly 85% of the current
    // monitor, capped at 2x the canvas (so even huge monitors don't open
    // an oversized window). Preserves canvas aspect so the letterbox
    // bars are minimal until the user resizes.
    fit_initial_window(&mut rl);
    rl.set_window_min_size(CANVAS_W as i32 / 2, CANVAS_H as i32 / 2);

    // Render target the whole tools UI draws into. Blit-scaled to the
    // window each frame; mouse coordinates are remapped via
    // `set_mouse_offset` / `set_mouse_scale` so raygui and direct reads
    // both land in canvas space without any per-call transform.
    let mut canvas = rl
        .load_render_texture(&thread, CANVAS_W as u32, CANVAS_H as u32)
        .map_err(|e| crate::Error::Cli(format!("creating tools canvas: {e}")))?;

    let audio = RaylibAudio::init_audio_device()
        .map_err(|e| crate::msg::err!("audio init failed: {}", e))
        .ok();

    let mut sfx = match (&audio, &vfs) {
        (Some(a), Some(v)) => SfxLibrary::load(a, v),
        _ => SfxLibrary::empty(),
    };
    let mut music_lib: MusicLibrary<'_> = match (&audio, &vfs) {
        (Some(a), Some(v)) => MusicLibrary::load(a, v),
        _ => MusicLibrary::empty(),
    };

    let mut sprites = vfs.as_ref().map(|v| SpriteSheet::load(&mut rl, &thread, v));
    let font = crate::font::load_bundled(&mut rl, &thread);

    // Make raygui draw with monogram instead of raylib's built-in font.
    // TEXT_SIZE = 2 * baseSize keeps the pixel-art glyphs on integer
    // scale; TEXT_SPACING = 0 matches the engine's draw_text_ex calls.
    rl.gui_set_font(&font);
    rl.gui_set_style(
        GuiControl::DEFAULT,
        GuiDefaultProperty::TEXT_SIZE,
        crate::font::MONOGRAM_SIZE * 2,
    );
    rl.gui_set_style(GuiControl::DEFAULT, GuiDefaultProperty::TEXT_SPACING, 0);
    apply_theme(&mut rl);

    let mut state = State {
        active: Tool::Jukebox,
        jukebox: jukebox::State::new(&sfx, music_lib.track_names()),
        tilepicker: tilepicker::State::new(project_config.sprite_size),
        save_inspector: save_inspector::State::new(project_path),
        color_palette: color_palette::State::new(
            vfs.as_ref().map(|v| v as &dyn crate::vfs::VirtualFs),
        ),
        toast: None,
    };

    // Track palette.png mtime so the ColorPalette tool hot-reloads
    // when the user edits / drops in / removes the file mid-session.
    let mut palette_mtime = vfs.as_ref().and_then(|v| v.palette_mtime());

    while !rl.window_should_close() {
        let dt = rl.get_frame_time();

        // Remap mouse coords into canvas space so raygui and every
        // get_mouse_position() call sees canvas pixels regardless of
        // window size. Recompute every frame; cheap and avoids a
        // stale-on-resize bug.
        let dst = letterbox_rect(rl.get_screen_width(), rl.get_screen_height());
        rl.set_mouse_offset(Vector2::new(-dst.x, -dst.y));
        rl.set_mouse_scale(CANVAS_W / dst.width, CANVAS_H / dst.height);

        if let Some(toast) = &mut state.toast {
            toast.timer -= dt;
            if toast.timer <= 0.0 {
                state.toast = None;
            }
        }

        if let (Some(a), Some(v)) = (&audio, &vfs)
            && sfx.reload_if_changed(a, v)
        {
            state.jukebox.refresh_names(&sfx);
            crate::msg::info!("jukebox reloaded sfx ({} sound(s))", sfx.len());
        }

        if let (Some(a), Some(v)) = (&audio, &vfs)
            && music_lib.reload_if_changed(a, v)
        {
            state.jukebox.refresh_music_names(music_lib.track_names());
            crate::msg::info!("jukebox reloaded music ({} track(s))", music_lib.len());
        }
        // raylib's music streams need an update each frame to refill the
        // audio buffer, even when the jukebox tab isn't active.
        music_lib.update();

        if let (Some(sheet), Some(v)) = (sprites.as_mut(), vfs.as_ref())
            && sheet.reload_if_changed(&mut rl, &thread, v)
        {
            crate::msg::info!("tools reloaded sprites.png");
        }

        if let Some(v) = vfs.as_ref() {
            let cur = v.palette_mtime();
            if cur != palette_mtime {
                palette_mtime = cur;
                state.color_palette.reload(Some(v));
                crate::msg::info!("tools reloaded palette.png");
            }
        }

        // Global tab shortcuts. Applied before per-tool input so switching
        // takes effect on the same frame.
        if rl.is_key_pressed(KeyboardKey::KEY_ONE) {
            state.active = Tool::Jukebox;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_TWO) {
            state.active = Tool::TilePicker;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_THREE) {
            state.active = Tool::SaveInspector;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_FOUR) {
            state.active = Tool::ColorPalette;
        }

        let tex = sprites.as_ref().and_then(|s| s.texture());
        match state.active {
            Tool::Jukebox => jukebox::handle_input(&rl, &mut state.jukebox, &sfx, &mut music_lib),
            Tool::TilePicker => {
                if let Some(msg) = tilepicker::handle_input(&mut rl, &mut state.tilepicker, tex, dt)
                {
                    state.toast = Some(Toast::new(msg));
                }
            }
            Tool::SaveInspector => {
                if let Some(msg) = save_inspector::handle_input(&rl, &mut state.save_inspector) {
                    state.toast = Some(Toast::new(msg));
                }
            }
            Tool::ColorPalette => {
                if let Some(msg) = color_palette::handle_input(&mut rl, &mut state.color_palette) {
                    state.toast = Some(Toast::new(msg));
                }
            }
        }

        {
            let mut d = rl.begin_drawing(&thread);
            // Same color as the canvas BG so the letterbox bars blend
            // seamlessly with the tool area at non-16:9 window shapes.
            d.clear_background(theme::BG);

            {
                let mut d_rt = d.begin_texture_mode(&thread, &mut canvas);
                d_rt.clear_background(theme::BG);

                if tab_button(
                    &mut d_rt,
                    Rectangle::new(20., 20., 170., 36.),
                    "Jukebox [1]",
                    state.active == Tool::Jukebox,
                ) {
                    state.active = Tool::Jukebox;
                }
                if tab_button(
                    &mut d_rt,
                    Rectangle::new(200., 20., 210., 36.),
                    "TilePicker [2]",
                    state.active == Tool::TilePicker,
                ) {
                    state.active = Tool::TilePicker;
                }
                if tab_button(
                    &mut d_rt,
                    Rectangle::new(420., 20., 250., 36.),
                    "SaveInspector [3]",
                    state.active == Tool::SaveInspector,
                ) {
                    state.active = Tool::SaveInspector;
                }
                if tab_button(
                    &mut d_rt,
                    Rectangle::new(680., 20., 230., 36.),
                    "ColorPalette [4]",
                    state.active == Tool::ColorPalette,
                ) {
                    state.active = Tool::ColorPalette;
                }

                match state.active {
                    Tool::Jukebox => jukebox::draw(
                        &mut d_rt,
                        &font,
                        &mut state.jukebox,
                        &sfx,
                        &mut music_lib,
                        project_path,
                        sfx_dir_display.as_deref(),
                        music_dir_display.as_deref(),
                    ),
                    Tool::TilePicker => tilepicker::draw(
                        &mut d_rt,
                        &font,
                        &state.tilepicker,
                        tex,
                        sprites_path_display.as_deref(),
                    ),
                    Tool::SaveInspector => {
                        if let Some(msg) = save_inspector::draw(
                            &mut d_rt,
                            &font,
                            &mut state.save_inspector,
                            project_path,
                        ) {
                            state.toast = Some(Toast::new(msg));
                        }
                    }
                    Tool::ColorPalette => {
                        color_palette::draw(&mut d_rt, &font, &state.color_palette);
                    }
                }

                if let Some(toast) = &state.toast {
                    draw_toast(&mut d_rt, &font, &toast.message);
                }
            }
            // d_rt drops here, EndTextureMode called.
            // Now blit the canvas to the window with letterboxing.
            // Source rect has negative height because raylib render
            // textures are y-flipped (OpenGL convention).
            d.draw_texture_pro(
                canvas.texture(),
                Rectangle::new(0., 0., CANVAS_W, -CANVAS_H),
                dst,
                Vector2::zero(),
                0.0,
                Color::WHITE,
            );
        }

        // Auto-play on selection change (covers mouse click into the
        // list_view which we can't intercept until after the draw returns).
        if state.active == Tool::Jukebox {
            jukebox::auto_play(&mut state.jukebox, &sfx);
        }
    }

    Ok(())
}

/// Computes the largest canvas-aspect rectangle that fits inside the
/// given window dimensions, centered. Used for both the per-frame blit
/// and the mouse-coordinate remap.
fn letterbox_rect(win_w: i32, win_h: i32) -> Rectangle {
    let win_w = win_w.max(1) as f32;
    let win_h = win_h.max(1) as f32;
    let aspect = CANVAS_W / CANVAS_H;
    let (dw, dh) = if win_w / win_h > aspect {
        (win_h * aspect, win_h)
    } else {
        (win_w, win_w / aspect)
    };
    Rectangle::new((win_w - dw) * 0.5, (win_h - dh) * 0.5, dw, dh)
}

/// Resizes the launch window to the largest integer multiple of the
/// canvas that fits ~85% of the current monitor, then centers it.
/// Integer scaling matters: the canvas is blit-scaled to the window
/// each frame, and a fractional ratio bilinear-bleeds high-contrast
/// edges (visible under color-palette swatches against their labels).
/// At 1x / 2x / 3x the bleed disappears. Resizing after launch is
/// allowed to land on a fractional scale; that's the user's choice.
/// No-op if the monitor query returns garbage.
fn fit_initial_window(rl: &mut RaylibHandle) {
    let monitor = sola_raylib::window::get_current_monitor();
    let mw = sola_raylib::window::get_monitor_width(monitor);
    let mh = sola_raylib::window::get_monitor_height(monitor);
    if mw <= 0 || mh <= 0 {
        return;
    }
    let max_w = (mw as f32 * 0.85) as i32;
    let max_h = (mh as f32 * 0.85) as i32;
    let scale_w = max_w / CANVAS_W as i32;
    let scale_h = max_h / CANVAS_H as i32;
    let scale = scale_w.min(scale_h).max(1);
    let w = CANVAS_W as i32 * scale;
    let h = CANVAS_H as i32 * scale;
    rl.set_window_size(w, h);

    // Center on the current monitor. `get_monitor_position` returns the
    // monitor's top-left in the virtual desktop; offset by half of
    // (monitor - window) to center.
    let pos = sola_raylib::window::get_monitor_position(monitor);
    let cx = pos.x as i32 + (mw - w) / 2;
    let cy = pos.y as i32 + (mh - h) / 2;
    rl.set_window_position(cx, cy);
}

/// Resolves the `usagi tools <path>` arg to a project directory:
///   - a directory is used directly
///   - anything that resolves via `cli::resolve_script_path` uses its parent dir
///   - otherwise None (tools open with no project loaded)
fn resolve_project_dir(path: &str) -> Option<PathBuf> {
    let p = Path::new(path);
    if p.is_dir() {
        return Some(p.to_path_buf());
    }
    let script = crate::cli::resolve_script_path(path).ok()?;
    Path::new(&script)
        .parent()
        .map(|parent| parent.to_path_buf())
}

/// Dark theme for the tools window. Reads colors from `theme.rs` so
/// the per-tool draw code (text labels, hint lines, selection boxes)
/// stays visually aligned with the raygui styling done here.
///
/// `DEFAULT` props (indices 0..=14) propagate to every control
/// automatically. Extended props like `LINE_COLOR` / `BACKGROUND_COLOR`
/// only affect controls that look them up explicitly (e.g. `GuiPanel`
/// uses `LINE_COLOR` for its border and `BACKGROUND_COLOR` for its
/// body).
fn apply_theme(rl: &mut RaylibHandle) {
    use GuiControlProperty as P;
    use GuiDefaultProperty as D;

    let c = |color: Color| color.color_to_int();

    // Normal: surface base with the primary text color.
    rl.gui_set_style(
        GuiControl::DEFAULT,
        P::BORDER_COLOR_NORMAL,
        c(theme::BORDER),
    );
    rl.gui_set_style(GuiControl::DEFAULT, P::BASE_COLOR_NORMAL, c(theme::SURFACE));
    rl.gui_set_style(GuiControl::DEFAULT, P::TEXT_COLOR_NORMAL, c(theme::TEXT));
    // Focused: accent border highlights the hovered control while the
    // base stays subdued so the focus reads at a glance.
    rl.gui_set_style(
        GuiControl::DEFAULT,
        P::BORDER_COLOR_FOCUSED,
        c(theme::ACCENT),
    );
    rl.gui_set_style(
        GuiControl::DEFAULT,
        P::BASE_COLOR_FOCUSED,
        c(theme::SURFACE),
    );
    rl.gui_set_style(GuiControl::DEFAULT, P::TEXT_COLOR_FOCUSED, c(theme::TEXT));
    // Pressed: full accent fill so a depressed button is unmistakable.
    rl.gui_set_style(
        GuiControl::DEFAULT,
        P::BORDER_COLOR_PRESSED,
        c(theme::ACCENT),
    );
    rl.gui_set_style(GuiControl::DEFAULT, P::BASE_COLOR_PRESSED, c(theme::ACCENT));
    rl.gui_set_style(
        GuiControl::DEFAULT,
        P::TEXT_COLOR_PRESSED,
        c(theme::ON_ACCENT),
    );
    // Disabled: muted text on the same surface.
    rl.gui_set_style(
        GuiControl::DEFAULT,
        P::BORDER_COLOR_DISABLED,
        c(theme::BORDER),
    );
    rl.gui_set_style(
        GuiControl::DEFAULT,
        P::BASE_COLOR_DISABLED,
        c(theme::SURFACE),
    );
    rl.gui_set_style(
        GuiControl::DEFAULT,
        P::TEXT_COLOR_DISABLED,
        c(theme::TEXT_MUTED),
    );
    rl.gui_set_style(GuiControl::DEFAULT, P::BORDER_WIDTH, 1);

    // Tool panels (gui_panel) read BACKGROUND_COLOR for their body and
    // LINE_COLOR for their border.
    rl.gui_set_style(GuiControl::DEFAULT, D::BACKGROUND_COLOR, c(theme::SURFACE));
    rl.gui_set_style(GuiControl::DEFAULT, D::LINE_COLOR, c(theme::BORDER));

    // Panel header strip (raygui draws it as a STATUSBAR). SURFACE_HIGH
    // is just one notch lighter than the panel body so the header reads
    // as a title bar without needing a different hue.
    rl.gui_set_style(
        GuiControl::STATUSBAR,
        P::BORDER_COLOR_NORMAL,
        c(theme::BORDER),
    );
    rl.gui_set_style(
        GuiControl::STATUSBAR,
        P::BASE_COLOR_NORMAL,
        c(theme::SURFACE_HIGH),
    );
    rl.gui_set_style(GuiControl::STATUSBAR, P::TEXT_COLOR_NORMAL, c(theme::TEXT));
}

/// Tab-bar button. When `active`, swaps the NORMAL/FOCUSED color slots
/// to the PRESSED palette for the duration of this draw so the active
/// tab consistently reads as depressed regardless of mouse hover.
fn tab_button(d: &mut RaylibDrawHandle, rect: Rectangle, label: &str, active: bool) -> bool {
    use GuiControlProperty as P;

    if !active {
        return d.gui_button(rect, label);
    }

    let stash = [
        P::BASE_COLOR_NORMAL,
        P::BORDER_COLOR_NORMAL,
        P::TEXT_COLOR_NORMAL,
        P::BASE_COLOR_FOCUSED,
        P::BORDER_COLOR_FOCUSED,
        P::TEXT_COLOR_FOCUSED,
    ]
    .map(|p| (p, d.gui_get_style(GuiControl::BUTTON, p)));
    let pressed_base = d.gui_get_style(GuiControl::BUTTON, P::BASE_COLOR_PRESSED);
    let pressed_border = d.gui_get_style(GuiControl::BUTTON, P::BORDER_COLOR_PRESSED);
    let pressed_text = d.gui_get_style(GuiControl::BUTTON, P::TEXT_COLOR_PRESSED);
    for p in [P::BASE_COLOR_NORMAL, P::BASE_COLOR_FOCUSED] {
        d.gui_set_style(GuiControl::BUTTON, p, pressed_base);
    }
    for p in [P::BORDER_COLOR_NORMAL, P::BORDER_COLOR_FOCUSED] {
        d.gui_set_style(GuiControl::BUTTON, p, pressed_border);
    }
    for p in [P::TEXT_COLOR_NORMAL, P::TEXT_COLOR_FOCUSED] {
        d.gui_set_style(GuiControl::BUTTON, p, pressed_text);
    }

    let clicked = d.gui_button(rect, label);

    for (p, v) in stash {
        d.gui_set_style(GuiControl::BUTTON, p, v);
    }
    clicked
}

fn draw_toast(d: &mut RaylibDrawHandle, font: &Font, message: &str) {
    // Match the body text size used by the rest of the tools (raygui's
    // TEXT_SIZE = 2 * MONOGRAM_SIZE). The original 1x size came out way
    // smaller than the surrounding labels, which made the toast read as
    // a tooltip rather than a confirmation.
    let text_size = (crate::font::MONOGRAM_SIZE * 2) as f32;
    let w = 420.0;
    let h = 56.0;
    let x = CANVAS_W - w - 20.0;
    let y = CANVAS_H - h - 20.0;
    d.gui_panel(Rectangle::new(x, y, w, h), "");
    d.draw_text_ex(
        font,
        message,
        Vector2::new(x + 14.0, y + (h - text_size) * 0.5),
        text_size,
        0.0,
        theme::TEXT,
    );
}
