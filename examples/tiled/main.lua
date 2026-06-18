-- Example showing how to load a level from Tiled and move around the map with
-- a camera. Only renders visible tiles. Includes a drop-in function you can use
-- to render Tiled levels
--
-- Assumes Tiled levels are exported as Lua code (Ctrl +
-- Shift + E the first time, then Ctrl + E after). You can also save your Tiled
-- levels as JSON in the ./data directory and load that with `usagi.read_json`
-- and use it in a similar way. This example uses the Lua export.
--
-- The Tiled renderer lives in tiled.lua. To use it in your own game, just
-- download that one file next to your main.lua and `require` it like below.
--
-- Sprites by Kenney https://kenney.nl/assets/pixel-line-platformer

local tiled = require("tiled")
local test_level = require("level")

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
  tiled.draw_map(test_level, State.camera)
end
