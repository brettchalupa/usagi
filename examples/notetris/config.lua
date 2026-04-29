-- Tuning knobs for the playfield, layout, and timings.

local M = {
  CELL = 8,
  COLS = 10,
  ROWS = 20,
  BOARD_X = 96,
  BOARD_Y = 10,
  -- Horizontal auto-repeat: delay before auto-repeat starts, then interval.
  DAS = 0.16,
  ARR = 0.04,
  SOFT_DROP_INTERVAL = 0.04,
  CLEAR_FLASH_DURATION = 0.14,
}
M.BOARD_W = M.COLS * M.CELL
M.BOARD_H = M.ROWS * M.CELL
M.UI_X = M.BOARD_X + M.BOARD_W + 12

return M
