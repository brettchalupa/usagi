---@diagnostic disable: undefined-global, lowercase-global, unused-local

-- Lua 5.5 vs LuaJIT stdlib gaps. Patch them up front so user code that
-- assumes 5.3+ semantics keeps working under Love2D / LuaJIT.
--
-- math.atan: in Lua 5.3+, `math.atan(y, x)` is the two-argument atan2.
-- In LuaJIT (Lua 5.1), `math.atan` only takes one argument and `atan2`
-- is a separate function. Without this back-fill, code that does
-- `math.atan(dy, dx)` to derive a firing angle silently drops the
-- second arg under LuaJIT, producing the wrong angle (often 0 → bullets
-- always fire to the right).
if math.atan2 and math.atan then
  local _atan = math.atan
  math.atan = function(y, x)
    if x == nil then return _atan(y) end
    return math.atan2(y, x)
  end
end

-- usagi_shim.lua
--
-- Drop next to a Usagi project's main.lua, add `require "usagi_shim"`
-- at the top, run `love .`. Targets Love2D 11.5 / LuaJIT.
--
-- Routes Usagi's runtime API (gfx, input, sfx, music, usagi, util,
-- effect) through Love's APIs. Autoloads font.png, palette.png, and
-- sprites.png from the project root. `usagi loveify` is the canonical
-- way to set up the destination project; see
-- examples/loveify/README.md in the Usagi repo for details.

local PALETTE = {
  [0]  = { 1.000, 1.000, 1.000 }, -- COLOR_TRUE_WHITE
  [1]  = { 0.000, 0.000, 0.000 }, -- COLOR_BLACK
  [2]  = { 29/255,  43/255,  83/255 }, -- COLOR_DARK_BLUE
  [3]  = { 126/255, 37/255,  83/255 }, -- COLOR_DARK_PURPLE
  [4]  = { 0.000, 135/255, 81/255 },   -- COLOR_DARK_GREEN
  [5]  = { 171/255, 82/255, 54/255 },  -- COLOR_BROWN
  [6]  = { 95/255,  87/255, 79/255 },  -- COLOR_DARK_GRAY
  [7]  = { 194/255, 195/255, 199/255 },-- COLOR_LIGHT_GRAY
  [8]  = { 1.000, 241/255, 232/255 },  -- COLOR_WHITE
  [9]  = { 1.000, 0.000, 77/255 },     -- COLOR_RED
  [10] = { 1.000, 163/255, 0.000 },    -- COLOR_ORANGE
  [11] = { 1.000, 236/255, 39/255 },   -- COLOR_YELLOW
  [12] = { 0.000, 228/255, 54/255 },   -- COLOR_GREEN
  [13] = { 41/255, 173/255, 1.000 },   -- COLOR_BLUE
  [14] = { 131/255, 118/255, 156/255 },-- COLOR_INDIGO
  [15] = { 1.000, 119/255, 168/255 },  -- COLOR_PINK
  [16] = { 1.000, 204/255, 170/255 },  -- COLOR_PEACH
}

-- Out-of-range indices render as bright magenta in Usagi as an
-- "obvious typo" sentinel. Mirror that here.
local MAGENTA = { 1.0, 0.0, 1.0 }

-- Walk a palette.png at the project root and replace slots 1..N. Pixels
-- are read row-major (left-to-right, top-to-bottom), one pixel per
-- slot. Any rectangular shape works. Slot 0 (COLOR_TRUE_WHITE) is
-- preserved off-palette so it stays a pure (1,1,1) identity tint.
local function load_palette()
  if not love.filesystem.getInfo("palette.png") then return end
  local data = love.image.newImageData("palette.png")
  local w, h = data:getDimensions()
  -- Clear slots 1..16 so a shorter palette doesn't leak Pico-8 defaults
  -- into the high indices (they'll render magenta, matching Usagi).
  for i = 1, 16 do PALETTE[i] = nil end
  local slot = 1
  for y = 0, h - 1 do
    for x = 0, w - 1 do
      local r, g, b = data:getPixel(x, y)
      PALETTE[slot] = { r, g, b }
      slot = slot + 1
    end
  end
end

-- Canvas + upscale state. Declared up front so the input module (which
-- needs scale + letterbox to translate window mouse to canvas space)
-- can reference them; the actual values get set in love.load and
-- recomputed on love.resize.
local canvas
local scale = 1
local letterbox_x = 0
local letterbox_y = 0
local pixel_perfect = false

-- Custom font state. Loaded from `font.png` (Usagi's baked atlas with
-- glyph metadata embedded as a zTXt chunk) in love.load. When set,
-- gfx.text / gfx.text_ex / usagi.measure_text route through it instead
-- of Love's default Vera Sans (which can't render pixel-art crisply).
-- Forward-declared so gfx.text closures resolve the slot at call time.
local custom_font_image
local custom_font_meta
local custom_font_quads = {}
local draw_text_custom  -- assigned later, see "Custom font" section
local utf8_iter         -- used by usagi.measure_text; defined alongside the font loader

local function rgb(c)
  return PALETTE[c] or MAGENTA
end

local function set_color(c, alpha)
  local p = rgb(c)
  love.graphics.setColor(p[1], p[2], p[3], alpha or 1.0)
end

-- gfx module ----------------------------------------------------------------

local gfx = {}

gfx.COLOR_TRUE_WHITE  = 0
gfx.COLOR_BLACK       = 1
gfx.COLOR_DARK_BLUE   = 2
gfx.COLOR_DARK_PURPLE = 3
gfx.COLOR_DARK_GREEN  = 4
gfx.COLOR_BROWN       = 5
gfx.COLOR_DARK_GRAY   = 6
gfx.COLOR_LIGHT_GRAY  = 7
gfx.COLOR_WHITE       = 8
gfx.COLOR_RED         = 9
gfx.COLOR_ORANGE      = 10
gfx.COLOR_YELLOW      = 11
gfx.COLOR_GREEN       = 12
gfx.COLOR_BLUE        = 13
gfx.COLOR_INDIGO      = 14
gfx.COLOR_PINK        = 15
gfx.COLOR_PEACH       = 16

-- Tracks the most recent gfx.clear color so love.draw can paint the
-- window backbuffer to match. Screen shake offsets the canvas blit
-- and any letterbox bars would otherwise show through as black; using
-- the same color the game cleared the canvas with keeps the bg
-- continuous through a shake.
local last_clear_color = { 0, 0, 0 }

function gfx.clear(c)
  local p = rgb(c)
  last_clear_color[1] = p[1]
  last_clear_color[2] = p[2]
  last_clear_color[3] = p[3]
  love.graphics.clear(p[1], p[2], p[3], 1.0)
end

function gfx.rect(x, y, w, h, c, alpha)
  set_color(c, alpha)
  love.graphics.setLineWidth(1)
  -- Love rasterizes "line" mode centered on the geometric path, so a
  -- rect at (x, y, w, h) can spill into the row at y+h / col at x+w.
  -- Insetting by 0.5 puts the path through pixel centers and the
  -- stroke lands exactly inside the bounding box, matching Usagi
  -- (raylib) which draws the outline at x..x+w-1, y..y+h-1.
  love.graphics.rectangle("line", x + 0.5, y + 0.5, w - 1, h - 1)
end

function gfx.rect_fill(x, y, w, h, c, alpha)
  set_color(c, alpha)
  love.graphics.rectangle("fill", x, y, w, h)
end

function gfx.rect_ex(x, y, w, h, thickness, c, alpha)
  set_color(c, alpha)
  love.graphics.setLineWidth(thickness)
  -- Inset by thickness/2 for the same reason as gfx.rect: the stroke
  -- is centered on the path, so we shift in by half its width.
  local t2 = thickness / 2
  love.graphics.rectangle("line", x + t2, y + t2, w - thickness, h - thickness)
  love.graphics.setLineWidth(1)
end

function gfx.circ(x, y, r, c, alpha)
  set_color(c, alpha)
  love.graphics.setLineWidth(1)
  love.graphics.circle("line", x, y, r)
end

function gfx.circ_fill(x, y, r, c, alpha)
  set_color(c, alpha)
  love.graphics.circle("fill", x, y, r)
end

function gfx.circ_ex(x, y, r, thickness, c, alpha)
  set_color(c, alpha)
  love.graphics.setLineWidth(thickness)
  love.graphics.circle("line", x, y, r)
  love.graphics.setLineWidth(1)
end

function gfx.line(x1, y1, x2, y2, c, alpha)
  set_color(c, alpha)
  love.graphics.setLineWidth(1)
  love.graphics.line(x1, y1, x2, y2)
end

function gfx.line_ex(x1, y1, x2, y2, thickness, c, alpha)
  set_color(c, alpha)
  love.graphics.setLineWidth(thickness)
  love.graphics.line(x1, y1, x2, y2)
  love.graphics.setLineWidth(1)
end

function gfx.tri(x1, y1, x2, y2, x3, y3, c, alpha)
  set_color(c, alpha)
  love.graphics.setLineWidth(1)
  love.graphics.polygon("line", x1, y1, x2, y2, x3, y3)
end

function gfx.tri_fill(x1, y1, x2, y2, x3, y3, c, alpha)
  set_color(c, alpha)
  love.graphics.polygon("fill", x1, y1, x2, y2, x3, y3)
end

function gfx.px(x, y, c, alpha)
  set_color(c, alpha)
  love.graphics.points(x + 0.5, y + 0.5)
end

function gfx.text(text, x, y, c, alpha)
  if custom_font_meta then
    draw_text_custom(tostring(text), x, y, 1, 0, c, alpha or 1.0)
    return
  end
  set_color(c, alpha)
  love.graphics.print(text, x, y)
end

function gfx.text_ex(text, x, y, scale, rotation, c, alpha)
  if custom_font_meta then
    draw_text_custom(tostring(text), x, y, scale, rotation, c, alpha)
    return
  end
  set_color(c, alpha)
  love.graphics.print(text, x, y, rotation or 0, scale or 1, scale or 1)
end

-- Post-process shaders are out of scope for the shim. These two stubs
-- exist so a Usagi project that uses shaders doesn't fail to load —
-- the calls are silent no-ops; use Love's native shader API
-- (love.graphics.newShader / setShader / Shader:send) for real shader
-- work in your port.
function gfx.shader_set(_name)
end

function gfx.shader_uniform(_name, _value)
end

-- Sprites -------------------------------------------------------------------
-- sprites.png lives next to main.lua in a Usagi project. The shim loads it
-- lazily on the first sprite call so projects without sprites don't error
-- at boot. `gfx.spr(index, x, y)` uses 1-based grid indexing left-to-right,
-- top-to-bottom over cells of `usagi.SPRITE_SIZE`.

local sprite_image
local sprite_image_data  -- CPU mirror for gfx.get_spr_px readback
local sprite_image_loaded = false
local sprite_quad_cache = {}
local sprite_cols

local function load_sprites()
  if sprite_image_loaded then return sprite_image end
  sprite_image_loaded = true
  if love.filesystem.getInfo("sprites.png") then
    sprite_image_data = love.image.newImageData("sprites.png")
    sprite_image = love.graphics.newImage(sprite_image_data)
    sprite_image:setFilter("nearest", "nearest")
    sprite_cols = math.floor(sprite_image:getWidth() / usagi.SPRITE_SIZE)
  end
  return sprite_image
end

local function quad_for_index(index)
  local cached = sprite_quad_cache[index]
  if cached then return cached end
  local img = sprite_image
  if not img then return nil end
  local sz = usagi.SPRITE_SIZE
  local col = (index - 1) % sprite_cols
  local row = math.floor((index - 1) / sprite_cols)
  local q = love.graphics.newQuad(col * sz, row * sz, sz, sz,
    img:getWidth(), img:getHeight())
  sprite_quad_cache[index] = q
  return q
end

function gfx.spr(index, x, y, alpha)
  local img = load_sprites()
  if not img then return end
  local q = quad_for_index(index)
  if not q then return end
  love.graphics.setColor(1, 1, 1, alpha or 1.0)
  love.graphics.draw(img, q, x, y)
end

function gfx.spr_ex(index, x, y, flip_x, flip_y, rotation, tint, alpha)
  local img = load_sprites()
  if not img then return end
  local q = quad_for_index(index)
  if not q then return end
  local sz = usagi.SPRITE_SIZE
  local sx = flip_x and -1 or 1
  local sy = flip_y and -1 or 1
  set_color(tint, alpha)
  -- Rotate around sprite center: position the draw at (x + sz/2, y + sz/2)
  -- and use (sz/2, sz/2) as the origin so the sprite stays at (x, y) and
  -- rotates in place.
  love.graphics.draw(img, q, x + sz / 2, y + sz / 2,
    rotation or 0, sx, sy, sz / 2, sz / 2)
end

function gfx.sspr(sx, sy, sw, sh, dx, dy, alpha)
  local img = load_sprites()
  if not img then return end
  local q = love.graphics.newQuad(sx, sy, sw, sh,
    img:getWidth(), img:getHeight())
  love.graphics.setColor(1, 1, 1, alpha or 1.0)
  love.graphics.draw(img, q, dx, dy)
end

function gfx.sspr_ex(sx, sy, sw, sh, dx, dy, dw, dh,
                     flip_x, flip_y, rotation, tint, alpha)
  local img = load_sprites()
  if not img then return end
  local q = love.graphics.newQuad(sx, sy, sw, sh,
    img:getWidth(), img:getHeight())
  local scale_x = (dw / sw) * (flip_x and -1 or 1)
  local scale_y = (dh / sh) * (flip_y and -1 or 1)
  set_color(tint, alpha)
  love.graphics.draw(img, q, dx + dw / 2, dy + dh / 2,
    rotation or 0, scale_x, scale_y, sw / 2, sh / 2)
end

-- Pixel readback -----------------------------------------------------------
-- gfx.get_px returns r, g, b, palette_index for the screen pixel at
-- (x, y). The screen is snapshotted from the canvas at the end of each
-- love.draw, so the first call returns nil (no snapshot yet); subsequent
-- frames see the previous frame's pixels. RGB is reported as 0..255
-- ints to match Usagi. palette_index is the 1-based slot if the color
-- matches the active palette exactly, or nil otherwise.

local screen_snapshot
local snapshot_requested = false

local function palette_slot_for(r8, g8, b8)
  for i = 1, 16 do
    local p = PALETTE[i]
    if p
        and math.floor(p[1] * 255 + 0.5) == r8
        and math.floor(p[2] * 255 + 0.5) == g8
        and math.floor(p[3] * 255 + 0.5) == b8 then
      return i
    end
  end
  return nil
end

function gfx.get_px(x, y)
  snapshot_requested = true
  if not screen_snapshot then return nil, nil, nil, nil end
  local xi = math.floor(x + 0.5)
  local yi = math.floor(y + 0.5)
  if xi < 0 or yi < 0 or xi >= usagi.GAME_W or yi >= usagi.GAME_H then
    return nil, nil, nil, nil
  end
  local r, g, b = screen_snapshot:getPixel(xi, yi)
  local r8 = math.floor(r * 255 + 0.5)
  local g8 = math.floor(g * 255 + 0.5)
  local b8 = math.floor(b * 255 + 0.5)
  return r8, g8, b8, palette_slot_for(r8, g8, b8)
end

function gfx.get_spr_px(index, x, y)
  load_sprites()
  if not sprite_image_data or index < 1 then
    return nil, nil, nil, nil
  end
  local sz = usagi.SPRITE_SIZE
  local xi = math.floor(x + 0.5)
  local yi = math.floor(y + 0.5)
  if xi < 0 or yi < 0 or xi >= sz or yi >= sz then
    return nil, nil, nil, nil
  end
  local col = (index - 1) % sprite_cols
  local row = math.floor((index - 1) / sprite_cols)
  if row * sz >= sprite_image_data:getHeight() then
    return nil, nil, nil, nil
  end
  local r, g, b, a = sprite_image_data:getPixel(col * sz + xi, row * sz + yi)
  if a == 0 then return nil, nil, nil, nil end
  local r8 = math.floor(r * 255 + 0.5)
  local g8 = math.floor(g * 255 + 0.5)
  local b8 = math.floor(b * 255 + 0.5)
  return r8, g8, b8, palette_slot_for(r8, g8, b8)
end

-- input module --------------------------------------------------------------
-- Phase 2A: action-mapped keyboard + gamepad. Mouse and direct-keyboard
-- come in Phase 2B.

local input = {}

input.LEFT  = 1
input.RIGHT = 2
input.UP    = 3
input.DOWN  = 4
input.BTN1  = 5
input.BTN2  = 6
input.BTN3  = 7

-- Default action -> keyboard bindings, matching Usagi's defaults: each
-- action accepts arrow / WASD equivalents, with ZXC and JKL bound to
-- BTN1/BTN2/BTN3 on the home row.
local ACTION_KEYS = {
  [input.LEFT]  = { "left", "a" },
  [input.RIGHT] = { "right", "d" },
  [input.UP]    = { "up", "w" },
  [input.DOWN]  = { "down", "s" },
  [input.BTN1]  = { "z", "j" },
  [input.BTN2]  = { "x", "k" },
  [input.BTN3]  = { "c", "l" },
}

-- SDL gamepad-mapped button bindings, mirroring Usagi's BINDINGS gamepad
-- portion (raylib's GAMEPAD_BUTTON_* names translated to SDL's names that
-- Love exposes via Joystick:isGamepadDown). BTN3 covers both Y and X face
-- buttons because either is easier to hit than the opposite corner.
local ACTION_PAD_BUTTONS = {
  [input.LEFT]  = { "dpleft" },
  [input.RIGHT] = { "dpright" },
  [input.UP]    = { "dpup" },
  [input.DOWN]  = { "dpdown" },
  [input.BTN1]  = { "a", "leftshoulder" },
  [input.BTN2]  = { "b", "rightshoulder" },
  [input.BTN3]  = { "y", "x" },
}

-- Analog stick → digital action thresholds. Only the four directionals
-- get stick bindings; BTN1/2/3 are button-only.
local ACTION_PAD_AXES = {
  [input.LEFT]  = { axis = "leftx", sign = -1 },
  [input.RIGHT] = { axis = "leftx", sign = 1 },
  [input.UP]    = { axis = "lefty", sign = -1 },
  [input.DOWN]  = { axis = "lefty", sign = 1 },
}

-- Stick deflection past this magnitude counts as a digital action press
-- in either direction. Matches Usagi's default deadzone.
local PAD_AXIS_DEADZONE = 0.3

-- All connected gamepad-capable joysticks. Inputs are OR'd across every
-- slot, so a Steam Deck plus an extra controller, or two controllers
-- sharing a couch, all feed the same action set.
local connected_gamepads = {}

-- Detected family of the first connected pad; recomputed on connect /
-- disconnect so action_held and mapping_for can adapt without scanning
-- the gamepad name each call.
local current_family = "xbox"

local function detect_family(pad)
  local name = pad:getName():lower()
  if name:find("playstation") or name:find("dualshock")
      or name:find("dualsense") or name:find("sony") then
    return "playstation"
  end
  if name:find("switch") or name:find("nintendo")
      or name:find("joy-con") or name:find("pro controller") then
    return "nintendo"
  end
  return "xbox"
end

local function refresh_gamepads()
  connected_gamepads = {}
  for _, j in ipairs(love.joystick.getJoysticks()) do
    if j:isGamepad() then
      connected_gamepads[#connected_gamepads + 1] = j
    end
  end
  current_family = connected_gamepads[1]
      and detect_family(connected_gamepads[1])
      or "xbox"
end

function love.joystickadded(_) refresh_gamepads() end
function love.joystickremoved(_) refresh_gamepads() end

-- Returns the SDL button names that fire `action` on the active family.
-- Nintendo controllers physically swap south/east vs Xbox/PlayStation:
-- the button printed "A" sits at the east position (SDL "b"), and "B"
-- sits at the south position (SDL "a"). We swap our BTN1/BTN2 button
-- lists so the player's "press A to confirm" expectation holds
-- regardless of which controller they brought to the couch.
local function pad_buttons_for(action)
  if current_family == "nintendo" then
    if action == input.BTN1 then return { "b", "leftshoulder" } end
    if action == input.BTN2 then return { "a", "rightshoulder" } end
  end
  return ACTION_PAD_BUTTONS[action]
end

local function action_held(action)
  local keys = ACTION_KEYS[action]
  if keys then
    for _, k in ipairs(keys) do
      if love.keyboard.isDown(k) then return true end
    end
  end
  local buttons = pad_buttons_for(action)
  local axis = ACTION_PAD_AXES[action]
  for _, pad in ipairs(connected_gamepads) do
    if buttons then
      for _, b in ipairs(buttons) do
        if pad:isGamepadDown(b) then return true end
      end
    end
    if axis then
      local v = pad:getGamepadAxis(axis.axis)
      if axis.sign < 0 and v <= -PAD_AXIS_DEADZONE then return true end
      if axis.sign > 0 and v >= PAD_AXIS_DEADZONE then return true end
    end
  end
  return false
end

-- Tracks "was this action held at the start of this frame?" so that
-- pressed/released can return single-frame transitions. Updated at the
-- tail of love.update.
local last_action_state = {}

function input.held(action)
  return action_held(action)
end

function input.pressed(action)
  return action_held(action) and not last_action_state[action]
end

function input.released(action)
  return (not action_held(action)) and last_action_state[action] == true
end

-- Direct keyboard -----------------------------------------------------------
-- KEY_* constant values are the Love key-name strings themselves
-- (Love's love.keyboard.isDown takes string names like "a", "left", "f1").
-- This lets `input.key_held(input.KEY_W)` resolve to
-- `love.keyboard.isDown("w")` with no translation table.

input.KEY_A = "a"; input.KEY_B = "b"; input.KEY_C = "c"; input.KEY_D = "d"
input.KEY_E = "e"; input.KEY_F = "f"; input.KEY_G = "g"; input.KEY_H = "h"
input.KEY_I = "i"; input.KEY_J = "j"; input.KEY_K = "k"; input.KEY_L = "l"
input.KEY_M = "m"; input.KEY_N = "n"; input.KEY_O = "o"; input.KEY_P = "p"
input.KEY_Q = "q"; input.KEY_R = "r"; input.KEY_S = "s"; input.KEY_T = "t"
input.KEY_U = "u"; input.KEY_V = "v"; input.KEY_W = "w"; input.KEY_X = "x"
input.KEY_Y = "y"; input.KEY_Z = "z"

input.KEY_0 = "0"; input.KEY_1 = "1"; input.KEY_2 = "2"; input.KEY_3 = "3"
input.KEY_4 = "4"; input.KEY_5 = "5"; input.KEY_6 = "6"; input.KEY_7 = "7"
input.KEY_8 = "8"; input.KEY_9 = "9"

input.KEY_F1 = "f1"; input.KEY_F2 = "f2"; input.KEY_F3 = "f3"
input.KEY_F4 = "f4"; input.KEY_F5 = "f5"; input.KEY_F6 = "f6"
input.KEY_F7 = "f7"; input.KEY_F8 = "f8"; input.KEY_F9 = "f9"
input.KEY_F10 = "f10"; input.KEY_F11 = "f11"; input.KEY_F12 = "f12"

input.KEY_SPACE = "space"
input.KEY_ENTER = "return"
input.KEY_ESCAPE = "escape"
input.KEY_TAB = "tab"
input.KEY_BACKSPACE = "backspace"
input.KEY_DELETE = "delete"

input.KEY_LEFT = "left"
input.KEY_RIGHT = "right"
input.KEY_UP = "up"
input.KEY_DOWN = "down"

input.KEY_LSHIFT = "lshift"; input.KEY_RSHIFT = "rshift"
input.KEY_LCTRL  = "lctrl";  input.KEY_RCTRL  = "rctrl"
input.KEY_LALT   = "lalt";   input.KEY_RALT   = "ralt"

input.KEY_BACKTICK = "`"
input.KEY_MINUS = "-"
input.KEY_EQUAL = "="
input.KEY_LBRACKET = "["
input.KEY_RBRACKET = "]"
input.KEY_BACKSLASH = "\\"
input.KEY_SEMICOLON = ";"
input.KEY_APOSTROPHE = "'"
input.KEY_COMMA = ","
input.KEY_PERIOD = "."
input.KEY_SLASH = "/"

-- Lazy tracking: only keys queried since boot are snapshot each frame.
-- Cheaper than scanning all 75 every update.
local tracked_keys = {}
local last_key_state = {}

function input.key_held(key)
  tracked_keys[key] = true
  return love.keyboard.isDown(key)
end

function input.key_pressed(key)
  tracked_keys[key] = true
  return love.keyboard.isDown(key) and not last_key_state[key]
end

function input.key_released(key)
  tracked_keys[key] = true
  return (not love.keyboard.isDown(key)) and last_key_state[key] == true
end

-- Mouse ---------------------------------------------------------------------
-- Usagi numbers MOUSE_LEFT=0, RIGHT=1, MIDDLE=2; Love uses 1, 2, 3. Use
-- Love's numbering directly so the user passes constants and we don't
-- have to translate.

input.MOUSE_LEFT   = 1
input.MOUSE_RIGHT  = 2
input.MOUSE_MIDDLE = 3

-- Mouse position in canvas (game) coordinates. Translates from window
-- pixels through the upscale/letterbox transform applied in love.draw.
function input.mouse()
  local mx, my = love.mouse.getPosition()
  local cx = math.floor((mx - letterbox_x) / scale)
  local cy = math.floor((my - letterbox_y) / scale)
  return cx, cy
end

-- True when the cursor is over the drawn game area: inside the window
-- (hasMouseFocus mirrors Usagi's is_cursor_on_screen check) and within
-- the canvas bounds rather than the letterbox bars.
function input.mouse_over()
  if not love.window.hasMouseFocus() then return false end
  local cx, cy = input.mouse()
  return cx >= 0 and cy >= 0 and cx < usagi.GAME_W and cy < usagi.GAME_H
end

local last_mouse_state = {}

function input.mouse_held(button)
  return love.mouse.isDown(button)
end

function input.mouse_pressed(button)
  return love.mouse.isDown(button) and not last_mouse_state[button]
end

function input.mouse_released(button)
  return (not love.mouse.isDown(button)) and last_mouse_state[button] == true
end

-- Per-frame vertical scroll delta. love.wheelmoved fires per OS scroll
-- event; we accumulate over the frame and reset at the tail of
-- love.update so _update / _draw within the same frame see the same
-- value.
local scroll_delta = 0.0

function input.mouse_scroll()
  return scroll_delta
end

function love.wheelmoved(_dx, dy)
  scroll_delta = scroll_delta + dy
end

function input.set_mouse_visible(visible)
  love.mouse.setVisible(visible)
end

function input.mouse_visible()
  return love.mouse.isVisible()
end

-- last_source / mapping_for -------------------------------------------------

input.SOURCE_KEYBOARD = "keyboard"
input.SOURCE_GAMEPAD = "gamepad"

local current_source = "keyboard"

local function any_action_keyboard_active()
  for action = input.LEFT, input.BTN3 do
    for _, k in ipairs(ACTION_KEYS[action]) do
      if love.keyboard.isDown(k) then return true end
    end
  end
  return false
end

local function any_action_gamepad_active()
  for _, pad in ipairs(connected_gamepads) do
    for action = input.LEFT, input.BTN3 do
      local buttons = pad_buttons_for(action)
      if buttons then
        for _, b in ipairs(buttons) do
          if pad:isGamepadDown(b) then return true end
        end
      end
      local axis = ACTION_PAD_AXES[action]
      if axis then
        local v = pad:getGamepadAxis(axis.axis)
        if axis.sign < 0 and v <= -PAD_AXIS_DEADZONE then return true end
        if axis.sign > 0 and v >= PAD_AXIS_DEADZONE then return true end
      end
    end
  end
  return false
end

local function update_current_source()
  -- Gamepad wins ties: a player actively pressing the pad is more
  -- deliberate than ambient keyboard hold. If neither fires this
  -- frame, the previous source sticks so stray Esc / F-key presses
  -- can't flip the indicator.
  if any_action_gamepad_active() then
    current_source = "gamepad"
  elseif any_action_keyboard_active() then
    current_source = "keyboard"
  end
end

function input.last_source()
  return current_source
end

local KEY_LABELS = {
  ["left"] = "Left", ["right"] = "Right", ["up"] = "Up", ["down"] = "Down",
  ["space"] = "Space", ["return"] = "Enter", ["escape"] = "Escape",
  ["tab"] = "Tab", ["backspace"] = "Backspace", ["delete"] = "Delete",
  ["lshift"] = "LShift", ["rshift"] = "RShift",
  ["lctrl"] = "LCtrl",   ["rctrl"] = "RCtrl",
  ["lalt"] = "LAlt",     ["ralt"] = "RAlt",
  ["`"] = "Backtick", ["-"] = "Minus", ["="] = "Equal",
  ["["] = "LBracket", ["]"] = "RBracket", ["\\"] = "Backslash",
  [";"] = "Semicolon", ["'"] = "Apostrophe",
  [","] = "Comma", ["."] = "Period", ["/"] = "Slash",
}

local PAD_BUTTON_LABELS = {
  xbox = {
    a = "A", b = "B", x = "X", y = "Y",
    dpleft = "DPad Left", dpright = "DPad Right",
    dpup = "DPad Up", dpdown = "DPad Down",
    leftshoulder = "LB", rightshoulder = "RB",
  },
  playstation = {
    a = "Cross", b = "Circle", x = "Square", y = "Triangle",
    dpleft = "DPad Left", dpright = "DPad Right",
    dpup = "DPad Up", dpdown = "DPad Down",
    leftshoulder = "L1", rightshoulder = "R1",
  },
  nintendo = {
    -- Physical-layout labels matching what's printed on the controller.
    -- SDL's A/B/X/Y in Nintendo gamepads are the abstract south/east/
    -- west/north positions, which differ from Nintendo's printed names.
    a = "B", b = "A", x = "Y", y = "X",
    dpleft = "DPad Left", dpright = "DPad Right",
    dpup = "DPad Up", dpdown = "DPad Down",
    leftshoulder = "L", rightshoulder = "R",
  },
}

function input.mapping_for(action)
  if current_source == "keyboard" then
    local keys = ACTION_KEYS[action]
    if keys and keys[1] then
      local k = keys[1]
      return KEY_LABELS[k] or k:upper()
    end
    return nil
  end
  local labels = PAD_BUTTON_LABELS[current_family]
  local buttons = pad_buttons_for(action)
  if buttons and buttons[1] then
    return labels[buttons[1]] or buttons[1]
  end
  local axis = ACTION_PAD_AXES[action]
  if axis then
    local base = (axis.axis == "leftx") and "Left X" or "Left Y"
    return (axis.sign < 0 and "-" or "+") .. base
  end
  return nil
end

-- usagi module --------------------------------------------------------------

local usagi = {}

usagi.GAME_W = 320
usagi.GAME_H = 180
usagi.SPRITE_SIZE = 16
usagi.elapsed = 0.0
-- Map Love's love.system.getOS() return values to Usagi's PLATFORM names.
-- Web is "Web" in Love.js; Usagi uses lowercased short forms.
local _os = love.system.getOS()
usagi.PLATFORM =
    (_os == "OS X" and "macos")
    or (_os == "Windows" and "windows")
    or (_os == "Linux" and "linux")
    or (_os == "Web" and "web")
    or "unknown"
-- Usagi uses IS_DEV to gate debug overlays etc.; Love has no equivalent
-- dev vs. shipped distinction (you run `love .` either way), so default
-- false and let users override it themselves if their dev/release
-- builds need different code paths.
usagi.IS_DEV = false
usagi.IS_RELEASE = true

function usagi.quit()
  love.event.quit()
end

function usagi.toggle_fullscreen()
  local fs = not love.window.getFullscreen()
  love.window.setFullscreen(fs)
  return fs
end

function usagi.is_fullscreen()
  return love.window.getFullscreen()
end

-- Pause menu is dropped in the shim (engine-managed feature). These
-- two stubs are no-ops so a Usagi project that registers menu items
-- doesn't fail to load; the registered items simply never render.
function usagi.menu_item(_label, _callback)
end

function usagi.clear_menu_items()
end

-- Returns (width, height) of the rendered string. Uses the custom
-- font's per-glyph advances + line_height when font.png is loaded,
-- otherwise falls back to Love's active font metrics.
function usagi.measure_text(text)
  if custom_font_meta then
    local s = tostring(text)
    local w = 0
    local i = 1
    while i <= #s do
      local cp, next_i = utf8_iter(s, i)
      if not cp then break end
      local g = custom_font_meta.glyphs[tostring(cp)]
      if g then w = w + g.advance end
      i = next_i
    end
    return w, custom_font_meta.line_height or 0
  end
  local font = love.graphics.getFont()
  return font:getWidth(text), font:getHeight()
end

-- JSON encoder + decoder ---------------------------------------------------
-- Encoder matches Usagi's strict shape rules so round-trips via save /
-- load behave identically: tables are either dense 1..n int arrays or
-- string-keyed maps, never mixed. Numbers must be finite. Other Lua
-- types (function, userdata, thread) error rather than silently
-- producing "null". Pretty-prints with 2-space indent so save files
-- diff cleanly.

local function json_classify_table(t)
  local int_keys, str_count, int_max = 0, 0, 0
  for k in pairs(t) do
    local kt = type(k)
    if kt == "string" then
      str_count = str_count + 1
    elseif kt == "number" and k % 1 == 0 and k >= 1 then
      int_keys = int_keys + 1
      if k > int_max then int_max = k end
    else
      error("JSON: table key must be a string or 1..n integer; got " .. kt, 0)
    end
  end
  if int_keys > 0 and str_count > 0 then
    error("JSON: table mixes string and integer keys", 0)
  end
  if int_keys > 0 then
    if int_keys ~= int_max then
      error(string.format(
        "JSON: integer-keyed table must be dense 1..n (no gaps); got %d keys, max %d",
        int_keys, int_max), 0)
    end
    return "array", int_max
  end
  if str_count > 0 then return "map", str_count end
  return "empty", 0
end

local JSON_ESCAPE = {
  ["\""] = "\\\"", ["\\"] = "\\\\",
  ["\b"] = "\\b",  ["\f"] = "\\f",
  ["\n"] = "\\n",  ["\r"] = "\\r", ["\t"] = "\\t",
}

local function json_encode_string(s)
  local parts = { '"' }
  for i = 1, #s do
    local c = s:sub(i, i)
    local byte = string.byte(c)
    if JSON_ESCAPE[c] then
      parts[#parts + 1] = JSON_ESCAPE[c]
    elseif byte < 32 then
      parts[#parts + 1] = string.format("\\u%04x", byte)
    else
      parts[#parts + 1] = c
    end
  end
  parts[#parts + 1] = '"'
  return table.concat(parts)
end

local json_encode_value
json_encode_value = function(v, indent)
  local t = type(v)
  if v == nil then return "null" end
  if t == "boolean" then return v and "true" or "false" end
  if t == "number" then
    if v ~= v or v == math.huge or v == -math.huge then
      error("JSON: cannot encode non-finite number", 0)
    end
    if v % 1 == 0 and math.abs(v) < 1e15 then
      return string.format("%d", v)
    end
    return tostring(v)
  end
  if t == "string" then return json_encode_string(v) end
  if t == "table" then
    local shape, count = json_classify_table(v)
    if shape == "empty" then return "{}" end
    local inner = indent .. "  "
    local nl = "\n" .. inner
    local close_nl = "\n" .. indent
    if shape == "array" then
      local parts = {}
      for i = 1, count do
        parts[i] = json_encode_value(v[i], inner)
      end
      return "[" .. nl .. table.concat(parts, "," .. nl) .. close_nl .. "]"
    end
    local keys = {}
    for k in pairs(v) do keys[#keys + 1] = k end
    table.sort(keys)
    local parts = {}
    for i, k in ipairs(keys) do
      parts[i] = json_encode_string(k) .. ": " .. json_encode_value(v[k], inner)
    end
    return "{" .. nl .. table.concat(parts, "," .. nl) .. close_nl .. "}"
  end
  error("JSON: cannot encode " .. t, 0)
end

local function json_encode(v)
  return json_encode_value(v, "")
end

local function json_skip_ws(s, i)
  while i <= #s do
    local b = s:byte(i)
    if b == 32 or b == 9 or b == 10 or b == 13 then
      i = i + 1
    else
      return i
    end
  end
  return i
end

local function json_decode_string(s, i)
  local out = {}
  i = i + 1
  while i <= #s do
    local c = s:sub(i, i)
    if c == '"' then
      return table.concat(out), i + 1
    elseif c == "\\" then
      local esc = s:sub(i + 1, i + 1)
      if esc == "n" then out[#out + 1] = "\n"
      elseif esc == "t" then out[#out + 1] = "\t"
      elseif esc == "r" then out[#out + 1] = "\r"
      elseif esc == "b" then out[#out + 1] = "\b"
      elseif esc == "f" then out[#out + 1] = "\f"
      elseif esc == "\"" then out[#out + 1] = "\""
      elseif esc == "\\" then out[#out + 1] = "\\"
      elseif esc == "/" then out[#out + 1] = "/"
      elseif esc == "u" then
        local code = tonumber(s:sub(i + 2, i + 5), 16)
        if not code then error("JSON: bad unicode escape at " .. i, 0) end
        if code < 0x80 then
          out[#out + 1] = string.char(code)
        elseif code < 0x800 then
          out[#out + 1] = string.char(
            0xC0 + math.floor(code / 64),
            0x80 + (code % 64))
        else
          out[#out + 1] = string.char(
            0xE0 + math.floor(code / 4096),
            0x80 + math.floor(code / 64) % 64,
            0x80 + (code % 64))
        end
        i = i + 4
      else
        error("JSON: bad escape \\" .. esc .. " at " .. i, 0)
      end
      i = i + 2
    else
      out[#out + 1] = c
      i = i + 1
    end
  end
  error("JSON: unterminated string", 0)
end

local function json_decode_number(s, i)
  local start = i
  if s:sub(i, i) == "-" then i = i + 1 end
  while i <= #s do
    local c = s:sub(i, i)
    if c:match("[0-9.eE+%-]") then
      i = i + 1
    else
      break
    end
  end
  local n = tonumber(s:sub(start, i - 1))
  if not n then error("JSON: bad number at " .. start, 0) end
  return n, i
end

local json_decode_value
json_decode_value = function(s, i)
  i = json_skip_ws(s, i)
  if i > #s then error("JSON: unexpected end of input", 0) end
  local c = s:sub(i, i)
  if c == '"' then return json_decode_string(s, i) end
  if c == "{" then
    local t = {}
    i = json_skip_ws(s, i + 1)
    if s:sub(i, i) == "}" then return t, i + 1 end
    while true do
      i = json_skip_ws(s, i)
      if s:sub(i, i) ~= '"' then error("JSON: expected string key at " .. i, 0) end
      local key
      key, i = json_decode_string(s, i)
      i = json_skip_ws(s, i)
      if s:sub(i, i) ~= ":" then error("JSON: expected ':' at " .. i, 0) end
      i = i + 1
      local val
      val, i = json_decode_value(s, i)
      t[key] = val
      i = json_skip_ws(s, i)
      local nc = s:sub(i, i)
      if nc == "," then i = i + 1
      elseif nc == "}" then return t, i + 1
      else error("JSON: expected ',' or '}' at " .. i, 0) end
    end
  end
  if c == "[" then
    local t = {}
    i = json_skip_ws(s, i + 1)
    if s:sub(i, i) == "]" then return t, i + 1 end
    local idx = 1
    while true do
      local val
      val, i = json_decode_value(s, i)
      t[idx] = val
      idx = idx + 1
      i = json_skip_ws(s, i)
      local nc = s:sub(i, i)
      if nc == "," then i = i + 1
      elseif nc == "]" then return t, i + 1
      else error("JSON: expected ',' or ']' at " .. i, 0) end
    end
  end
  if c == "t" and s:sub(i, i + 3) == "true" then return true, i + 4 end
  if c == "f" and s:sub(i, i + 4) == "false" then return false, i + 5 end
  if c == "n" and s:sub(i, i + 3) == "null" then return nil, i + 4 end
  if c == "-" or c:match("[0-9]") then return json_decode_number(s, i) end
  error("JSON: unexpected character '" .. c .. "' at " .. i, 0)
end

local function json_decode(s)
  local v, i = json_decode_value(s, 1)
  i = json_skip_ws(s, i)
  if i <= #s then error("JSON: trailing content after value", 0) end
  return v
end

-- Custom font loader + renderer --------------------------------------------
-- Reads Usagi's baked `font.png` (PNG atlas + zTXt JSON metadata) and
-- routes gfx.text / gfx.text_ex / usagi.measure_text through a manual
-- per-glyph blit so pixel-art fonts stay crisp. When no `font.png` is
-- present, the shim falls back to Love's default Vera Sans.

-- Iterate UTF-8 codepoints in a Lua string. Returns (codepoint,
-- next_index) or (nil) at the end. Invalid bytes yield U+FFFD and
-- advance one byte. Handles 1–4 byte sequences. Forward-declared at
-- the top of the file because usagi.measure_text uses it.
utf8_iter = function(s, i)
  if i > #s then return nil end
  local b1 = s:byte(i)
  if b1 < 0x80 then
    return b1, i + 1
  elseif b1 < 0xC0 then
    return 0xFFFD, i + 1
  elseif b1 < 0xE0 then
    local b2 = s:byte(i + 1) or 0
    return (b1 - 0xC0) * 64 + (b2 - 0x80), i + 2
  elseif b1 < 0xF0 then
    local b2 = s:byte(i + 1) or 0
    local b3 = s:byte(i + 2) or 0
    return (b1 - 0xE0) * 4096 + (b2 - 0x80) * 64 + (b3 - 0x80), i + 3
  else
    local b2 = s:byte(i + 1) or 0
    local b3 = s:byte(i + 2) or 0
    local b4 = s:byte(i + 3) or 0
    return (b1 - 0xF0) * 262144 + (b2 - 0x80) * 4096
        + (b3 - 0x80) * 64 + (b4 - 0x80), i + 4
  end
end

-- Read a 4-byte big-endian unsigned int from a string at position i.
local function read_u32_be(s, i)
  local b1, b2, b3, b4 = s:byte(i, i + 3)
  return b1 * 16777216 + b2 * 65536 + b3 * 256 + b4
end

-- Walk a PNG byte stream looking for a zTXt chunk whose keyword
-- matches. Returns the decompressed payload as a string, or nil if not
-- found / malformed. PNG layout: 8-byte signature, then repeated
-- chunks of (4-byte length, 4-byte type, length bytes data, 4-byte CRC).
local function extract_ztxt(bytes, keyword)
  if #bytes < 8 or bytes:sub(1, 8) ~= "\137PNG\r\n\26\n" then
    return nil
  end
  local pos = 9
  while pos + 7 <= #bytes do
    local length = read_u32_be(bytes, pos)
    local chunk_type = bytes:sub(pos + 4, pos + 7)
    local data_start = pos + 8
    local data_end = data_start + length - 1
    if chunk_type == "zTXt" and data_end <= #bytes then
      local chunk_data = bytes:sub(data_start, data_end)
      local nul = chunk_data:find("\0", 1, true)
      if nul then
        local kw = chunk_data:sub(1, nul - 1)
        if kw == keyword then
          -- After \0 is a single compression method byte (usually 0 =
          -- deflate), then the zlib stream.
          local compressed = chunk_data:sub(nul + 2)
          local ok, result = pcall(love.data.decompress, "string", "zlib", compressed)
          if ok and type(result) == "string" then
            return result
          end
        end
      end
    end
    pos = data_end + 5
  end
  return nil
end

local function load_custom_font()
  if not love.filesystem.getInfo("font.png") then return end
  local bytes = love.filesystem.read("font.png")
  if not bytes then return end
  local json_str = extract_ztxt(bytes, "usagi-font")
  if not json_str then return end
  local ok, meta = pcall(json_decode, json_str)
  if not ok or type(meta) ~= "table" or type(meta.glyphs) ~= "table" then
    return
  end
  custom_font_meta = meta
  custom_font_image = love.graphics.newImage("font.png")
  custom_font_image:setFilter("nearest", "nearest")
end

local function quad_for_cp(cp)
  local cached = custom_font_quads[cp]
  if cached then return cached end
  local g = custom_font_meta and custom_font_meta.glyphs[tostring(cp)]
  if not g or g.w == 0 then return nil end
  local q = love.graphics.newQuad(g.x, g.y, g.w, g.h,
    custom_font_image:getWidth(), custom_font_image:getHeight())
  custom_font_quads[cp] = q
  return q
end

-- Forward-declared at the top of the file; assigning the body here.
draw_text_custom = function(text, x, y, scale_, rotation, c, alpha)
  set_color(c, alpha)
  love.graphics.push()
  love.graphics.translate(x, y)
  if rotation and rotation ~= 0 then love.graphics.rotate(rotation) end
  if scale_ and scale_ ~= 1 then love.graphics.scale(scale_, scale_) end
  local pen_x = 0
  local i = 1
  while i <= #text do
    local cp, next_i = utf8_iter(text, i)
    if not cp then break end
    local g = custom_font_meta.glyphs[tostring(cp)]
    if g then
      if g.w > 0 then
        local q = quad_for_cp(cp)
        if q then
          love.graphics.draw(custom_font_image, q, pen_x + g.ox, g.oy)
        end
      end
      pen_x = pen_x + g.advance
    end
    i = next_i
  end
  love.graphics.pop()
end

-- Reject paths that would escape the data/ prefix: absolute, backslashes,
-- or `..` segments. Mirrors Usagi's vfs::safe_rel_path.
local function safe_rel_path(path, fn_name)
  if path:sub(1, 1) == "/" then
    error(fn_name .. ": absolute paths not allowed", 2)
  end
  if path:find("\\") then
    error(fn_name .. ": use forward slashes (no \\)", 2)
  end
  if path:find("%.%.") then
    error(fn_name .. ": '..' segments not allowed", 2)
  end
  return path
end

function usagi.to_json(t)
  return json_encode(t)
end

function usagi.read_json(path)
  safe_rel_path(path, "usagi.read_json")
  local full = "data/" .. path
  local data, err = love.filesystem.read(full)
  if not data then
    error("usagi.read_json: " .. full .. " not found", 2)
  end
  return json_decode(data)
end

function usagi.read_text(path)
  safe_rel_path(path, "usagi.read_text")
  local full = "data/" .. path
  local data, err = love.filesystem.read(full)
  if not data then
    error("usagi.read_text: " .. full .. " not found", 2)
  end
  return data
end

-- save.json lives in the per-game identity dir. love.filesystem.write
-- handles the directory creation. setIdentity (called from love.load
-- when _config().game_id is set) is what makes the path game-specific.
local SAVE_FILE = "save.json"

function usagi.save(t)
  local s = json_encode(t)
  local ok, err = love.filesystem.write(SAVE_FILE, s)
  if not ok then error("usagi.save: " .. tostring(err), 2) end
end

function usagi.load()
  if not love.filesystem.getInfo(SAVE_FILE) then return nil end
  local data = love.filesystem.read(SAVE_FILE)
  if not data then return nil end
  return json_decode(data)
end

-- usagi.dump — pretty-print any Lua value to a string. Ported from
-- runtime/usagi.lua; pure Lua, identical output to Usagi.
local function format_key(k)
  if type(k) == "string" and k:match("^[%a_][%w_]*$") then
    return k
  end
  return "[" .. tostring(k) .. "]"
end

local function is_array_like(t)
  local n = 0
  for _ in pairs(t) do n = n + 1 end
  for i = 1, n do
    if t[i] == nil then return false, n end
  end
  return true, n
end

local function dump_value(v, indent, seen)
  local t = type(v)
  if t == "nil" then return "nil" end
  if t == "boolean" or t == "number" then return tostring(v) end
  if t == "string" then return string.format("%q", v) end
  if t == "function" then return "<function>" end
  if t == "userdata" then return "<userdata>" end
  if t == "thread" then return "<thread>" end
  if t == "table" then
    if seen[v] then return "<cycle>" end
    seen[v] = true
    local is_arr, n = is_array_like(v)
    if n == 0 then
      seen[v] = nil
      return "{}"
    end
    local indent2 = indent .. "  "
    local parts = {}
    if is_arr then
      for i = 1, n do
        parts[#parts + 1] = indent2 .. dump_value(v[i], indent2, seen)
      end
    else
      local keys = {}
      for k in pairs(v) do keys[#keys + 1] = k end
      table.sort(keys, function(a, b) return tostring(a) < tostring(b) end)
      for _, k in ipairs(keys) do
        parts[#parts + 1] = indent2 .. format_key(k) .. " = " .. dump_value(v[k], indent2, seen)
      end
    end
    seen[v] = nil
    return "{\n" .. table.concat(parts, ",\n") .. ",\n" .. indent .. "}"
  end
  return "<" .. t .. ">"
end

function usagi.dump(v)
  return dump_value(v, "", {})
end

-- util module (ported from runtime/util.lua) -------------------------------

local util = {}

local function assert_shape(value, fields, fn_name, arg_idx)
  if type(value) ~= "table" then
    error(string.format("util.%s: arg %d must be a table, got %s",
      fn_name, arg_idx, type(value)), 3)
  end
  for _, f in ipairs(fields) do
    if type(value[f]) ~= "number" then
      error(string.format(
        "util.%s: arg %d table missing or non-numeric field '%s'",
        fn_name, arg_idx, f), 3)
    end
  end
end

function util.clamp(v, lo, hi)
  if v < lo then return lo end
  if v > hi then return hi end
  return v
end

function util.sign(v)
  if v > 0 then return 1 end
  if v < 0 then return -1 end
  return 0
end

function util.round(v)
  return math.floor(v + 0.5)
end

function util.approach(current, target, max_delta)
  if current < target then
    return math.min(current + max_delta, target)
  elseif current > target then
    return math.max(current - max_delta, target)
  end
  return current
end

function util.flash(t, hz)
  return math.floor(t * hz) % 2 == 0
end

function util.lerp(a, b, t)
  return a + (b - a) * t
end

function util.wrap(v, lo, hi)
  local span = hi - lo
  return ((v - lo) % span) + lo
end

function util.remap(v, start_a, end_a, start_b, end_b)
  return (v - start_a) / (end_a - start_a) * (end_b - start_b) + start_b
end

function util.vec_normalize(v)
  assert_shape(v, { "x", "y" }, "vec_normalize", 1)
  local len = math.sqrt(v.x * v.x + v.y * v.y)
  if len == 0 then return { x = 0, y = 0 } end
  return { x = v.x / len, y = v.y / len }
end

function util.vec_dist(a, b)
  assert_shape(a, { "x", "y" }, "vec_dist", 1)
  assert_shape(b, { "x", "y" }, "vec_dist", 2)
  local dx = a.x - b.x
  local dy = a.y - b.y
  return math.sqrt(dx * dx + dy * dy)
end

function util.vec_dist_sq(a, b)
  assert_shape(a, { "x", "y" }, "vec_dist_sq", 1)
  assert_shape(b, { "x", "y" }, "vec_dist_sq", 2)
  local dx = a.x - b.x
  local dy = a.y - b.y
  return dx * dx + dy * dy
end

function util.vec_from_angle(angle, len)
  len = len or 1
  return { x = math.cos(angle) * len, y = math.sin(angle) * len }
end

function util.point_in_rect(p, r)
  assert_shape(p, { "x", "y" }, "point_in_rect", 1)
  assert_shape(r, { "x", "y", "w", "h" }, "point_in_rect", 2)
  return p.x >= r.x and p.x < r.x + r.w
    and p.y >= r.y and p.y < r.y + r.h
end

function util.point_in_circ(p, c)
  assert_shape(p, { "x", "y" }, "point_in_circ", 1)
  assert_shape(c, { "x", "y", "r" }, "point_in_circ", 2)
  local dx = p.x - c.x
  local dy = p.y - c.y
  return dx * dx + dy * dy < c.r * c.r
end

function util.rect_overlap(a, b)
  assert_shape(a, { "x", "y", "w", "h" }, "rect_overlap", 1)
  assert_shape(b, { "x", "y", "w", "h" }, "rect_overlap", 2)
  return a.x < b.x + b.w
      and b.x < a.x + a.w
      and a.y < b.y + b.h
      and b.y < a.y + a.h
end

function util.circ_overlap(a, b)
  assert_shape(a, { "x", "y", "r" }, "circ_overlap", 1)
  assert_shape(b, { "x", "y", "r" }, "circ_overlap", 2)
  local dx = a.x - b.x
  local dy = a.y - b.y
  local rsum = a.r + b.r
  return dx * dx + dy * dy < rsum * rsum
end

function util.circ_rect_overlap(c, r)
  assert_shape(c, { "x", "y", "r" }, "circ_rect_overlap", 1)
  assert_shape(r, { "x", "y", "w", "h" }, "circ_rect_overlap", 2)
  local cx = util.clamp(c.x, r.x, r.x + r.w)
  local cy = util.clamp(c.y, r.y, r.y + r.h)
  local dx = c.x - cx
  local dy = c.y - cy
  return dx * dx + dy * dy < c.r * c.r
end

-- sfx module ---------------------------------------------------------------
-- Loads sfx/<name>.wav lazily on first play. Each effect gets a pool of
-- 8 sources cloned from one decoded buffer; the pool advances
-- round-robin so a rapidly retriggered sfx overlaps (the 9th play in a
-- row steals the oldest voice). Matches Usagi's overlap semantics.

local sfx = {}

local SFX_POOL_SIZE = 8
local sfx_pools = {}

local function load_sfx(name)
  local path = "sfx/" .. name .. ".wav"
  if not love.filesystem.getInfo(path) then return nil end
  local src = love.audio.newSource(path, "static")
  local pool = { sources = { src }, next_idx = 1 }
  for i = 2, SFX_POOL_SIZE do
    pool.sources[i] = src:clone()
  end
  sfx_pools[name] = pool
  return pool
end

-- Pick a source for the next play. Prefer any idle voice (matches
-- Usagi's "overlap, don't cut" semantic); only when all 8 are busy do
-- we steal via round-robin (the "9th play steals oldest" rule).
-- Stopping a still-playing source mid-sample causes an audible click,
-- so finding an idle voice first avoids the crunchiness rapid plays
-- would otherwise produce.
local function pick_sfx_voice(pool)
  for _, src in ipairs(pool.sources) do
    if not src:isPlaying() then return src end
  end
  local src = pool.sources[pool.next_idx]
  src:stop()
  pool.next_idx = pool.next_idx % SFX_POOL_SIZE + 1
  return src
end

function sfx.play(name)
  local pool = sfx_pools[name] or load_sfx(name)
  if not pool then return end
  local src = pick_sfx_voice(pool)
  src:setVolume(1.0)
  src:setPitch(1.0)
  src:play()
end

-- Pan is silently ignored on Love 11.5; there's no direct setPan on
-- sources until Love 12. If a port really needs stereo pan, swap the
-- source to a stereo buffer and use setPosition(x, 0, 0).
function sfx.play_ex(name, volume, pitch, _pan)
  local pool = sfx_pools[name] or load_sfx(name)
  if not pool then return end
  local src = pick_sfx_voice(pool)
  src:setVolume(volume or 1.0)
  src:setPitch(pitch or 1.0)
  src:play()
end

function sfx.stop(name)
  local pool = sfx_pools[name]
  if not pool then return end
  for _, src in ipairs(pool.sources) do
    src:stop()
  end
end

function sfx.stop_all()
  for _, pool in pairs(sfx_pools) do
    for _, src in ipairs(pool.sources) do
      src:stop()
    end
  end
end

function sfx.is_playing(name)
  local pool = sfx_pools[name]
  if not pool then return false end
  for _, src in ipairs(pool.sources) do
    if src:isPlaying() then return true end
  end
  return false
end

-- music module -------------------------------------------------------------
-- One streaming source at a time. Subsequent music.play* calls stop the
-- previous track.

local music = {}

local MUSIC_EXTS = { "ogg", "mp3", "wav", "flac" }
local music_sources = {}
local current_music

local function load_music(name)
  local cached = music_sources[name]
  if cached then return cached end
  for _, ext in ipairs(MUSIC_EXTS) do
    local path = "music/" .. name .. "." .. ext
    if love.filesystem.getInfo(path) then
      local src = love.audio.newSource(path, "stream")
      music_sources[name] = src
      return src
    end
  end
  return nil
end

function music.stop()
  if current_music then
    current_music:stop()
    current_music = nil
  end
end

function music.play(name)
  music.stop()
  local src = load_music(name)
  if not src then return end
  src:setVolume(1.0)
  src:setPitch(1.0)
  src:setLooping(false)
  src:play()
  current_music = src
end

function music.loop(name)
  music.stop()
  local src = load_music(name)
  if not src then return end
  src:setVolume(1.0)
  src:setPitch(1.0)
  src:setLooping(true)
  src:play()
  current_music = src
end

function music.play_ex(name, volume, pitch, _pan, looping)
  music.stop()
  local src = load_music(name)
  if not src then return end
  src:setVolume(volume or 1.0)
  src:setPitch(pitch or 1.0)
  src:setLooping(looping == true)
  src:play()
  current_music = src
end

function music.mutate(volume, pitch, _pan)
  if not current_music then return end
  if volume then current_music:setVolume(volume) end
  if pitch then current_music:setPitch(pitch) end
end

-- effect module -----------------------------------------------------------
-- Engine-level juice primitives. Stacking rule across all four:
-- longer duration wins; for the magnitude param the latest call wins.
-- That way `effect.screen_shake(0.1, 2)` followed by
-- `effect.screen_shake(0.5, 4)` gives 0.5s at intensity 4 (the union
-- of both calls), and spam-calling is safe.
--
-- Timers decay with real wall-clock dt (not affected by slow_mo).
-- Hitstop freezes `_update` for its duration; `_draw` and effect decay
-- continue. Shake offset is applied to the canvas blit position so the
-- whole rendered frame rattles. Flash is a full-screen color overlay
-- drawn after `_draw` so the player can still see the game underneath
-- the fading tint.

local effect = {}

local efx = {
  hitstop_left = 0,
  shake_left = 0, shake_total = 0, shake_intensity = 0,
  flash_left = 0, flash_total = 0, flash_color = 0,
  slow_mo_left = 0, slow_mo_scale = 1,
}

function effect.hitstop(time)
  local t = math.max(0, time or 0)
  if t > efx.hitstop_left then efx.hitstop_left = t end
end

function effect.screen_shake(time, intensity)
  local t = math.max(0, time or 0)
  if t > efx.shake_left then
    efx.shake_left = t
    efx.shake_total = t
  end
  efx.shake_intensity = math.max(0, intensity or 0)
end

function effect.flash(time, color)
  local t = math.max(0, time or 0)
  if t > efx.flash_left then
    efx.flash_left = t
    efx.flash_total = t
  end
  efx.flash_color = color or 0
end

function effect.slow_mo(time, scale)
  local t = math.max(0, time or 0)
  if t > efx.slow_mo_left then efx.slow_mo_left = t end
  efx.slow_mo_scale = math.max(0, scale or 0)
end

function effect.stop()
  efx.hitstop_left = 0
  efx.shake_left = 0; efx.shake_total = 0; efx.shake_intensity = 0
  efx.flash_left = 0; efx.flash_total = 0; efx.flash_color = 0
  efx.slow_mo_left = 0; efx.slow_mo_scale = 1
end

-- Internal helpers used by love.update / love.draw.

local function effect_tick(dt)
  efx.hitstop_left = math.max(0, efx.hitstop_left - dt)
  efx.shake_left = math.max(0, efx.shake_left - dt)
  efx.flash_left = math.max(0, efx.flash_left - dt)
  efx.slow_mo_left = math.max(0, efx.slow_mo_left - dt)
  if efx.slow_mo_left == 0 then efx.slow_mo_scale = 1 end
end

local function effect_shake_offset()
  if efx.shake_left <= 0 or efx.shake_total <= 0 then return 0, 0 end
  local decay = efx.shake_left / efx.shake_total
  local mag = efx.shake_intensity * decay
  local angle = math.random() * math.pi * 2
  return math.cos(angle) * mag, math.sin(angle) * mag
end

-- Install globals -----------------------------------------------------------

_G.gfx = gfx
_G.usagi = usagi
_G.util = util
_G.input = input
_G.sfx = sfx
_G.music = music
_G.effect = effect

-- Callback wrapping ---------------------------------------------------------

local function recompute_scale()
  local ww, wh = love.graphics.getDimensions()
  local sx = ww / usagi.GAME_W
  local sy = wh / usagi.GAME_H
  scale = math.min(sx, sy)
  if pixel_perfect then
    scale = math.max(1, math.floor(scale))
  end
  letterbox_x = math.floor((ww - usagi.GAME_W * scale) / 2)
  letterbox_y = math.floor((wh - usagi.GAME_H * scale) / 2)
end

function love.load()
  local cfg = {}
  if type(_G._config) == "function" then
    cfg = _G._config() or {}
  end

  usagi.GAME_W = cfg.game_width or usagi.GAME_W
  usagi.GAME_H = cfg.game_height or usagi.GAME_H
  usagi.SPRITE_SIZE = cfg.sprite_size or usagi.SPRITE_SIZE
  pixel_perfect = cfg.pixel_perfect == true

  -- game_id namespaces the save dir under Love's identity system.
  -- Called early so any usagi.save / load in _init resolves correctly.
  if cfg.game_id then
    love.filesystem.setIdentity(cfg.game_id)
  end

  if cfg.name then love.window.setTitle(cfg.name) end

  -- Starting window size: the game resolution at the largest integer scale
  -- that fits the primary display within 0.66 of each axis (matching the
  -- engine).
  local dw, dh = love.window.getDesktopDimensions()
  local target_scale = math.max(
    1,
    math.min(
      math.floor(dw * 0.66 / usagi.GAME_W),
      math.floor(dh * 0.66 / usagi.GAME_H)
    )
  )
  love.window.setMode(usagi.GAME_W * target_scale, usagi.GAME_H * target_scale, {
    resizable = true,
    vsync = true,
    fullscreen = cfg.initial_fullscreen == true,
    fullscreentype = "desktop",
  })

  love.graphics.setDefaultFilter("nearest", "nearest")
  -- Disable line/polygon edge anti-aliasing. Affects line(), polygon("line"),
  -- rectangle("line"), circle("line"). Filled primitives don't anti-alias.
  love.graphics.setLineStyle("rough")
  -- msaa=0 keeps the canvas from getting multi-sampled, which would soften
  -- pixel-perfect content during the upscale blit.
  canvas = love.graphics.newCanvas(usagi.GAME_W, usagi.GAME_H, { msaa = 0 })
  canvas:setFilter("nearest", "nearest")
  -- Love's default font was created before this love.load ran, so it still
  -- has the default linear filter; force nearest so the cached glyph atlas
  -- isn't filtered when the canvas is upscaled.
  love.graphics.getFont():setFilter("nearest", "nearest")

  recompute_scale()
  refresh_gamepads()
  load_palette()
  load_custom_font()

  -- Window icon from a sprites.png tile, mirroring Usagi's _config().icon.
  -- Loads sprites.png if not yet cached, slices out the requested cell,
  -- and hands it to Love. No-op on web (love.window.setIcon is desktop).
  if cfg.icon and love.filesystem.getInfo("sprites.png") then
    local sheet = love.image.newImageData("sprites.png")
    local sz = usagi.SPRITE_SIZE
    local cols = math.floor(sheet:getWidth() / sz)
    local col = (cfg.icon - 1) % cols
    local row = math.floor((cfg.icon - 1) / cols)
    local icon = love.image.newImageData(sz, sz)
    icon:paste(sheet, 0, 0, col * sz, row * sz, sz, sz)
    love.window.setIcon(icon)
  end

  if type(_G._init) == "function" then
    _G._init()
  end
end

function love.resize(_w, _h)
  recompute_scale()
end

function love.update(dt)
  -- Cap dt so a stalled loop (window unfocused, machine slept) resumes with
  -- one normal step instead of a huge jump. Matches the engine.
  dt = math.min(dt, 0.1)
  usagi.elapsed = usagi.elapsed + dt
  effect_tick(dt)
  update_current_source()
  -- Hitstop freezes _update entirely; slow_mo scales dt. Both decay
  -- on real wall-clock dt above so the effect still expires while
  -- frozen.
  if efx.hitstop_left == 0 and type(_G._update) == "function" then
    local scaled_dt = efx.slow_mo_left > 0 and (dt * efx.slow_mo_scale) or dt
    _G._update(scaled_dt)
  end
  -- Snapshot per-action state for next frame's pressed/released checks.
  -- Done at the tail of update so input.pressed within _update compares
  -- "now" against "start of this frame".
  for action = input.LEFT, input.BTN3 do
    last_action_state[action] = action_held(action)
  end
  -- Same trick for tracked keys and the three mouse buttons.
  for key in pairs(tracked_keys) do
    last_key_state[key] = love.keyboard.isDown(key)
  end
  last_mouse_state[1] = love.mouse.isDown(1)
  last_mouse_state[2] = love.mouse.isDown(2)
  last_mouse_state[3] = love.mouse.isDown(3)
end

function love.draw()
  love.graphics.setCanvas(canvas)
  love.graphics.clear(0, 0, 0, 1)
  if type(_G._draw) == "function" then
    _G._draw(math.min(love.timer.getDelta(), 0.1))
  end
  -- Flash overlay: drawn on the canvas after _draw so it scales
  -- nearest-neighbor with the rest of the frame. Alpha decays linearly
  -- from 1.0 to 0 over the flash duration.
  if efx.flash_left > 0 and efx.flash_total > 0 then
    local alpha = efx.flash_left / efx.flash_total
    set_color(efx.flash_color, alpha)
    love.graphics.rectangle("fill", 0, 0, usagi.GAME_W, usagi.GAME_H)
  end
  -- Snapshot the just-rendered frame for next frame's gfx.get_px. Only
  -- pay the GPU->CPU download cost if the game has actually called
  -- gfx.get_px at least once.
  if snapshot_requested then
    screen_snapshot = canvas:newImageData()
  end
  love.graphics.setCanvas()
  -- Screen shake nudges the canvas blit position. Multiply by scale so
  -- a 4-pixel intensity reads as 4 game pixels regardless of window
  -- size.
  local shx, shy = effect_shake_offset()
  -- Mirrors Usagi: letterbox bars stay black (the window's default
  -- backbuffer color), but when shake is active we fill the unshaken
  -- game viewport with the last gfx.clear color first. The shifted
  -- canvas blit covers most of that fill, so the only visible part is
  -- the strip exposed at the offset edge, which reads as the game's bg
  -- instead of letterbox black. When no shake is active the canvas
  -- blit covers the full viewport and the fill is unneeded.
  if shx ~= 0 or shy ~= 0 then
    love.graphics.setColor(
      last_clear_color[1], last_clear_color[2], last_clear_color[3], 1.0)
    love.graphics.rectangle("fill",
      letterbox_x, letterbox_y,
      usagi.GAME_W * scale, usagi.GAME_H * scale)
  end
  love.graphics.setColor(1, 1, 1, 1)
  love.graphics.draw(canvas,
    letterbox_x + shx * scale, letterbox_y + shy * scale,
    0, scale, scale)
  -- Per-frame scroll resets at the tail of draw so _update and _draw
  -- in the same frame see the same value, then the next frame's
  -- love.wheelmoved events accumulate from zero.
  scroll_delta = 0.0
end
