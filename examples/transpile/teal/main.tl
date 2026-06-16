-- barebones typedef example for Usagi's `gfx` functions and constants
global record gfx
  clear: function(color: number)
  text: function(text: string, x: number, y: number, color: number)
  COLOR_BLACK: number
  COLOR_WHITE: number
  COLOR_PEACH: number
end

global function _draw(dt: number)
  gfx.clear(gfx.COLOR_BLACK)
  gfx.text("Hello, Teal!", 10, 10, gfx.COLOR_WHITE)
  gfx.text("dt: " .. dt, 10, 32, gfx.COLOR_PEACH)
end
