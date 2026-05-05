-- Demonstrates `_config().sprite_size`. The engine default is 16;
-- this game runs at 32, so `gfx.spr` indexes into sprites.png at a
-- 32-pixel grid, the tile-picker tool's overlay matches, and the
-- window-icon slicer reads from the same cell size.
--
-- Sprites by Hexany Ives (CC0): https://hexany-ives.itch.io/hexanys-monster-menagerie

function _config()
  return {
    name = "custom_sprite_size",
    sprite_size = 32,
    -- `usagi tools .` will lay its grid out at 32px to match.
  }
end

function _init()
  State = { t = 0 }
end

function _update(dt)
  State.t = State.t + dt
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_DARK_BLUE)

  gfx.text("custom_sprite_size demo (sprites by Hexany Ives (CC0)", 6, 6, gfx.COLOR_WHITE)
  gfx.text("usagi.SPRITE_SIZE = " .. usagi.SPRITE_SIZE, 6, 18, gfx.COLOR_LIGHT_GRAY)
  gfx.text("drop a 32x32 sprites.png next to main.lua", 6, 28, gfx.COLOR_LIGHT_GRAY)

  -- Walk a row of tile indices. Each cell is `usagi.SPRITE_SIZE`
  -- pixels; gfx.spr's 1-based index reads from that grid.
  local margin = 6
  local y = 80
  local i = 1
  while true do
    local x = margin + (i - 1) * usagi.SPRITE_SIZE
    if i > 8 then break end
    gfx.spr(i, x, y)
    gfx.rect(x, y, usagi.SPRITE_SIZE, usagi.SPRITE_SIZE, gfx.COLOR_DARK_GRAY)
    gfx.text(tostring(i), x + 2, y + usagi.SPRITE_SIZE + 2, gfx.COLOR_WHITE)
    i = i + 1
  end
end
