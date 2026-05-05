-- Portrait-oriented game at 180x320. Demonstrates that the engine
-- doesn't assume 16:9; the window opens tall and the RT, mouse
-- coords, pause menu, and capture all follow the configured dims.

function _config()
  return { name = "vertical 180x320", game_width = 180, game_height = 320 }
end

function _init()
  State = { x = 90, y = 40, vx = 50, vy = 80 }
end

function _update(dt)
  State.x = State.x + State.vx * dt
  State.y = State.y + State.vy * dt
  if State.x < 4 or State.x > usagi.GAME_W - 4 then State.vx = -State.vx end
  if State.y < 4 or State.y > usagi.GAME_H - 4 then State.vy = -State.vy end
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_DARK_PURPLE)
  gfx.rect(0, 0, usagi.GAME_W - 1, usagi.GAME_H - 1, gfx.COLOR_PINK)
  gfx.circ_fill(State.x, State.y, 4, gfx.COLOR_YELLOW)
  gfx.text(string.format("%dx%d", usagi.GAME_W, usagi.GAME_H), 6, 6, gfx.COLOR_WHITE)
  gfx.text("portrait", 6, 16, gfx.COLOR_LIGHT_GRAY)
end
