-- Drop-in LDtk renderer for usagi. Download this file into your game next to
-- your main.lua, then:
--
--   local ldtk = require("ldtk")
--   local project = usagi.read_json("level.ldtk") -- your .ldtk file, in ./data
--   local level = project.levels[1]
--   ...
--   function _draw()
--     ldtk.draw_level(level, camera)
--   end
--
-- An .ldtk file is already JSON, so drop it in ./data and read it with
-- `usagi.read_json`. Depends only on `usagi`, `gfx.spr`, and `gfx.spr_ex`.

local ldtk = {}

-- Draws one LDtk level's tile layers, offset by a camera, skipping tiles that
-- fall outside the screen.
--
-- `level`   one entry from the .ldtk file's `levels` array
-- `camera`  table with `x` and `y` keys: world position of the screen's
--           top-left (defaults to 0, 0)
function ldtk.draw_level(level, camera)
  local cam_x = camera.x or 0
  local cam_y = camera.y or 0

  -- LDtk lists the topmost layer first, so draw back-to-front
  local layers = level.layerInstances
  for i = #layers, 1, -1 do
    local layer = layers[i]
    local g = layer.__gridSize
    local off_x = layer.__pxTotalOffsetX - cam_x
    local off_y = layer.__pxTotalOffsetY - cam_y

    -- a Tiles layer fills `gridTiles`; an Auto/IntGrid layer fills `autoLayerTiles`
    local tiles = #layer.gridTiles > 0 and layer.gridTiles or layer.autoLayerTiles
    for _, tile in ipairs(tiles) do
      -- LDtk gives each tile its pixel position directly, no row/col math
      local x = tile.px[1] + off_x
      local y = tile.px[2] + off_y

      -- skip tiles fully off-screen
      if x + g > 0 and x < usagi.GAME_W and y + g > 0 and y < usagi.GAME_H then
        local index = tile.t + 1 -- LDtk tile ids are 0-based; gfx.spr is 1-based
        local flip_x = (tile.f & 1) ~= 0
        local flip_y = (tile.f & 2) ~= 0
        if flip_x or flip_y then
          gfx.spr_ex(index, x, y, flip_x, flip_y, 0, gfx.COLOR_TRUE_WHITE, 1.0)
        else
          gfx.spr(index, x, y)
        end
      end
    end
  end
end

return ldtk
