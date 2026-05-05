-- Double-default 640x360. Useful for confirming that the RT,
-- letterbox math, and capture pipeline all scale up; also shows
-- the trade-off of a fixed sprite size and bundled font (both look
-- tiny relative to the viewport at this resolution).

function _config()
  return { name = "high_res 640x360", game_width = 640, game_height = 360 }
end

function _init()
  State = { x = 80, y = 80, vx = 180, vy = 130 }
end

function _update(dt)
  State.x = State.x + State.vx * dt
  State.y = State.y + State.vy * dt
  if State.x < 8 or State.x > usagi.GAME_W - 8 then State.vx = -State.vx end
  if State.y < 8 or State.y > usagi.GAME_H - 8 then State.vy = -State.vy end
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_DARK_BLUE)
  gfx.rect(0, 0, usagi.GAME_W - 1, usagi.GAME_H - 1, gfx.COLOR_INDIGO)
  gfx.circ_fill(State.x, State.y, 8, gfx.COLOR_PEACH)
  gfx.text(string.format("%dx%d", usagi.GAME_W, usagi.GAME_H), 12, 12, gfx.COLOR_WHITE)
  gfx.text("notice the font + sprite are now tiny", 12, 24, gfx.COLOR_LIGHT_GRAY)
end
