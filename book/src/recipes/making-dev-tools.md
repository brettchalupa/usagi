# Making Dev Tools

You can make custom dev tools for your games. Just create a file for your tool,
like `leveleditor.lua` and build it like an Usagi game. Then run
`usagi dev leveleditor.lua` to launch your tool and have Usagi's live reload.
You can even `require("myfile")` and use the Lua code from your game in your
tool. Pretty nice!

You could parse and load your game data from `data` directory using plain text
or JSON. Combine this with Lua's `io` module, and you can write files back to
the disk:

```lua
Level = usagi.read_json("level1.json")
-- make changes to `Level`; then save it:
local f = io.open("data/level1.json", "w")
if f then
  f:write(usagi.to_json(Level))
  f:close()
  print("Saved data/leve1.json")
end
```

You can also use Lua tables in a similar way:

```lua
local level1 = require("level1")
-- make changes to `level1` based on interactions; then save it
local f = io.open("level1.lua", "w")
if f then
  f:write("return " .. usagi.dump(State.level) .. "\n")
  f:close()
  print("Saved level1.lua")
end
```

The file opening and writing are great candidates to wrap in a function. You
could call that whenever your level changes or when you press certain keyboard
keys to save. Since it's all just simple data, you can track it in version
control to make your changes less risky.

You can also use `gfx.spr` too to render your sprites from your game.

The benefit of building your own dev tools with Usagi is that the tool meets
your game's specific needs, you can reuse your source code, and you learn a lot!

Here are some ideas for what dev tools you could make:

- Tile-based level editor
- Database browser if you had all your game data in data files, like an enemy
  bestiary
- Spawn scheduler for a shmup

Here's an example of a dev tool I made for one of my shmups that lets me place
enemies that spawn at a specific tick in the game. I interact with the UI with
the mouse and it auto-saves everytime I make changes.

```lua
-- schedit - schedule editor

local level1 = require("level1")
local LANE = require("lane")
local Enemy = require("enemy")

local enemy_names = {}

for name, _v in pairs(Enemy.kind) do
  table.insert(enemy_names, name)
end
table.sort(enemy_names)
print("enemies: " .. usagi.dump(enemy_names))

local function current_enemy()
  return enemy_names[State.enemy_idx]
end

local function save_level()
  local f = io.open("level1.lua", "w")
  if f then
    f:write("return " .. usagi.dump(State.level) .. "\n")
    f:close()
    print("Saved level1.lua")
  end
end

Color = require("color")

function _config()
  return { name = "Schedule Editor", game_id = "com.neogeargame.schedit", game_width = 320, game_height = 320 }
end

function _init()
  State = {
    tick = 0,
    level = level1,
    enemy_idx = 1,
  }
end

local function next_with_spawn()
  local ticks = {}

  for tick, schedule in pairs(State.level) do
    if #schedule > 0 then
      table.insert(ticks, tick)
    end
  end

  table.sort(ticks)

  for _i, tick in ipairs(ticks) do
    if tick > State.tick then
      return tick
    end
  end

  return nil
end

local function prev_with_spawn()
  local ticks = {}

  for tick, schedule in pairs(State.level) do
    if #schedule > 0 then
      table.insert(ticks, tick)
    end
  end

  table.sort(ticks, function(a, b)
    return a > b
  end)

  for _i, tick in ipairs(ticks) do
    if tick < State.tick then
      return tick
    end
  end

  return nil
end

-- side-entry spawn slots for rook-style enemies that arc in from off-screen
local SIDE = {
  L = { x = -20, ui_x = 16 },
  R = { x = usagi.GAME_W + 20, ui_x = usagi.GAME_W - 16 },
}
local SIDE_SPAWN_Y = 20
local SLOT_Y = 58
local SLOT_H = 70

-- places `kind` at the schedule entry for the current tick at `x` (with optional `y`),
-- overwriting any spawn already at that x
local function place_at(x, y)
  print("place " .. current_enemy() .. " at x=" .. x .. (y and (",y=" .. y) or ""))

  local schedule = State.level[State.tick] or {}
  local set = false
  for i, spawn in ipairs(schedule) do
    if spawn.x == x then
      schedule[i].kind = current_enemy()
      schedule[i].y = y
      set = true
    end
  end

  if not set then
    table.insert(schedule, { kind = current_enemy(), x = x, y = y })
  end

  State.level[State.tick] = schedule
  save_level()
end

-- removes the spawn at `x` for the current tick
local function rm_at(x)
  print("rm spawn at x=" .. x)

  local schedule = State.level[State.tick]
  if schedule then
    local to_rm = nil
    for i, spawn in ipairs(schedule) do
      if spawn.x == x then
        to_rm = i
      end
    end
    if to_rm then
      table.remove(schedule, to_rm)
    end
    save_level()
  end

  State.level[State.tick] = schedule
end

-- returns one of "L" / "R" / a lane index / nil, identifying the placement slot under the mouse
local function get_mouse_slot()
  local mx, my = input.mouse()
  local mp = { x = mx, y = my }

  for side, info in pairs(SIDE) do
    local rect = { x = info.ui_x - 4, y = SLOT_Y, w = 8, h = SLOT_H }
    if util.point_in_rect(mp, rect) then
      return side
    end
  end

  for i, x in ipairs(LANE) do
    local rect = { x = x - 4, y = SLOT_Y, w = 8, h = SLOT_H }
    if util.point_in_rect(mp, rect) then
      return i
    end
  end

  return nil
end

local function slot_x(slot)
  return type(slot) == "string" and SIDE[slot].x or LANE[slot]
end

local function slot_y(slot)
  return type(slot) == "string" and SIDE_SPAWN_Y or nil
end

local SCROLL_TICKS = 10
function _update(_dt)
  if input.pressed(input.LEFT) then
    State.enemy_idx -= 1
  end
  if input.pressed(input.RIGHT) then
    State.enemy_idx += 1
  end
  State.enemy_idx = util.clamp(State.enemy_idx, 1, #enemy_names)

  if input.pressed(input.DOWN) or input.mouse_scroll() == 1 then
    State.tick -= SCROLL_TICKS
  end
  if input.pressed(input.UP) or input.mouse_scroll() == -1 then
    State.tick += SCROLL_TICKS
  end

  if input.key_pressed(input.KEY_Q) then
    local prev_tick_with_spawn = prev_with_spawn()
    if prev_tick_with_spawn then
      State.tick = prev_tick_with_spawn
    end
  end
  if input.key_pressed(input.KEY_E) then
    local next_tick_with_spawn = next_with_spawn()
    if next_tick_with_spawn then
      State.tick = next_tick_with_spawn
    end
  end

  State.tick = util.clamp(State.tick, 0, 10000)

  if input.key_pressed(input.KEY_X) then
    save_level()
  end

  if input.mouse_pressed(input.MOUSE_LEFT) then
    local slot = get_mouse_slot()
    if slot then
      place_at(slot_x(slot), slot_y(slot))
    end
  end
  if input.mouse_pressed(input.MOUSE_RIGHT) then
    local slot = get_mouse_slot()
    if slot then
      rm_at(slot_x(slot))
    end
  end
end

function _draw()
  gfx.clear(Color.BLACK)
  gfx.text("tick: " .. State.tick, 8, 8, Color.WHITE)
  gfx.text(string.format("time: %.2fs", State.tick / 60), 92, 8, Color.PEACH)

  gfx.text("A/D = change enemy; W/S = change tick;\nQ/E = jump tick; LMB to place, RMB to rm",
    8, usagi.GAME_H - 40, Color.PEACH)

  local schedule = State.level[State.tick]

  for i, x in ipairs(LANE) do
    local txt = "lane " .. i
    local w, _h = usagi.measure_text(txt)
    gfx.text(txt, x - w / 2, 40, Color.LIGHT_BLUE)

    gfx.rect_fill(x - 4, SLOT_Y, 8, SLOT_H, Color.LIGHT_BLUE)
  end

  for side, info in pairs(SIDE) do
    local w, _h = usagi.measure_text(side)
    gfx.text(side, info.ui_x - w / 2, 40, Color.ORANGE)
    gfx.rect_fill(info.ui_x - 4, SLOT_Y, 8, SLOT_H, Color.ORANGE)
  end

  if schedule then
    for _i, spawn in ipairs(schedule) do
      local txt = spawn.kind
      local w, _h = usagi.measure_text(txt)
      -- side-spawned enemies have an off-screen x; clamp the label to the side's UI slot
      local label_x = spawn.x
      if spawn.x < 0 then
        label_x = SIDE.L.ui_x
      elseif spawn.x > usagi.GAME_W then
        label_x = SIDE.R.ui_x
      end
      gfx.text(txt, label_x - w / 2, 132, Color.PEACH)
    end
  end

  local mx, my = input.mouse()
  gfx.text(current_enemy(), mx, my, Color.LIGHT_GRAY)
end
```
