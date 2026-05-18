-- Loads a tile grid from a CSV file under `data/`. The engine doesn't
-- ship a CSV parser. For a prototyping engine, the simple-grid case is two
-- `string.gmatch` loops and you see exactly what's happening.
--
-- `usagi.read_text` is called at the top of the chunk so live reload
-- picks up edits to `data/level.csv` without F5. Open the CSV,
-- change a few digits, save, and the level updates in place.

function _config()
  return { name = "Level from CSV" }
end

-- Splits a CSV string into a list-of-lists of strings. Trims each
-- line's trailing \r so Windows-saved CSVs Just Work. Empty trailing
-- lines are dropped.
local function parse_csv(text)
  local rows = {}
  for raw in text:gmatch("[^\n]+") do
    local line = raw:gsub("\r$", "")
    if line ~= "" then
      local cells = {}
      for cell in line:gmatch("[^,]+") do
        cells[#cells + 1] = cell
      end
      rows[#rows + 1] = cells
    end
  end
  return rows
end

local TILE_SIZE = 12
local PALETTE   = {
  ["0"] = nil, -- empty (skip draw)
  ["1"] = gfx.COLOR_DARK_GRAY,
  ["2"] = gfx.COLOR_BROWN,
  ["3"] = gfx.COLOR_DARK_BLUE,
  ["4"] = gfx.COLOR_YELLOW,
  ["5"] = gfx.COLOR_RED,
}

local grid      = parse_csv(usagi.read_text("level.csv"))

function _draw(_dt)
  gfx.clear(gfx.COLOR_BLACK)
  local offx = (usagi.GAME_W - TILE_SIZE * #grid[1]) / 2
  local offy = (usagi.GAME_H - TILE_SIZE * #grid) / 2 + 6

  for row, cells in ipairs(grid) do
    for col, ch in ipairs(cells) do
      local color = PALETTE[ch]
      if color then
        gfx.rect_fill(offx + (col - 1) * TILE_SIZE, offy + (row - 1) * TILE_SIZE, TILE_SIZE, TILE_SIZE, color)
      end
    end
  end

  gfx.text("level from CSV", 4, 4, gfx.COLOR_WHITE)
  gfx.text("edit data/level.csv & save", 4, usagi.GAME_H - 12, gfx.COLOR_LIGHT_GRAY)
end
