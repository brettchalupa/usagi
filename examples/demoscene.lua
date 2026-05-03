-- USAGI demoscene demo: a spinning 3D wireframe cube,
-- a 3D starfield streaming past the camera, a color-cycling
-- sine-wave title, and a classic rasterbar at the bottom.

local W, H = usagi.GAME_W, usagi.GAME_H
local CX, CY = W / 2, H / 2

local FOV = 150
local CUBE_SCALE = 0.95

local CUBE_V = {
  { -1, -1, -1 },
  { 1,  -1, -1 },
  { 1,  1,  -1 },
  { -1, 1,  -1 },
  { -1, -1, 1 },
  { 1,  -1, 1 },
  { 1,  1,  1 },
  { -1, 1,  1 },
}
local CUBE_E = {
  { 1, 2 },
  { 2, 3 },
  { 3, 4 },
  { 4, 1 },
  { 5, 6 },
  { 6, 7 },
  { 7, 8 },
  { 8, 5 },
  { 1, 5 },
  { 2, 6 },
  { 3, 7 },
  { 4, 8 },
}

local PALETTE_CYCLE = {
  gfx.COLOR_RED,
  gfx.COLOR_ORANGE,
  gfx.COLOR_YELLOW,
  gfx.COLOR_GREEN,
  gfx.COLOR_BLUE,
  gfx.COLOR_INDIGO,
  gfx.COLOR_PINK,
  gfx.COLOR_PEACH,
}

local TITLE = "USAGI"
local CHAR_W = 4

local NUM_STARS = 90

function _config()
  return { title = "Usagi Demo" }
end

local function spawn_star(z)
  return {
    x = (math.random() - 0.5) * 5,
    y = (math.random() - 0.5) * 3,
    z = z or (0.4 + math.random() * 4.6),
  }
end

function _init()
  State = { t = 0, stars = {} }
  for i = 1, NUM_STARS do
    State.stars[i] = spawn_star()
  end
end

local function rot_x(p, a)
  local s, c = math.sin(a), math.cos(a)
  return { p[1], c * p[2] - s * p[3], s * p[2] + c * p[3] }
end

local function rot_y(p, a)
  local s, c = math.sin(a), math.cos(a)
  return { c * p[1] + s * p[3], p[2], -s * p[1] + c * p[3] }
end

local function rot_z(p, a)
  local s, c = math.sin(a), math.cos(a)
  return { c * p[1] - s * p[2], s * p[1] + c * p[2], p[3] }
end

-- Camera sits at z = -3.5 looking down +z; smaller z = closer to viewer.
local function project(x, y, z)
  local zc = z + 3.5
  if zc < 0.2 then
    zc = 0.2
  end
  local f = FOV / zc
  return CX + x * f, CY + y * f, zc
end

local function star_color(z)
  if z < 1.2 then
    return gfx.COLOR_WHITE
  end
  if z < 2.2 then
    return gfx.COLOR_LIGHT_GRAY
  end
  if z < 3.2 then
    return gfx.COLOR_INDIGO
  end
  return gfx.COLOR_DARK_BLUE
end

function _update(dt)
  State.t = State.t + dt
  for i, s in ipairs(State.stars) do
    s.z = s.z - dt * 1.8
    if s.z < 0.1 then
      State.stars[i] = spawn_star(5.0)
    end
  end
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_BLACK)

  -- Starfield: closer State.stars are brighter and fatter. Distant State.stars get
  -- a single-pixel `gfx.pixel`; near State.stars upgrade to a 2x2 rect_fill.
  for _, s in ipairs(State.stars) do
    local sx, sy, sz = project(s.x, s.y, s.z)
    if sx >= -2 and sx < W + 2 and sy >= -2 and sy < H + 2 then
      if sz < 1.4 then
        gfx.rect_fill(sx, sy, 2, 2, star_color(sz))
      else
        gfx.pixel(sx, sy, star_color(sz))
      end
    end
  end

  -- Tumbling wireframe cube with rainbow edges.
  local rotated = {}
  for i, v in ipairs(CUBE_V) do
    local p = { v[1] * CUBE_SCALE, v[2] * CUBE_SCALE, v[3] * CUBE_SCALE }
    p = rot_x(p, State.t * 0.7)
    p = rot_y(p, State.t * 0.95)
    p = rot_z(p, State.t * 0.4)
    rotated[i] = p
  end

  for i, e in ipairs(CUBE_E) do
    local a, b = rotated[e[1]], rotated[e[2]]
    local x1, y1 = project(a[1], a[2], a[3])
    local x2, y2 = project(b[1], b[2], b[3])
    local idx = (math.floor(State.t * 4) + i - 1) % #PALETTE_CYCLE + 1
    gfx.line(x1, y1, x2, y2, PALETTE_CYCLE[idx])
  end

  for _, p in ipairs(rotated) do
    local x, y = project(p[1], p[2], p[3])
    gfx.circ_fill(x, y, 1, gfx.COLOR_WHITE)
  end

  -- Orbiting bubbles, four phases offset around the cube.
  for i = 0, 3 do
    local angle = State.t * 1.5 + i * math.pi / 2
    local ox = math.cos(angle) * 1.7
    local oz = math.sin(angle) * 1.7
    local oy = math.sin(angle * 0.5 + i) * 0.7
    local sx, sy, sz = project(ox, oy, oz)
    local r = math.max(1, math.floor(5 - sz))
    local idx = (i + math.floor(State.t * 3)) % #PALETTE_CYCLE + 1
    gfx.circ_fill(sx, sy, r, PALETTE_CYCLE[idx])
  end

  -- Sine-wave, color-cycling title.
  local base_x = CX - (#TITLE * CHAR_W) / 2
  for i = 1, #TITLE do
    local ch = string.sub(TITLE, i, i)
    local x = base_x + (i - 1) * CHAR_W
    local y = 12 + math.sin(State.t * 3 + i * 0.7) * 4
    local idx = (math.floor(State.t * 8) + i - 1) % #PALETTE_CYCLE + 1
    gfx.text(ch, x, y, PALETTE_CYCLE[idx])
  end

  gfx.text("demo", CX - 8, 28, gfx.COLOR_LIGHT_GRAY)

  -- Bottom rasterbar for that demoscene flavor.
  for y = 0, 5 do
    local idx = (math.floor(State.t * 12) + y) % #PALETTE_CYCLE + 1
    gfx.rect_fill(0, H - 6 + y, W, 1, PALETTE_CYCLE[idx])
  end
end
