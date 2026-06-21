# Scenes

Most games need scenes like main menu, gameplay, options, credits, etc. Players
expect a certain flow to the game's interface. The game launches to the main
menu and can choose to start the game, load a previous game, view the credits,
that sort of thing.

Building out various scenes and the ability to switch between them is simpler
than you might think if you've never coded it before. In our game, we'll keep
track of what the active scene is. Then in the Usagi game loop, in `_update` and
`_draw`, all we have to do is call the update and draw function for our active
scene. We'll organize our scenes into separate Lua files to make it easy to find
and add new ones.

In `main.lua`, load our main menu and gameplay scenes (which we'll add in a
moment), a global function called `SwitchScene`, some handling of making the
pending scene active, and then call out to the current scene's `update` and
`draw` functions:

```lua
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
  scenes[State.current_scene].update(dt)
end

function _draw()
  gfx.clear(gfx.COLOR_BLACK)
  scenes[State.current_scene].draw()
end
```

The code in `_update` handles the lifecycle functions that our scene switching
example has. Scenes can optionally define an `init()` function and a `close()`
function that gets called when they're first initialized and when they're
switched away from (a.k.a. closed). You can use these lifecycle functions to set
up data structures or tear them down, start music, that sort of thing.

Our local `scenes` table is used to organize the various scenes in a way plays
friendly with Usagi's live reload mechanism. By storing them in a table, as they
change, that table gets refreshed with the scene and then we find scene based on
its key from `State.current_scene` and call the appropriate function.

In `_init`, `SwitchScene("main_menu")` is called so that when our game launches,
it immediately switches to the main menu.

Now let's define our two scenes. Create the `scenes` folder. In
`scenes/main_menu.lua`, put this:

```lua
local M = {}

function M.init()
  print("main_menu init")
end

function M.close()
  print("main_menu close")
end

function M.update(_dt)
  if input.pressed(input.BTN1) then
    SwitchScene("gameplay")
  end
end

function M.draw()
  gfx.text("Hello from Main Menu!", 10, 10, gfx.COLOR_WHITE)
  gfx.text("Press " .. input.mapping_for(input.BTN1) .. " to switch to Gameplay!", 10, 30, gfx.COLOR_PEACH)
end

return M
```

It defines a Lua table with functions associated and returns it. The key
functions we need are defined: `update` and `draw`. Plus some printing to show
us that `init` and `close` work as expected.

In `update`, there's code that checks if BTN1 one is pressed. If it is, then
`SwitchScene` is called and changes the current scene to gameplay.

In `scenes/gameplay.lua`, add this:

```lua
local M = {}

function M.init()
  print("gameplay init")
end

function M.close()
  print("gameplay close")
end

function M.update(_dt)
  if input.pressed(input.BTN2) then
    SwitchScene("main_menu")
  end
end

function M.draw()
  gfx.text("Hello from Gameplay!", 10, 10, gfx.COLOR_WHITE)
  gfx.text("Press " .. input.mapping_for(input.BTN2) .. " to switch to Main Menu!", 10, 30, gfx.COLOR_PEACH)
end

return M
```

Gameplay functions very similarly to the main menu except BTN2 goes back and
different text is rendered. In your gameplay scene, you'd code your actual game
there.

You could, in `main.lua`, switch to gameplay automatically in dev mode to make
it more convenient to test:

```lua
if usagi.IS_DEV then
  SwitchScene("gameplay")
else
  SwitchScene("main_menu")
end
```

That's how you can build a simple yet powerful scene switching mechanism for
your game. Adding new scenes is as simple as creating the Lua file, implementing
the behavior, and adding to the `scenes` table in `main.lua`

Here are some ideas on how to extend the scene switching if you wanted:

- Add support for subscenes, where a scene can switch between different
  subscenes that are assocated with it. Like in a JRPG, gameplay could have a
  field subscene, a combat subscene, a menu subscene, etc.
- Add transitions between scenes, like a fade or wipe or something fancy.

[View the full scene_switching example.](https://codeberg.org/brettchalupa/usagi/src/branch/main/examples/scene_switching)
