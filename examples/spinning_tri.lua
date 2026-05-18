-- Triangles have no _ex variant with a rotation param, so to spin
-- one you compute each rotated vertex yourself and pass the six
-- coordinates to gfx.tri / gfx.tri_fill. The fill primitive
-- auto-corrects vertex winding, so the triangle stays solid as it
-- rotates through every orientation.

function _config()
  return { name = "Spinning Triangle" }
end

local TWO_PI = math.pi * 2

-- Three corners of an equilateral triangle of `size` around (cx, cy),
-- with corner 0 pointing straight up at angle=0. Returns six numbers
-- ready to splat into gfx.tri / gfx.tri_fill.
local function tri_points(cx, cy, size, angle)
  local function corner(i)
    local a = angle + i * TWO_PI / 3
    return cx + math.sin(a) * size, cy - math.cos(a) * size
  end
  local x1, y1 = corner(0)
  local x2, y2 = corner(1)
  local x3, y3 = corner(2)
  return x1, y1, x2, y2, x3, y3
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_BLACK)

  local cx, cy = usagi.GAME_W / 2, usagi.GAME_H / 2
  local angle = usagi.elapsed * 2 -- radians per second

  -- Big filled triangle in the middle. As `angle` walks through 0..2pi,
  -- the same three vertices alternate between CW and CCW screen order;
  -- gfx.tri_fill keeps it solid the whole way around.
  local x1, y1, x2, y2, x3, y3 = tri_points(cx, cy, 40, angle)
  gfx.tri_fill(x1, y1, x2, y2, x3, y3, gfx.COLOR_PINK)

  -- Smaller outline spinning the other direction.
  x1, y1, x2, y2, x3, y3 = tri_points(60, cy, 22, -angle * 1.4)
  gfx.tri(x1, y1, x2, y2, x3, y3, gfx.COLOR_GREEN)

  -- And a fast filled one on the right.
  x1, y1, x2, y2, x3, y3 = tri_points(usagi.GAME_W - 60, cy, 22, angle * 2.2)
  gfx.tri_fill(x1, y1, x2, y2, x3, y3, gfx.COLOR_ORANGE)

  gfx.text("spinning triangles", 4, 4, gfx.COLOR_WHITE)
end
