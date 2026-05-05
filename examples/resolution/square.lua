-- Square res 1:1 256x256 test; 2x Pico-8's resolution

function _config()
  return { name = "1:1 - 256x256", game_width = 256, game_height = 256 }
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
  gfx.text("1:1 test - 256x256", 12, 24, gfx.COLOR_LIGHT_GRAY)
end
