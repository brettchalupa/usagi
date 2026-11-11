-- Minimal typed usagi game

local record GameState
end

global State: GameState

global function _init()
  State = {}
end

global function _update(dt:number)
end

global function _draw(dt: number)
  gfx.clear(gfx.COLOR_BLACK)
  gfx.text("Hello, Teal!", 10, 10, gfx.COLOR_WHITE)
  gfx.text("dt: " .. dt, 10, 32, gfx.COLOR_PEACH)
end
