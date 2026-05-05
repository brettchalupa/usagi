-- Playdate Prototype: resolution at half the Playdate's screen size with a
-- shader to make it look like Playdate's colors. Useful for prototyping and
-- eventual shim to run the Usagi API via the Playdate SDK.

function _config()
  return {
    name = "Usagi Playdate Prototype",
    game_width = 200,
    game_height = 120,
    pixel_perfect = true
  }
end

function _init()
  gfx.shader_set("playdate_palette")

  State = {
    x = 40,
    y = 30,
    vx = 120,
    vy = 60,
    r = 8
  }
end

function _update(dt)
  State.x = State.x + State.vx * dt
  State.y = State.y + State.vy * dt

  if State.x < State.r then
    State.x = State.r
    State.vx = -State.vx
  elseif State.x + State.r > usagi.GAME_W then
    State.x = usagi.GAME_W - State.r
    State.vx = -State.vx
  end

  if State.y < State.r then
    State.y = State.r
    State.vy = -State.vy
  elseif State.y + State.r > usagi.GAME_H then
    State.y = usagi.GAME_H - State.r
    State.vy = -State.vy
  end

  local cx, cy, r = 100, 50, 20
  local speed = 2 -- radians / sec
  local angle = usagi.elapsed * speed
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_WHITE)
  gfx.circ_fill(State.x, State.y, State.r, gfx.COLOR_BLACK)
  gfx.text("Usagi Playdate Prototype", 10, 10, gfx.COLOR_BLACK)
end
