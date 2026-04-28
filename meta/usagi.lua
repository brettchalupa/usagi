---@meta
-- Usagi API stubs for lua-language-server.
-- Declarations only; this file is never executed by the runtime.

---Pico-8 palette, indices 0-15.
---@class Usagi.Gfx
---@field COLOR_BLACK        integer  0
---@field COLOR_DARK_BLUE    integer  1
---@field COLOR_DARK_PURPLE  integer  2
---@field COLOR_DARK_GREEN   integer  3
---@field COLOR_BROWN        integer  4
---@field COLOR_DARK_GRAY    integer  5
---@field COLOR_LIGHT_GRAY   integer  6
---@field COLOR_WHITE        integer  7
---@field COLOR_RED          integer  8
---@field COLOR_ORANGE       integer  9
---@field COLOR_YELLOW       integer  10
---@field COLOR_GREEN        integer  11
---@field COLOR_BLUE         integer  12
---@field COLOR_INDIGO       integer  13
---@field COLOR_PINK         integer  14
---@field COLOR_PEACH        integer  15
gfx = {}

---Clears the screen to the given color.
---@param color integer  a gfx.COLOR_* constant
function gfx.clear(color) end

---Draws text at (x, y) in the given color. Default 8px font.
---@param text  string  string to render
---@param x     number  left edge in game-space pixels
---@param y     number  top edge in game-space pixels
---@param color integer  a gfx.COLOR_* constant
function gfx.text(text, x, y, color) end

---Draws a rectangle outline.
---@param x     number  left edge in game-space pixels
---@param y     number  top edge in game-space pixels
---@param w     number  width in pixels
---@param h     number  height in pixels
---@param color integer  a gfx.COLOR_* constant
function gfx.rect(x, y, w, h, color) end

---Draws a filled rectangle.
---@param x     number  left edge in game-space pixels
---@param y     number  top edge in game-space pixels
---@param w     number  width in pixels
---@param h     number  height in pixels
---@param color integer  a gfx.COLOR_* constant
function gfx.rect_fill(x, y, w, h, color) end

---Draws a circle outline centered at (x, y).
---@param x     number  center x in game-space pixels
---@param y     number  center y in game-space pixels
---@param r     number  radius in pixels
---@param color integer  a gfx.COLOR_* constant
function gfx.circ(x, y, r, color) end

---Draws a filled circle centered at (x, y).
---@param x     number  center x in game-space pixels
---@param y     number  center y in game-space pixels
---@param r     number  radius in pixels
---@param color integer  a gfx.COLOR_* constant
function gfx.circ_fill(x, y, r, color) end

---Draws a line from (x1, y1) to (x2, y2).
---@param x1    number  start x in game-space pixels
---@param y1    number  start y in game-space pixels
---@param x2    number  end x in game-space pixels
---@param y2    number  end y in game-space pixels
---@param color integer  a gfx.COLOR_* constant
function gfx.line(x1, y1, x2, y2, color) end

---Sets a single pixel.
---@param x     number  x in game-space pixels
---@param y     number  y in game-space pixels
---@param color integer  a gfx.COLOR_* constant
function gfx.pixel(x, y, color) end

---Draws a 16×16 sprite from the loaded sheet at (x, y). The sheet is
---`sprites.png` next to the game's main .lua; indices run left-to-right,
---top-to-bottom. Alpha-channel pixels render as transparent.
---@param index integer  one-based sprite index (1 = top-left cell)
---@param x     number   destination left edge in game-space pixels
---@param y     number   destination top edge in game-space pixels
function gfx.spr(index, x, y) end

---Extended `spr`: draws a 16×16 sprite with required flip flags. Same
---indexing as `gfx.spr`.
---@param index  integer  one-based sprite index (1 = top-left cell)
---@param x      number   destination left edge in game-space pixels
---@param y      number   destination top edge in game-space pixels
---@param flip_x boolean  flip horizontally (mirror left/right) when true
---@param flip_y boolean  flip vertically (mirror top/bottom) when true
function gfx.spr_ex(index, x, y, flip_x, flip_y) end

---Draws an arbitrary (sx, sy, sw, sh) rectangle from `sprites.png` at
---(dx, dy) at its original size. `s*` args index into the source sheet
---in pixels; `d*` args are the destination on screen.
---@param sx number  source rect left edge on `sprites.png` (pixels)
---@param sy number  source rect top edge on `sprites.png` (pixels)
---@param sw number  source rect width in pixels
---@param sh number  source rect height in pixels
---@param dx number  destination left edge in game-space pixels
---@param dy number  destination top edge in game-space pixels
function gfx.sspr(sx, sy, sw, sh, dx, dy) end

---Extended `sspr`: source rect stretched to (dw, dh) at the destination
---with required flip flags. All ten args required; write a thin
---wrapper if a particular flag combination shows up often in your
---code.
---@param sx     number   source rect left edge on `sprites.png` (pixels)
---@param sy     number   source rect top edge on `sprites.png` (pixels)
---@param sw     number   source rect width in pixels
---@param sh     number   source rect height in pixels
---@param dx     number   destination left edge in game-space pixels
---@param dy     number   destination top edge in game-space pixels
---@param dw     number   destination width in pixels (stretches the source)
---@param dh     number   destination height in pixels (stretches the source)
---@param flip_x boolean  flip horizontally (mirror left/right) when true
---@param flip_y boolean  flip vertically (mirror top/bottom) when true
function gfx.sspr_ex(sx, sy, sw, sh, dx, dy, dw, dh, flip_x, flip_y) end

---@class Usagi.Sfx
sfx = {}

---Plays a sound effect by name. Names are file stems from the `sfx/`
---directory next to the game's main .lua (e.g. `sfx/jump.wav` → "jump").
---Unknown names silently no-op. Calling while already playing restarts.
---@param name string  file stem of a `.wav` under `sfx/`
function sfx.play(name) end

---Abstract input actions. Each is a union over keyboard keys, gamepad
---buttons, and analog-stick directions:
---
---- LEFT:  arrow left, A, dpad left, left stick left
---- RIGHT: arrow right, D, dpad right, left stick right
---- UP:    arrow up, W, dpad up, left stick up
---- DOWN:  arrow down, S, dpad down, left stick down
---- BTN1:  Z, J; gamepad south face (Xbox A, PS Cross)
---- BTN2:  X, K; gamepad east face  (Xbox B, PS Circle)
---- BTN3:  C, L; gamepad north + west face (Xbox Y/X, PS Triangle/Square)
---@class Usagi.Input
---@field LEFT  integer
---@field RIGHT integer
---@field UP    integer
---@field DOWN  integer
---@field BTN1  integer
---@field BTN2  integer
---@field BTN3  integer
input = {}

---Returns true the frame any source bound to `action` first went down.
---@param action integer  one of input.LEFT / RIGHT / UP / DOWN / BTN1 / BTN2 / BTN3
---@return boolean
function input.pressed(action) end

---Returns true while any source bound to `action` is held.
---@param action integer  one of input.LEFT / RIGHT / UP / DOWN / BTN1 / BTN2 / BTN3
---@return boolean
function input.down(action) end

---Engine-level info. The per-domain APIs (`gfx`, `input`) are top-level
---globals, not fields on this table.
---@class Usagi
---@field GAME_W  number   game render width in pixels
---@field GAME_H  number   game render height in pixels
---@field IS_DEV  boolean  true under `usagi dev`; false for `usagi run` and compiled binaries
---@field elapsed number   wall-clock seconds since session start; updated once per frame before _update
usagi = {}

---Config table returned by `_config()`. All fields optional; missing
---fields fall back to engine defaults.
---@class Usagi.Config
---@field title? string  window title (default: "Usagi")
---@field pixel_perfect? boolean integer scaling with bars (default: true)

---Optional. Returns engine config read once before the window opens.
---Omit if the defaults are fine.
---@return Usagi.Config?
function _config() end

---Called once when the game starts. Use for loading assets and initializing state.
function _init() end

---Called every frame to update game state. Runs before _draw.
---@param dt number  delta-time: seconds since last frame
function _update(dt) end

---Called every frame to render. Runs after _update.
---@param dt number  delta-time: seconds since last frame
function _draw(dt) end
