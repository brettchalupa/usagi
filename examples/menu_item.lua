-- usagi.menu_item demo: register custom pause-menu rows that are
-- *contextual* to the current scene. Title screen has no game-action
-- items registered; entering gameplay registers three items at the
-- 3-item cap, including one that uses the stay-open escape hatch.
--
-- Items appear between Continue and Settings on the pause menu's Top
-- view. The callback fires when the player picks the row; the menu
-- closes by default. Return Lua `true` from the callback to keep it
-- open, which "Add 10 Coins" does so the player can spam-add without
-- reopening the menu each tap.
--
-- Items auto-clear right before each `_init` re-run (Reset Game / F5).
-- `usagi.clear_menu_items()` is for swapping the registered set
-- mid-game, which is what this demo uses on scene transitions.

function _config()
  return { name = "Menu Item Demo" }
end

local state
local level
local coins

local function enter_title()
  state = "title"
  level = 1
  coins = 0
  -- Title scene has no game actions in the pause menu, so wipe any
  -- previously-registered items. Calling clear when none are
  -- registered is a safe no-op.
  usagi.clear_menu_items()
end

local function enter_playing()
  state = "playing"
  usagi.clear_menu_items()

  usagi.menu_item("Title Screen", enter_title)

  usagi.menu_item("Skip Level", function()
    level = level + 1
  end)

  -- Stay-open variant: returning `true` from the callback keeps the
  -- pause menu open so the player can hammer the row to rack up
  -- coins without reopening the menu between presses. Also prints
  -- to the terminal each call so the developer can confirm the
  -- callback fired in the log.
  usagi.menu_item("Add 10 Coins", function()
    coins = coins + 10
    print("[menu_item demo] coins = " .. coins)
    return true
  end)
end

function _init()
  enter_title()
end

function _update()
  if state == "title" then
    if input.pressed(input.BTN1) then
      enter_playing()
    end
  end
end

function _draw()
  gfx.clear(gfx.COLOR_BLACK)
  if state == "title" then
    gfx.text("Menu Item Demo", 4, 4, gfx.COLOR_WHITE)
    gfx.text("Press BTN1 to start", 4, 20, gfx.COLOR_LIGHT_GRAY)
    gfx.text("(no game items in pause yet)", 4, 36, gfx.COLOR_DARK_GRAY)
  else
    gfx.text("Level " .. level, 4, 4, gfx.COLOR_WHITE)
    gfx.text("Coins " .. coins, 4, 14, gfx.COLOR_YELLOW)
    gfx.text("Press P to pause", 4, 32, gfx.COLOR_LIGHT_GRAY)
    gfx.text("Try the 3 custom items:", 4, 48, gfx.COLOR_LIGHT_GRAY)
    gfx.text("Title / Skip / Add Coins", 4, 58, gfx.COLOR_LIGHT_GRAY)
  end
end
