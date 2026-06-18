-- Example showing how to load a level from LDtk and move around the map with
-- a camera. Only renders visible tiles. Includes a drop-in function you can use
-- to render LDtk levels.
--
-- .ldtk files are already JSON, so just drop it in ./data and read it with
-- `usagi.read_json`. It holds one or more `levels` and this example draws
-- the first one.
--
-- The LDtk renderer lives in ldtk.lua. To use it in your own game, just
-- download that one file next to your main.lua and `require` it like below.
--
-- Sprites by Kenney https://kenney.nl/assets/pixel-line-platformer

local ldtk = require("ldtk")
local ldtk_project = usagi.read_json("level.ldtk")
local test_level = ldtk_project.levels[1]

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

  cam.x = util.clamp(cam.x, 0, test_level.pxWid - usagi.GAME_W)
  cam.y = util.clamp(cam.y, 0, test_level.pxHei - usagi.GAME_H)
end

function _draw(_dt)
  ldtk.draw_level(test_level, State.camera)
end
