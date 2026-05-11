-- Pico-8-flavored helpers for usagi. Calling `require "pico8"` installs
-- bare globals (`spr`, `cls`, `rectfill`, `btn`, `flr`, ...) that wrap
-- usagi's `gfx` / `input` / `math` APIs, so muscle memory from Pico-8
-- carries over.
--
-- This is NOT a Pico-8 compatibility layer. It covers the subset that
-- maps cleanly onto usagi's current API:
--
--   covered: cls, rect, rectfill, circ, circfill, line, print, pset,
--            spr (with flip args), sspr (with stretch + flip args), btn,
--            btnp, flr, ceil, abs, min, max, mid, sqrt, cos, sin, rnd,
--            srand, time, t
--
--   missing: music, palette swap (pal/palt), camera, clip, fillp,
--            peek/poke, map, mget/mset, sfx (index-keyed; usagi sfx are
--            name-keyed), atan2.
--
-- Notable behaviors preserved from Pico-8:
--   * `spr(0)` draws the first sprite (Pico-8 is 0-based; usagi is
--     1-based — the wrapper adds 1).
--   * Colors are 0-based in Pico-8 (`rectfill(..., 8)` is red) but
--     1-based in usagi (`gfx.rect_fill(..., 9)` is red). The wrappers
--     bump each color arg by 1 on the way through.
--   * `cos`/`sin` take turns, not radians, and `sin` is negated to
--     match Pico-8's screen-Y-down convention.
--   * `rect`/`rectfill` use inclusive corner coordinates (x0,y0,x1,y1)
--     while usagi uses (x,y,w,h).
--   * `print` shadows Lua's stdout `print`. If you need stdout from a
--     pico8 example, use `io.write`.

local function rect_args(x0, y0, x1, y1)
  return x0, y0, x1 - x0 + 1, y1 - y0 + 1
end

-- Pico-8 color index (0-15) → usagi slot (1-16). Pico-8 carts pass
-- raw integers everywhere; usagi's named constants would deviate from
-- the source. The +1 keeps the cart's literals working unchanged.
local function pc(c)
  return c + 1
end

cls = function(c)
  gfx.clear(c and pc(c) or gfx.COLOR_BLACK)
end

rect = function(x0, y0, x1, y1, c)
  local x, y, w, h = rect_args(x0, y0, x1, y1)
  gfx.rect(x, y, w, h, pc(c))
end

rectfill = function(x0, y0, x1, y1, c)
  local x, y, w, h = rect_args(x0, y0, x1, y1)
  gfx.rect_fill(x, y, w, h, pc(c))
end

circ = function(x, y, r, c)
  gfx.circ(x, y, r, pc(c))
end

circfill = function(x, y, r, c)
  gfx.circ_fill(x, y, r, pc(c))
end

line = function(x0, y0, x1, y1, c)
  -- Pico-8 treats `line(x,y,x,y)` as a single-pixel draw because its
  -- endpoints are inclusive; raylib's DrawLine on a zero-length line
  -- draws nothing. Bridge that with gfx.pixel to keep `for star in
  -- ... line(sx, sy, sx, sy, c) end` working as Pico-8 users expect.
  if x0 == x1 and y0 == y1 then
    gfx.pixel(x0, y0, pc(c))
  else
    gfx.line(x0, y0, x1, y1, pc(c))
  end
end

print = function(s, x, y, c)
  gfx.text(tostring(s), x or 0, y or 0, c and pc(c) or gfx.COLOR_WHITE)
end

-- Pico-8 `spr(n, x, y, [w, h, [flip_x, [flip_y]]])`. Usagi's spr is a
-- fixed (idx, x, y); the extended `spr_ex` adds required flip args. We
-- forward to whichever fits the call. The Pico-8 `w, h` (multi-tile)
-- args are ignored — usagi doesn't have multi-tile sprite draws yet;
-- pass single-tile indices.
spr = function(n, x, y, _w, _h, flip_x, flip_y)
  if flip_x or flip_y then
    -- usagi's spr_ex has more knobs than Pico-8 ever exposed; pass the
    -- identity values for rotation / tint / alpha so this wrapper
    -- behaves like Pico-8.
    gfx.spr_ex(n + 1, x, y, flip_x or false, flip_y or false, 0, gfx.COLOR_WHITE, 1.0)
  else
    gfx.spr(n + 1, x, y)
  end
end

-- Pico-8 `sspr(sx, sy, sw, sh, dx, dy, [dw, [dh, [flip_x, [flip_y]]]])`.
-- Forwards to `gfx.sspr` for plain 1:1 draws and `gfx.sspr_ex` when any
-- power arg (dest size or flip) is given.
sspr = function(sx, sy, sw, sh, dx, dy, dw, dh, flip_x, flip_y)
  local plain = dw == nil and dh == nil and not flip_x and not flip_y
  if plain then
    gfx.sspr(sx, sy, sw, sh, dx, dy)
  else
    gfx.sspr_ex(
      sx, sy, sw, sh, dx, dy,
      dw or sw, dh or sh,
      flip_x or false, flip_y or false,
      0, gfx.COLOR_WHITE, 1.0
    )
  end
end

pset = function(x, y, c)
  gfx.pixel(x, y, pc(c))
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
  return input.held(a)
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

-- Pico-8 exposes both `time()` and the shorthand `t()`; both return
-- seconds since the cart started running. Maps to `usagi.elapsed`.
time = function()
  return usagi.elapsed
end
t = time

return {}
