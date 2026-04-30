function _config()
  return { name = "Game", game_id = "com.usagiengine.YOURGAMENAME" }
end

function _init()
end

function _update(dt)
end

function _draw(dt)
  gfx.clear(gfx.COLOR_BLACK)
  gfx.text("Hello, Usagi!", 10, 10, gfx.COLOR_WHITE)
end
