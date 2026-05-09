// Emscripten library: toggles browser fullscreen for the canvas.
//
// Linked at build time via `--js-library web/usagi_fullscreen.js` (set
// in `.cargo/config.toml`). The C-side declaration lives in
// `src/session.rs`'s emscripten cfg block.
//
// raylib's `ToggleBorderlessWindowed` / `ToggleFullscreen` are
// "desktop-only" (they go through GLFW's fullscreen path which the
// emscripten port doesn't wire to the browser API), so on web we
// reach the real Fullscreen API directly. The browser requires the
// call to happen inside a user-gesture call stack — that's why
// pause-menu BTN1 / Alt+Enter work but a delayed timer wouldn't.
//
// State is owned by the DOM (`document.fullscreenElement`); we don't
// track it ourselves. That keeps us correct when the user exits via
// the browser's Esc.

mergeInto(LibraryManager.library, {
  usagi_fullscreen_toggle: function () {
    try {
      if (document.fullscreenElement) {
        document.exitFullscreen();
      } else {
        var c = Module.canvas || document.getElementById("canvas");
        if (c && c.requestFullscreen) {
          c.requestFullscreen();
        }
      }
    } catch (e) {
      console.error("[usagi] fullscreen toggle failed:", e);
    }
  },
});
