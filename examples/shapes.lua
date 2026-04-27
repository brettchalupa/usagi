-- Draws all of the available shapes to the screen.

function _draw(_dt)
  gfx.clear(gfx.COLOR_PEACH)
  gfx.rect_fill(10, 10, 20, 12, gfx.COLOR_DARK_GREEN)
  gfx.rect(10, 30, 20, 12, gfx.COLOR_DARK_BLUE)
end
