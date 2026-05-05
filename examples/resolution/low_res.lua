-- Half-default 160x90. Useful for stress-testing tight UI layouts:
-- the bundled font (5x7) and 16x16 sprites take up a much larger
-- fraction of the viewport, and the pause menu starts to feel
-- cramped at this size.

function _config()
  return { name = "low_res 160x90", game_width = 160, game_height = 90 }
end

function _init()
  State = { x = 16, y = 16, vx = 30, vy = 25 }
end

function _update(dt)
  State.x = State.x + State.vx * dt
  State.y = State.y + State.vy * dt
  if State.x < 2 or State.x > usagi.GAME_W - 2 then State.vx = -State.vx end
  if State.y < 2 or State.y > usagi.GAME_H - 2 then State.vy = -State.vy end
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_DARK_GREEN)
  gfx.rect(0, 0, usagi.GAME_W - 1, usagi.GAME_H - 1, gfx.COLOR_GREEN)
  gfx.circ_fill(State.x, State.y, 2, gfx.COLOR_WHITE)
  gfx.text(string.format("%dx%d", usagi.GAME_W, usagi.GAME_H), 4, 4, gfx.COLOR_WHITE)
end
