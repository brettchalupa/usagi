---@meta
-- Usagi API stubs for lua-language-server.
-- Declarations only; this file is never executed by the runtime.

---@class Usagi.Gfx
---@field COLOR_BLACK integer
---@field COLOR_WHITE integer
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

---Draws a filled rectangle.
---@param x      number
---@param y      number
---@param w      number
---@param h      number
---@param color  integer  a gfx.COLOR_* constant
function gfx.rect(x, y, w, h, color) end

---@class Usagi.Input
---@field LEFT  integer
---@field RIGHT integer
---@field UP    integer
---@field DOWN  integer
---@field A     integer
---@field B     integer
input = {}

---Returns true while the given key is just pressed
---@param key integer  one of input.LEFT / RIGHT / UP / DOWN / A / B
---@return boolean
function input.pressed(key) end

---Returns true while the given key is held down
---@param key integer  one of input.LEFT / RIGHT / UP / DOWN / A / B
---@return boolean
function input.down(key) end

---@class Usagi
---@field gfx    Usagi.Gfx
---@field input  Usagi.Input
---@field GAME_W number  game render width in pixels
---@field GAME_H number  game render height in pixels
usagi = {}

---Called once when the game starts. Use for loading assets and initializing state.
function _init() end

---Called every frame to update game state. Runs before _draw.
---@param dt number  seconds since last frame
function _update(dt) end

---Called every frame to render. Runs after _update.
---@param dt number  seconds since last frame
function _draw(dt) end
