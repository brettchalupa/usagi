-- Grid model. All functions take a board grid + piece explicitly.

local cfg = require("config")
local COLS, ROWS = cfg.COLS, cfg.ROWS

local M = {}

function M.new()
  local b = {}
  for r = 1, ROWS do
    b[r] = {}
    for c = 1, COLS do
      b[r][c] = 0
    end
  end
  return b
end

function M.collides(board, piece, dx, dy, grid)
  grid = grid or piece.grid
  for r = 1, 4 do
    for c = 1, 4 do
      if grid[r][c] == 1 then
        local br = piece.y + r - 1 + dy
        local bc = piece.x + c - 1 + dx
        if bc < 1 or bc > COLS or br > ROWS then
          return true
        end
        if br >= 1 and board[br][bc] ~= 0 then
          return true
        end
      end
    end
  end
  return false
end

function M.lock(board, piece)
  for r = 1, 4 do
    for c = 1, 4 do
      if piece.grid[r][c] == 1 then
        local br = piece.y + r - 1
        local bc = piece.x + c - 1
        if br >= 1 and br <= ROWS and bc >= 1 and bc <= COLS then
          board[br][bc] = piece.color
        end
      end
    end
  end
end

function M.detect_full_rows(board)
  local rows = {}
  for r = 1, ROWS do
    local full = true
    for c = 1, COLS do
      if board[r][c] == 0 then
        full = false
        break
      end
    end
    if full then
      table.insert(rows, r)
    end
  end
  return rows
end

function M.remove_rows(board, rows)
  for i = #rows, 1, -1 do
    table.remove(board, rows[i])
    local row = {}
    for c = 1, COLS do
      row[c] = 0
    end
    table.insert(board, 1, row)
  end
end

function M.ghost_drop_distance(board, piece)
  local dy = 0
  while not M.collides(board, piece, 0, dy + 1) do
    dy = dy + 1
  end
  return dy
end

return M
