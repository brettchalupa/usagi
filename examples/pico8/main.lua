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

function _config()
  return { title = "Pico-8 flavor" }
end

function _init()
  state = {
    p = { x = 50, y = 80, spd = 200 },
    count = 0,
    spin = 0,
  }
end

function _update(dt)
  if btn(0) then
    state.p.x = state.p.x - state.p.spd * dt
  end
  if btn(1) then
    state.p.x = state.p.x + state.p.spd * dt
  end
  if btn(2) then
    state.p.y = state.p.y - state.p.spd * dt
  end
  if btn(3) then
    state.p.y = state.p.y + state.p.spd * dt
  end
  if btnp(4) then
    state.count += 1
  end

  state.p.x = mid(0, state.p.x, usagi.GAME_W - 16)
  state.p.y = mid(0, state.p.y, usagi.GAME_H - 16)
  state.spin = state.spin + dt * 0.25
end

function _draw(_dt)
  cls(gfx.COLOR_DARK_BLUE)

  -- HUD bar drawn with rectfill (inclusive corners, Pico-8 style).
  rectfill(0, 0, usagi.GAME_W - 1, 13, gfx.COLOR_BLACK)
  print("pico-8 flavor", 2, 1, gfx.COLOR_PEACH)
  print("count: " .. state.count, 200, 1, gfx.COLOR_YELLOW)

  -- Sprite from the spr example. Pico-8 is 0-based; pico8.lua adds 1.
  spr(SPR.BUNNY, 20, 30)
  spr(SPR.SHIP, state.p.x, state.p.y)
  spr(SPR.BULLET_SM, 20, 50)
  spr(SPR.BULLET_LG, 50, 50)

  -- Orbiting circle: cos/sin are in turns (full revolution = 1.0) and
  -- sin is negated, exactly like Pico-8.
  local cx, cy = 280, 100
  circ(cx, cy, 18, gfx.COLOR_DARK_GRAY)
  local px = cx + cos(state.spin) * 18
  local py = cy + sin(state.spin) * 18
  circfill(px, py, 3, gfx.COLOR_PINK)

  -- Random sparkles via rnd().
  for _ = 1, 20 do
    local sx = flr(rnd(usagi.GAME_W))
    local sy = flr(rnd(usagi.GAME_H - 16)) + 16
    line(sx, sy, sx, sy, gfx.COLOR_WHITE)
  end
end
