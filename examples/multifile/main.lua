-- Multi-file example: main.lua loads helpers from sibling files via
-- `require`. Usagi resolves dotted module names against the project root,
-- so this example runs unchanged whether you `usagi dev` it or export it.

local ball = require("ball")
local colors = require("colors")

function _config()
  return { title = "Multifile" }
end

function _init()
  Balls = {
    ball.new(40, 30, 80, 50, colors.PLAYER),
    ball.new(120, 60, -60, 70, colors.ENEMY),
    ball.new(80, 90, 40, -90, colors.PICKUP),
  }
end

function _update(dt)
  for _, b in ipairs(Balls) do
    ball.update(b, dt)
  end
end

function _draw(dt)
  gfx.clear(gfx.COLOR_DARK_BLUE)
  for _, b in ipairs(Balls) do
    ball.draw(b)
  end
  gfx.text("required modules: ball + colors", 4, 4, gfx.COLOR_WHITE)
end
