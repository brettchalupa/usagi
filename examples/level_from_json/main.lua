-- Loads tile grids from a JSON file under `data/`. The same call works
-- in dev (reads `examples/level_from_json/data/levels.json`) and in
-- exported builds (reads it out of the bundle).
--
-- `usagi.read_json` is called at the top of the chunk so live reload
-- picks up edits to `data/levels.json` without F5. Try opening
-- `data/levels.json` while this is running, swap a `.` for a `2`, and
-- save. The new wall shows up immediately.

function _config()
  return { name = "Level from JSON" }
end

local LEVELS = usagi.read_json("levels.json").levels

-- Map the JSON-side palette names back to gfx.COLOR_* constants.
-- Strings in JSON, integers in gfx; one lookup table bridges them.
local COLOR = {
  GREEN      = gfx.COLOR_GREEN,
  BROWN      = gfx.COLOR_BROWN,
  DARK_BLUE  = gfx.COLOR_DARK_BLUE,
  YELLOW     = gfx.COLOR_YELLOW,
  DARK_GRAY  = gfx.COLOR_DARK_GRAY,
  LIGHT_GRAY = gfx.COLOR_LIGHT_GRAY,
  INDIGO     = gfx.COLOR_INDIGO,
  PEACH      = gfx.COLOR_PEACH,
}

function _init()
  State = { level_idx = 1 }
end

function _update(_dt)
  if input.pressed(input.BTN1) then
    State.level_idx = (State.level_idx % #LEVELS) + 1
  end
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_BLACK)
  local level = LEVELS[State.level_idx]
  local size  = level.tile_size
  local offx  = (usagi.GAME_W - size * #level.tiles[1]) / 2
  local offy  = (usagi.GAME_H - size * #level.tiles) / 2 + 6

  for row, line in ipairs(level.tiles) do
    for col = 1, #line do
      local ch = line:sub(col, col)
      if ch ~= "." then
        local color = COLOR[level.palette[ch]] or gfx.COLOR_PINK
        gfx.rect_fill(offx + (col - 1) * size, offy + (row - 1) * size, size, size, color)
      end
    end
  end

  gfx.text("level: " .. level.name, 4, 4, gfx.COLOR_WHITE)
  gfx.text("BTN1: next level", 4, usagi.GAME_H - 12, gfx.COLOR_LIGHT_GRAY)
end
