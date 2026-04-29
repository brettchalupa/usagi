-- Render primitives. Pure draw calls; the caller decides positions.

local cfg = require("config")
local CELL = cfg.CELL

local M = {}

function M.cell(x, y, color)
  -- 1px gap between cells: fill 7x7 inside the 8px slot, gap shows the bg.
  gfx.rect_fill(x, y, CELL - 1, CELL - 1, color)
end

function M.piece(grid, color, px, py)
  for r = 1, 4 do
    for c = 1, 4 do
      if grid[r][c] == 1 then
        M.cell(px + (c - 1) * CELL, py + (r - 1) * CELL, color)
      end
    end
  end
end

function M.ghost(grid, color, px, py)
  for r = 1, 4 do
    for c = 1, 4 do
      if grid[r][c] == 1 then
        local x = px + (c - 1) * CELL
        local y = py + (r - 1) * CELL
        gfx.rect(x, y, CELL - 1, CELL - 1, color)
      end
    end
  end
end

return M
