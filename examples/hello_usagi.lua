-- Live reload preserves globals but re-runs the chunk, so locals get
-- fresh nil bindings each save. Cross-frame mutable data goes in a
-- capitalized global (assigned only in _init); constants stay local.
-- F5 calls _init to reset.

local MSG = "Hello, Usagi!"

function _config()
  return { name = "Hello, Usagi!" }
end

function _init()
  State = {
    x = 40,
    y = 30,
    vx = 120,
    vy = 60,
    sx = 0,
    sy = 0,
  }
end

function _update(dt)
  State.x = State.x + State.vx * dt
  State.y = State.y + State.vy * dt

  if State.x < 0 then
    State.x = 0
    State.vx = -State.vx
  elseif State.x + usagi.SPRITE_SIZE > usagi.GAME_W then
    State.x = usagi.GAME_W - usagi.SPRITE_SIZE
    State.vx = -State.vx
  end

  if State.y < 0 then
    State.y = 0
    State.vy = -State.vy
  elseif State.y + usagi.SPRITE_SIZE > usagi.GAME_H then
    State.y = usagi.GAME_H - usagi.SPRITE_SIZE
    State.vy = -State.vy
  end

  local cx, cy, r = 100, 50, 20
  local speed = 2 -- radians / sec
  local angle = usagi.elapsed * speed
  State.sx = cx + math.cos(angle) * r
  State.sy = cy + math.sin(angle) * r
end

function _draw(dt)
  gfx.clear(gfx.COLOR_BLACK)
  local padding = 10
  gfx.text(MSG, padding, padding, gfx.COLOR_WHITE)

  if usagi.IS_DEV then
    gfx.text("DEV mode!", usagi.GAME_W - usagi.measure_text("DEV mode!") - padding, padding, gfx.COLOR_PINK)
  end

  gfx.spr(1, State.x, State.y)
  gfx.spr(2, State.sx, State.sy)
end
