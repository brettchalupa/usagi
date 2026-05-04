-- Save/load demo: data that persists across runs.
--
--   usagi.save(t)  -- writes a Lua table as JSON to a per-game file
--   usagi.load()   -- returns the table, or nil if there's no save yet
--
-- `game_id` (reverse-DNS) namespaces the save so it doesn't clobber
-- saves from other usagi games on the same machine. Required for
-- save / load. Convention matches Playdate bundle IDs and macOS /
-- iOS app bundle identifiers, so the same string is reusable when
-- packaging targets land in future versions.
--
-- Saves live at:
--   linux  : ~/.local/share/com.usagi.savedemo/save.json
--   macos  : ~/Library/Application Support/com.usagi.savedemo/save.json
--   windows: %APPDATA%\com.usagi.savedemo\save.json
--   web    : window.localStorage, key "usagi.save.com.usagi.savedemo"

function _config()
  return { name = "Save Demo", game_id = "com.usagi.savedemo" }
end

local function fresh_state()
  return { last_saved_at = nil, playtime = 0 }
end

function _init()
  State = usagi.load() or fresh_state()
end

function _update(dt)
  State.playtime += dt

  if input.pressed(input.BTN1) then
    State.last_saved_at = tonumber(os.time())
    usagi.save(State)
    print("Saved!")
  end

  if input.pressed(input.BTN2) then
    State = fresh_state()
    usagi.save(State)
    print("Reset save!")
  end

  if input.pressed(input.BTN3) then
    State = usagi.load() or fresh_state()
    print("Loaded!")
  end
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_BLACK)
  gfx.text("SAVE DEMO", 10, 10, gfx.COLOR_WHITE)
  local now = os.date("%Y-%m-%d %H:%M:%S", os.time())
  gfx.text("current time: " .. now, 10, 30, gfx.COLOR_PEACH)
  local saved = State.last_saved_at and os.date("%Y-%m-%d %H:%M:%S", State.last_saved_at) or "never"
  gfx.text("last saved at: " .. saved, 10, 46, gfx.COLOR_PINK)
  gfx.text(string.format("playtime: %.1fs", State.playtime), 10, 62, gfx.COLOR_YELLOW)
  local b1 = input.mapping_for(input.BTN1) or "BTN1"
  local b2 = input.mapping_for(input.BTN2) or "BTN2"
  local b3 = input.mapping_for(input.BTN3) or "BTN3"
  gfx.text(b1 .. " save; " .. b2 .. " reset; " .. b3 .. " load", 10, usagi.GAME_H - 18, gfx.COLOR_LIGHT_GRAY)
end
