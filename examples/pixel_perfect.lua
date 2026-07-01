function _config()
  return {
    pixel_perfect = true
  }
end

function _draw()
  gfx.clear(gfx.COLOR_BLUE)
  gfx.text("Pixel Perfect Test", 10, 10, gfx.COLOR_BLACK)
end
