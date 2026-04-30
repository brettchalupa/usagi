-- Live reload preserves globals but re-runs the chunk, so locals get
-- fresh nil bindings each save. Keep mutable state in globals (assigned
-- only in _init); keep constants local. F5 calls _init to reset.

local MSG = "Hello, Usagi!"

function _config()
  return { title = "Hello, Usagi!" }
end

function _init()
  -- usagi.measure_text returns (width, height) in the bundled font
  text_w, text_h = usagi.measure_text(MSG)
  x = 40
  y = 30
  vx = 120
  vy = 60
end

function _update(dt)
  x = x + vx * dt
  y = y + vy * dt

  if x < 0 then
    x = 0
    vx = -vx
  elseif x + text_w > usagi.GAME_W then
    x = usagi.GAME_W - text_w
    vx = -vx
  end

  if y < 0 then
    y = 0
    vy = -vy
  elseif y + text_h > usagi.GAME_H then
    y = usagi.GAME_H - text_h
    vy = -vy
  end

  local cx, cy, r = 100, 50, 20
  local speed = 2 -- radians / sec
  local angle = usagi.elapsed * speed
  sx = cx + math.cos(angle) * r
  sy = cy + math.sin(angle) * r
end

function _draw(dt)
  gfx.clear(gfx.COLOR_BLACK)
  local padding = 10
  gfx.text(MSG, padding, padding, gfx.COLOR_WHITE)

  if usagi.IS_DEV then
    gfx.text("DEV mode!", usagi.GAME_W - usagi.measure_text("DEV mode!") - padding, padding, gfx.COLOR_PINK)
  end

  gfx.spr(1, x, y)
  gfx.spr(2, sx, sy)
end
