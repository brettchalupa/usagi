-- Pico-8-flavored demo: requires the local `pico8` module to install
-- bare globals (cls, spr, btn, btnp, print, ...), then writes the rest
-- of the game in Pico-8 style. See pico8.lua for what's covered and
-- what's intentionally missing.

require "pico8"

local SPR = {
  BUNNY = 0,
  SHIP = 1,
  BULLET_LG = 2,
  BULLET_SM = 3,
}

-- Warm palette cycle for the ship's exhaust trail.
local EXHAUST_COLORS = {
  gfx.COLOR_YELLOW,
  gfx.COLOR_ORANGE,
  gfx.COLOR_RED,
  gfx.COLOR_BROWN,
}

function _config()
  return { name = "Pico-8 flavor" }
end

function _init()
  State = {
    p = { x = 50, y = 80, spd = 200, face_left = false },
    count = 0,
    spin = 0,
    sparks = {},
  }
end

local function emit_spark()
  -- Ship is 16×16 and points up (top-down view), so the exhaust
  -- spawns at the bottom edge and trails downward.
  local tail_x = State.p.x + 6 + flr(rnd(4))
  local tail_y = State.p.y + 16
  State.sparks[#State.sparks + 1] = {
    x = tail_x,
    y = tail_y,
    vx = rnd(20) - 10,
    vy = 40 + rnd(40),
    life = 0.4 + rnd(0.3),
    color = EXHAUST_COLORS[1 + flr(rnd(#EXHAUST_COLORS))],
  }
end

function _update(dt)
  if btn(0) then
    State.p.x = State.p.x - State.p.spd * dt
    State.p.face_left = true
  end
  if btn(1) then
    State.p.x = State.p.x + State.p.spd * dt
    State.p.face_left = false
  end
  if btn(2) then
    State.p.y = State.p.y - State.p.spd * dt
  end
  if btn(3) then
    State.p.y = State.p.y + State.p.spd * dt
  end
  if btnp(4) then
    State.count += 1
  end

  State.p.x = mid(0, State.p.x, usagi.GAME_W - 16)
  State.p.y = mid(0, State.p.y, usagi.GAME_H - 16)
  State.spin = State.spin + dt * 0.25

  -- Two sparks per frame, then update positions and drop dead ones.
  emit_spark()
  emit_spark()
  for i = #State.sparks, 1, -1 do
    local s = State.sparks[i]
    s.x = s.x + s.vx * dt
    s.y = s.y + s.vy * dt
    s.life = s.life - dt
    if s.life <= 0 then
      table.remove(State.sparks, i)
    end
  end
end

function _draw(_dt)
  cls(gfx.COLOR_DARK_BLUE)

  -- HUD bar with rectfill (inclusive corners, Pico-8 style) and a
  -- horizontal `line` separator below it.
  rectfill(0, 0, usagi.GAME_W - 1, 13, gfx.COLOR_BLACK)
  line(0, 14, usagi.GAME_W - 1, 14, gfx.COLOR_DARK_GRAY)
  print("pico-8 flavor", 2, 1, gfx.COLOR_PEACH)
  print("count: " .. State.count, 200, 1, gfx.COLOR_YELLOW)

  -- Sprite from the spr example. Pico-8 is 0-based; pico8.lua adds 1.
  -- The flip args route through gfx.spr_ex when face_left is true.
  spr(SPR.BUNNY, 20, 30)
  spr(SPR.SHIP, State.p.x, State.p.y, nil, nil, State.p.face_left, false)
  spr(SPR.BULLET_SM, 20, 50)
  spr(SPR.BULLET_LG, 50, 50)

  -- Ship exhaust particle emitter. Each spark is one pixel via `pset`,
  -- Pico-8's single-pixel draw. The shim forwards pset to gfx.pixel.
  for _, s in ipairs(State.sparks) do
    pset(s.x, s.y, s.color)
  end

  -- Orbiting circle with a `line` crosshair through it. cos/sin take
  -- turns and sin is negated, exactly like Pico-8.
  local cx, cy = 280, 100
  line(cx - 22, cy, cx + 22, cy, gfx.COLOR_DARK_GRAY)
  line(cx, cy - 22, cx, cy + 22, gfx.COLOR_DARK_GRAY)
  circ(cx, cy, 18, gfx.COLOR_DARK_GRAY)
  local px = cx + cos(State.spin) * 18
  local py = cy + sin(State.spin) * 18
  circfill(px, py, 3, gfx.COLOR_PINK)

  print("arrows move, btn1 fires", 2, usagi.GAME_H - 10, gfx.COLOR_LIGHT_GRAY)
end
