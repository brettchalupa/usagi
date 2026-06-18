-- Drop-in Tiled renderer for usagi. Download this file into your game next to
-- your main.lua, then:
--
--   local tiled = require("tiled")
--   local level = require("level") -- your map, exported from Tiled as Lua
--   ...
--   function _draw()
--     tiled.draw_map(level, camera)
--   end
--
-- Export your Tiled maps as Lua code (Ctrl + Shift + E the first time, then
-- Ctrl + E after). Depends only on `usagi`, `util.clamp`, and `gfx.spr`.

local tiled = {}

-- Draws a Tiled map's tile layers, offset by a camera, skipping tiles that fall
-- outside the screen.
--
-- `level`   a Tiled map exported as Lua (`require` the file)
-- `camera`  table with `x` and `y` keys: world position of the screen's
--           top-left (defaults to 0, 0)
function tiled.draw_map(level, camera)
  local cam_x = camera.x or 0
  local cam_y = camera.y or 0
  local spr_size = usagi.SPRITE_SIZE

  for _, layer in ipairs(level.layers) do
    if layer.type == "tilelayer" and layer.data then
      local tiles_wide = layer.width
      local tiles_high = layer.height

      -- only draw the tiles overlapping the screen, clamped to the layer bounds
      local first_col = util.clamp(math.floor(cam_x / spr_size), 0, tiles_wide - 1)
      local first_row = util.clamp(math.floor(cam_y / spr_size), 0, tiles_high - 1)
      local last_col = util.clamp(math.floor((cam_x + usagi.GAME_W) / spr_size), 0, tiles_wide - 1)
      local last_row = util.clamp(math.floor((cam_y + usagi.GAME_H) / spr_size), 0, tiles_high - 1)

      for row = first_row, last_row do
        for col = first_col, last_col do
          local spr = layer.data[row * tiles_wide + col + 1]
          if spr ~= 0 then -- 0 is Tiled's empty tile
            gfx.spr(spr, col * spr_size - cam_x, row * spr_size - cam_y)
          end
        end
      end
    end
  end
end

return tiled
