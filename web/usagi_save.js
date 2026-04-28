// Emscripten library: backs Usagi's save/load API with `localStorage`.
//
// Linked at build time via `--js-library web/usagi_save.js` (set in the
// justfile recipes). The C-side declarations live in `src/save.rs`'s
// emscripten cfg block.
//
// Memory ownership:
// - `usagi_save_read` returns a pointer to a freshly malloc'd C string
//   the Rust caller must free via `usagi_save_free`. We can't return
//   the JS string directly because Rust's `CStr::from_ptr` reads from
//   wasm memory until the trailing NUL.
// - `usagi_save_free` calls `_free` (libc free, which `_malloc` here
//   pairs with). Routing free through the JS side keeps the pairing
//   symmetric and obvious.
//
// Failures (quota exceeded, localStorage disabled in private mode,
// etc.) are logged to console but don't surface as Rust errors —
// localStorage can fail per-browser-tab in ways the game code can't
// usefully recover from. A logged warning is more honest than a
// fabricated success.

mergeInto(LibraryManager.library, {
  usagi_save_write: function (keyPtr, valPtr) {
    var key = UTF8ToString(keyPtr);
    var val = UTF8ToString(valPtr);
    try {
      localStorage.setItem(key, val);
    } catch (e) {
      console.error("[usagi] save write failed for key '" + key + "':", e);
    }
  },

  usagi_save_read: function (keyPtr) {
    var key = UTF8ToString(keyPtr);
    var val = null;
    try {
      val = localStorage.getItem(key);
    } catch (e) {
      console.error("[usagi] save read failed for key '" + key + "':", e);
    }
    if (val === null) return 0;
    var len = lengthBytesUTF8(val) + 1;
    var ptr = _malloc(len);
    stringToUTF8(val, ptr, len);
    return ptr;
  },

  usagi_save_free: function (ptr) {
    _free(ptr);
  },
});
