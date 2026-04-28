-- Pico-8-flavored helpers for usagi. Calling `require "pico8"` installs
-- bare globals (`spr`, `cls`, `rectfill`, `btn`, `flr`, ...) that wrap
-- usagi's `gfx` / `input` / `math` APIs, so muscle memory from Pico-8
-- carries over.
--
-- This is NOT a Pico-8 compatibility layer. It covers the subset that
-- maps cleanly onto usagi's current API:
--
--   covered: cls, rect, rectfill, circ, circfill, line, print, spr,
--            btn, btnp, flr, ceil, abs, min, max, mid, sqrt, cos, sin,
--            rnd, srand
--
--   missing: music, pset, palette swap (pal/palt), camera, clip, fillp,
--            peek/poke, map, sspr, mget/mset, sfx (index-keyed; usagi
--            sfx are name-keyed), atan2, time/t.
--
-- Notable behaviors preserved from Pico-8:
--   * `spr(0)` draws the first sprite (Pico-8 is 0-based; usagi is
--     1-based — the wrapper adds 1).
--   * `cos`/`sin` take turns, not radians, and `sin` is negated to
--     match Pico-8's screen-Y-down convention.
--   * `rect`/`rectfill` use inclusive corner coordinates (x0,y0,x1,y1)
--     while usagi uses (x,y,w,h).
--   * `print` shadows Lua's stdout `print`. If you need stdout from a
--     pico8 example, use `io.write`.

local function rect_args(x0, y0, x1, y1)
  return x0, y0, x1 - x0 + 1, y1 - y0 + 1
end

cls = function(c)
  gfx.clear(c or gfx.COLOR_BLACK)
end

rect = function(x0, y0, x1, y1, c)
  local x, y, w, h = rect_args(x0, y0, x1, y1)
  gfx.rect(x, y, w, h, c)
end

rectfill = function(x0, y0, x1, y1, c)
  local x, y, w, h = rect_args(x0, y0, x1, y1)
  gfx.rect_fill(x, y, w, h, c)
end

circ = function(x, y, r, c)
  gfx.circ(x, y, r, c)
end

circfill = function(x, y, r, c)
  gfx.circ_fill(x, y, r, c)
end

line = function(x0, y0, x1, y1, c)
  gfx.line(x0, y0, x1, y1, c)
end

print = function(s, x, y, c)
  gfx.text(tostring(s), x or 0, y or 0, c or gfx.COLOR_WHITE)
end

spr = function(n, x, y)
  gfx.spr(n + 1, x, y)
end

local BTN_TO_ACTION = {
  [0] = input.LEFT,
  [1] = input.RIGHT,
  [2] = input.UP,
  [3] = input.DOWN,
  [4] = input.BTN1,
  [5] = input.BTN2,
}

btn = function(b)
  local a = BTN_TO_ACTION[b]
  if a == nil then
    return false
  end
  return input.down(a)
end

btnp = function(b)
  local a = BTN_TO_ACTION[b]
  if a == nil then
    return false
  end
  return input.pressed(a)
end

flr = math.floor
ceil = math.ceil
abs = math.abs
min = math.min
max = math.max
sqrt = math.sqrt

mid = function(a, b, c)
  if a > b then
    a, b = b, a
  end
  if b > c then
    b = c
  end
  if a > b then
    b = a
  end
  return b
end

local TAU = math.pi * 2

cos = function(x)
  return math.cos(x * TAU)
end

sin = function(x)
  return -math.sin(x * TAU)
end

rnd = function(x)
  if x == nil then
    return math.random()
  end
  if type(x) == "table" then
    return x[math.random(1, #x)]
  end
  return math.random() * x
end

srand = function(s)
  math.randomseed(s)
end

return {}
