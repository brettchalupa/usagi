-- Custom pause menu demo. Disables the built-in pause overlay via
-- `_config().pause_menu = false`, then rebuilds an equivalent (smaller)
-- menu in pure Lua using the standalone APIs:
--
--   * `usagi.toggle_fullscreen` / `usagi.is_fullscreen`
--   * `usagi.quit`               (hidden on web, where it freezes the canvas)
--   * `usagi.PLATFORM`           (used to gate the Quit row)
--   * `input.key_pressed`        (raw keyboard, for menu open/nav)
--
-- Player drives a square around with arrows; press Esc, P, or Enter to
-- pop the custom menu.

function _config()
  return {
    name = "Custom Menu Demo",
    pause_menu = false,
  }
end

local menu_open = false
local cursor = 1
local px, py = 60, 30

local function build_items()
  local items = {
    { label = "Resume", action = function()
      menu_open = false
    end },
    { label = "Toggle Fullscreen", action = function()
      usagi.toggle_fullscreen()
    end },
  }
  -- usagi.quit on web only freezes the canvas; hide the row there so a
  -- player can't accidentally brick the page.
  if usagi.PLATFORM ~= "web" then
    table.insert(items, { label = "Quit Game", action = function()
      usagi.quit()
    end })
  end
  return items
end

local items

function _init()
  items = build_items()
end

local function update_menu()
  if input.key_pressed(input.KEY_UP) then
    cursor = cursor - 1
    if cursor < 1 then cursor = #items end
  end
  if input.key_pressed(input.KEY_DOWN) then
    cursor = cursor + 1
    if cursor > #items then cursor = 1 end
  end
  if input.key_pressed(input.KEY_ENTER) or input.pressed(input.BTN1) then
    items[cursor].action()
  end
  -- Esc / P also close the menu, mirroring how the built-in pause feels.
  if input.key_pressed(input.KEY_ESCAPE) or input.key_pressed(input.KEY_P) then
    menu_open = false
  end
end

local function update_game()
  if input.pressed(input.LEFT) then px = px - 4 end
  if input.pressed(input.RIGHT) then px = px + 4 end
  if input.pressed(input.UP) then py = py - 4 end
  if input.pressed(input.DOWN) then py = py + 4 end
  -- Any of the engine's old pause triggers opens our custom menu.
  if input.key_pressed(input.KEY_ESCAPE)
    or input.key_pressed(input.KEY_P)
    or input.key_pressed(input.KEY_ENTER) then
    menu_open = true
    cursor = 1
  end
end

function _update()
  if menu_open then
    update_menu()
  else
    update_game()
  end
end

local function draw_menu()
  local x, y = 60, 40
  local w, h = 200, 100
  gfx.rect_fill(x, y, w, h, gfx.COLOR_DARK_BLUE)
  gfx.rect(x, y, w, h, gfx.COLOR_WHITE)
  gfx.text("MENU", x + 8, y + 8, gfx.COLOR_WHITE)
  for i, item in ipairs(items) do
    local row_y = y + 24 + (i - 1) * 12
    local label = item.label
    if item.label == "Toggle Fullscreen" then
      label = "Fullscreen: " .. (usagi.is_fullscreen() and "On" or "Off")
    end
    local color = (i == cursor) and gfx.COLOR_YELLOW or gfx.COLOR_WHITE
    gfx.text((i == cursor and "> " or "  ") .. label, x + 8, row_y, color)
  end
end

function _draw()
  gfx.clear(gfx.COLOR_BLACK)
  gfx.rect_fill(px, py, 8, 8, gfx.COLOR_RED)
  gfx.text("Arrows: move", 4, 4, gfx.COLOR_LIGHT_GRAY)
  gfx.text("Esc / P / Enter: menu", 4, 14, gfx.COLOR_LIGHT_GRAY)
  if menu_open then
    draw_menu()
  end
end
