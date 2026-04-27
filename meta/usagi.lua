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

---Draws text at (x, y) in the given color.
---@param text  string
---@param x     number
---@param y     number
---@param color integer  a gfx.COLOR_* constant
function gfx.text(text, x, y, color) end

---Draws a rectangle outline.
---@param x      number
---@param y      number
---@param w      number
---@param h      number
---@param color  integer  a gfx.COLOR_* constant
function gfx.rect(x, y, w, h, color) end

---Draws a filled rectangle.
---@param x      number
---@param y      number
---@param w      number
---@param h      number
---@param color  integer  a gfx.COLOR_* constant
function gfx.rect_fill(x, y, w, h, color) end

---Draws a circle outline centered at (x, y).
---@param x      number
---@param y      number
---@param r      number  radius in pixels
---@param color  integer  a gfx.COLOR_* constant
function gfx.circ(x, y, r, color) end

---Draws a filled circle centered at (x, y).
---@param x      number
---@param y      number
---@param r      number  radius in pixels
---@param color  integer  a gfx.COLOR_* constant
function gfx.circ_fill(x, y, r, color) end

---Draws a line from (x1, y1) to (x2, y2).
---@param x1     number
---@param y1     number
---@param x2     number
---@param y2     number
---@param color  integer  a gfx.COLOR_* constant
function gfx.line(x1, y1, x2, y2, color) end

---Draws a 16×16 sprite from the loaded sheet at (x, y). The sheet is
---`sprites.png` next to the game's main .lua; indices run left-to-right,
---top-to-bottom. Alpha-channel pixels render as transparent.
---@param index integer  one-based sprite index (1 = top-left cell)
---@param x     number
---@param y     number
function gfx.spr(index, x, y) end

---@class Usagi.Sfx
sfx = {}

---Plays a sound effect by name. Names are file stems from the `sfx/`
---directory next to the game's main .lua (e.g. `sfx/jump.wav` → "jump").
---Unknown names silently no-op. Calling while already playing restarts.
---@param name string
function sfx.play(name) end

---Abstract input actions. Each is a union over keyboard keys, gamepad
---buttons, and analog-stick directions:
---
---- LEFT:    arrow left, A, dpad left, left stick left
---- RIGHT:   arrow right, D, dpad right, left stick right
---- UP:      arrow up, W, dpad up, left stick up
---- DOWN:    arrow down, S, dpad down, left stick down
---- CONFIRM: Z, J; gamepad south + west face (Xbox A/X, PS Cross/Square)
---- CANCEL:  X, K; gamepad east + north face (Xbox B/Y, PS Circle/Triangle)
---@class Usagi.Input
---@field LEFT    integer
---@field RIGHT   integer
---@field UP      integer
---@field DOWN    integer
---@field CONFIRM integer
---@field CANCEL  integer
input = {}

---Returns true the frame any source bound to `action` first went down.
---@param action integer  one of input.LEFT / RIGHT / UP / DOWN / CONFIRM / CANCEL
---@return boolean
function input.pressed(action) end

---Returns true while any source bound to `action` is held.
---@param action integer  one of input.LEFT / RIGHT / UP / DOWN / CONFIRM / CANCEL
---@return boolean
function input.down(action) end

---Engine-level info. The per-domain APIs (`gfx`, `input`) are top-level
---globals, not fields on this table.
---@class Usagi
---@field GAME_W number   game render width in pixels
---@field GAME_H number   game render height in pixels
---@field IS_DEV boolean  true under `usagi dev`; false for `usagi run` and compiled binaries
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
---@param dt number  seconds since last frame
function _update(dt) end

---Called every frame to render. Runs after _update.
---@param dt number  seconds since last frame
function _draw(dt) end
