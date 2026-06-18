-- Example showing how to load a level from Tiled and move around the map with
-- a camera. Only renders visible tiles. Includes a drop-in function you can use
-- to render Tiled levels. Assumed Tiled levels are exported as Lua code (Ctrl +
-- Shift + E the first time, then Ctrl + E after). You can also save your Tiled
-- levels as JSON in the ./data directory and load that with `usagi.read_json`
-- and use it in a similar way. This example uses the Lua export.
--
-- Sprites by Kenney https://kenney.nl/assets/pixel-line-platformer

local test_level = require("level")

Tiled = {}

-- Draws a Tiled map's tile layers, offset by a camera, skipping tiles that
-- fall outside the screen. Drop this into your own game if you use Tiled!
--
-- `level`  a Tiled map exported as Lua (`require` the file)
-- `camera`  table of with `x` and `y` keys of world position of the screen's top-left (defaults to 0, 0)
function Tiled.draw_map(level, camera)
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

function _init()
  State = {
    camera = { x = 0, y = 0 }
  }
end

local SPEED = 200 -- px/sec

function _update(dt)
  local cam = State.camera
  if input.held(input.LEFT) then cam.x = cam.x - SPEED * dt end
  if input.held(input.RIGHT) then cam.x = cam.x + SPEED * dt end
  if input.held(input.UP) then cam.y = cam.y - SPEED * dt end
  if input.held(input.DOWN) then cam.y = cam.y + SPEED * dt end

  local map_w = test_level.width * usagi.SPRITE_SIZE
  local map_h = test_level.height * usagi.SPRITE_SIZE
  cam.x = util.clamp(cam.x, 0, map_w - usagi.GAME_W)
  cam.y = util.clamp(cam.y, 0, map_h - usagi.GAME_H)
end

function _draw(_dt)
  Tiled.draw_map(test_level, State.camera)
end
