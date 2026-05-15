-- Scene switching example: how to organize your game into multiple different scenes, like a main menu, gameplay, credits, etc.
-- Each scene is a table returned from a Lua file in ./scenes that must have a
-- `draw()` and `update()` function. An optional `init()` and `close()` get
-- called if defined.

function _config()
  return { name = "Scene Switching" }
end

local scenes = {
  main_menu = require("scenes.main_menu"),
  gameplay = require("scenes.gameplay"),
}

-- changes the current scene to the one matching the passed in key
-- uses a pending scene to so that the switch is on the next _update loop
function SwitchScene(key)
  local new_scene = scenes[key]
  assert(new_scene, "scene not found: " .. key)
  State.pending_scene = key
end

function _init()
  State = {}
  SwitchScene("main_menu")
end

function _update(dt)
  if State.pending_scene then
    if State.current_scene and scenes[State.current_scene].close then
      scenes[State.current_scene].close()
    end

    State.current_scene = State.pending_scene
    State.pending_scene = nil

    if scenes[State.current_scene].init then
      scenes[State.current_scene].init()
    end
  end
  scenes[State.current_scene].update()
end

function _draw()
  gfx.clear(gfx.COLOR_BLACK)
  scenes[State.current_scene].draw()
end
