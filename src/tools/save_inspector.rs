//! Save Inspector tool: shows the JSON content of the loaded project's
//! save file, with buttons to refresh, clear, and open the containing
//! directory in the OS file manager.
//!
//! The tools window doesn't run the user's `_update` / `_draw`, but we
//! do need `_config().game_id` so we can find the save file. Approach:
//! one-shot Lua eval at startup that loads the project script, calls
//! `_config()`, and pulls `game_id` out. The eval is sandboxed (no
//! window, no audio); top-level code in `main.lua` typically just
//! defines functions, so it's safe to execute.
//!
//! This tool only inspects native saves. Web saves live in
//! `localStorage` inside the browser running the wasm build, which is
//! reachable via DevTools, not from a native binary.
//!
//! No live polling: the file is re-read on first frame and whenever
//! the user hits Refresh. We could mtime-poll like the sfx hot-reload
//! path, but inspection is cheap and explicit feels better here.

use super::{HINT_Y, PANEL_H, PANEL_W, PANEL_X, PANEL_Y};
use crate::{game_id::GameId, tools::theme};
use sola_raylib::prelude::*;
use std::path::PathBuf;

pub(super) struct State {
    /// `game_id` from `_config()`. None when the project doesn't set
    /// one, when the script can't be loaded, or when the tools window
    /// was opened with no project. All three cases render the same
    /// "no game_id" message.
    pub game_id: Option<String>,
    /// Resolved save file path. None when `game_id` is None.
    pub path: Option<PathBuf>,
    /// JSON content as it currently lives on disk. None means "no save
    /// yet" (file missing). `Some(s)` is the literal file bytes; we
    /// don't reformat since the engine writes pretty-printed already.
    pub content: Option<String>,
    /// Set when something went wrong loading the project's `_config()`
    /// or reading the save file. Rendered above the JSON area.
    pub error: Option<String>,
}

impl State {
    /// `project_path` is the same arg `usagi tools` was invoked with;
    /// we re-resolve the script path here (rather than reusing the
    /// shell's project-dir vfs) because the shell's vfs is built with
    /// `from_project_dir`, which leaves `script_filename` unset, and
    /// `load_script` needs that filename to read `main.lua`.
    pub fn new(game_id: Option<String>) -> Self {
        let mut s = Self {
            game_id,
            path: None,
            content: None,
            error: None,
        };
        if let Some(_id) = &s.game_id {
            s.refresh()
        }
        s
    }

    /// Reads the save file from disk and stashes it on the state.
    /// Captures any read error in `self.error` rather than returning
    /// it: the inspector keeps rendering with whatever it last knew
    /// about, plus the error message above it.
    pub fn refresh(&mut self) {
        let Some(id) = self.game_id.as_ref() else {
            return;
        };
        let Some(game_id) = GameId::try_from_explicit(id) else {
            return;
        };
        match crate::save::save_path(&game_id) {
            Ok(p) => self.path = Some(p),
            Err(e) => {
                self.error = Some(format!("save_path: {e}"));
                return;
            }
        }
        match crate::save::read_save(&game_id) {
            Ok(s) => {
                self.content = s;
                self.error = None;
            }
            Err(e) => self.error = Some(format!("read: {e}")),
        }
    }

    pub fn clear(&mut self) -> Result<(), String> {
        let Some(id) = self.game_id.as_ref() else {
            return Err("no game_id".into());
        };
        let Some(game_id) = GameId::try_from_explicit(id) else {
            return Err("game_id invalid format".into());
        };
        crate::save::clear_save(&game_id).map_err(|e| format!("clear: {e}"))?;
        self.content = None;
        self.error = None;
        Ok(())
    }
}

/// Spawns the OS file manager to show the directory containing the
/// save file. Best-effort: if the spawn fails (no DE installed, etc.)
/// we surface the error to the toast layer rather than panicking.
fn open_in_file_manager(path: &std::path::Path) -> std::io::Result<()> {
    // Open the parent dir, since some platforms don't accept a file
    // path here and others would open the JSON in a text editor.
    let dir = path.parent().unwrap_or(path);
    #[cfg(target_os = "linux")]
    let cmd = "xdg-open";
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(target_os = "windows")]
    let cmd = "explorer";
    std::process::Command::new(cmd).arg(dir).spawn().map(|_| ())
}

/// Returns Some(toast message) when an action ran. Tools shell hoists
/// it into the shared toast slot.
pub(super) fn handle_input(rl: &RaylibHandle, state: &mut State) -> Option<String> {
    if rl.is_key_pressed(KeyboardKey::KEY_R) {
        state.refresh();
        return Some("Refreshed.".into());
    }
    if rl.is_key_pressed(KeyboardKey::KEY_F)
        && let Some(p) = state.path.as_deref()
    {
        return match open_in_file_manager(p) {
            Ok(()) => Some("Opened in file manager.".into()),
            Err(e) => Some(format!("Open failed: {e}")),
        };
    }
    None
}

pub(super) fn draw(
    d: &mut RaylibDrawHandle,
    font: &Font,
    state: &mut State,
    project_path: Option<&str>,
) -> Option<String> {
    const SMALL: f32 = (crate::font::MONOGRAM_SIZE * 2) as f32;

    d.gui_panel(
        Rectangle::new(PANEL_X, PANEL_Y, PANEL_W, PANEL_H),
        "Save Inspector",
    );

    let mut y = PANEL_Y + 30.0;

    let project_line = match project_path {
        Some(p) => format!("project: {}", p),
        None => "no project. Run `usagi tools path/to/project`.".into(),
    };
    d.draw_text_ex(
        font,
        &project_line,
        Vector2::new(30.0, y),
        SMALL,
        0.0,
        theme::TEXT,
    );
    y += 24.0;

    let game_id_line = match state.game_id.as_deref() {
        Some(id) => format!("game_id: {}", id),
        None => "game_id: (not set; add `game_id` to _config())".into(),
    };
    d.draw_text_ex(
        font,
        &game_id_line,
        Vector2::new(30.0, y),
        SMALL,
        0.0,
        theme::TEXT_DIM,
    );
    y += 24.0;

    if let Some(p) = state.path.as_deref() {
        d.draw_text_ex(
            font,
            &format!("path: {}", p.display()),
            Vector2::new(30.0, y),
            SMALL,
            0.0,
            theme::TEXT_DIM,
        );
        y += 24.0;
    }

    let mut toast: Option<String> = None;

    let have_id = state.game_id.is_some();
    let have_save = state.content.is_some();

    // Action row. raylib_rs's GuiButton doesn't expose a "disabled"
    // mode, so we just no-op when there's nothing to act on.
    let btn_y = y + 8.0;
    if d.gui_button(Rectangle::new(30.0, btn_y, 180.0, 40.0), "Refresh [R]") && have_id {
        state.refresh();
        toast = Some("Refreshed.".into());
    }
    if d.gui_button(Rectangle::new(220.0, btn_y, 140.0, 40.0), "Clear") && have_save {
        match state.clear() {
            Ok(()) => toast = Some("Save cleared.".into()),
            Err(msg) => toast = Some(format!("Clear failed: {msg}")),
        }
    }
    if d.gui_button(Rectangle::new(370.0, btn_y, 240.0, 40.0), "Open Folder [F]")
        && let Some(p) = state.path.as_deref()
    {
        match open_in_file_manager(p) {
            Ok(()) => toast = Some("Opened in file manager.".into()),
            Err(e) => toast = Some(format!("Open failed: {e}")),
        }
    }
    y = btn_y + 56.0;

    if let Some(err) = &state.error {
        d.draw_text_ex(font, err, Vector2::new(30.0, y), SMALL, 0.0, theme::DANGER);
        y += 24.0;
    }

    // JSON pane background, then line-by-line render. The engine
    // writes pretty-printed already, so we don't reformat.
    let pane_x = 30.0;
    let pane_y = y;
    let pane_w = PANEL_W - 20.0;
    let pane_h = HINT_Y - pane_y - 16.0;
    d.gui_panel(Rectangle::new(pane_x, pane_y, pane_w, pane_h), "save.json");

    let placeholder = match (have_id, have_save) {
        (false, _) => Some("(set game_id in _config() to inspect saves)"),
        (true, false) => Some("(no save yet; run the game and save once)"),
        _ => None,
    };
    if let Some(msg) = placeholder {
        d.draw_text_ex(
            font,
            msg,
            Vector2::new(pane_x + 14.0, pane_y + 30.0),
            SMALL,
            0.0,
            theme::TEXT_MUTED,
        );
    } else if let Some(content) = state.content.as_deref() {
        let mut line_y = pane_y + 26.0;
        for line in content.lines() {
            if line_y + SMALL > pane_y + pane_h - 8.0 {
                break; // overflow; leave a hint after the loop
            }
            d.draw_text_ex(
                font,
                line,
                Vector2::new(pane_x + 14.0, line_y),
                SMALL,
                0.0,
                theme::TEXT,
            );
            line_y += 22.0;
        }
        let total_lines = content.lines().count();
        let drawn = ((pane_h - 34.0) / 22.0) as usize;
        if total_lines > drawn {
            d.draw_text_ex(
                font,
                &format!("... ({} more line(s) clipped)", total_lines - drawn),
                Vector2::new(pane_x + 14.0, pane_y + pane_h - 22.0),
                SMALL,
                0.0,
                theme::TEXT_MUTED,
            );
        }
    }

    d.draw_text_ex(
        font,
        "R: refresh   F: reveal containing folder   Clear: delete save file",
        Vector2::new(30.0, HINT_Y),
        SMALL,
        0.0,
        theme::TEXT_MUTED,
    );

    toast
}
