---@diagnostic disable: undefined-global, lowercase-global
-- Companion to usagi-shim.lua. Drop this in next to main.lua and
-- usagi-shim.lua.
--
-- Love2D opens a default 800x600 window automatically unless this file
-- says otherwise. Setting `t.window = false` defers window creation to
-- the shim's `love.load`, which calls `love.window.setMode` with the
-- size your `_config().game_width / game_height` declared. Result: the
-- window opens at the correct size in one step, no resize flash.
--
-- If you need other Love-side config (audio mix mode, identity for save
-- paths, disabled modules), set them here. The shim doesn't depend on
-- anything beyond `t.window = false`.

function love.conf(t)
  t.window = false
end
