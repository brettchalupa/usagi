-- Pure scoring + gravity math. No state, no side effects.

local M = {}

function M.level_for_lines(lines)
  return math.floor(lines / 10) + 1
end

function M.gravity_interval(level)
  -- Tetris Guideline: seconds per row = (0.8 - (level-1) * 0.007) ^ (level-1).
  -- L1=1.00s, L5=0.39s, L10=0.12s, L15=0.023s. Floor at one frame (~16ms)
  -- so the multi-step update loop doesn't burn CPU on degenerate intervals.
  local t = (0.8 - (level - 1) * 0.007) ^ (level - 1)
  return math.max(1 / 60, t)
end

function M.score_for_lines(n, level)
  local base = ({ 100, 300, 500, 800 })[n] or 0
  return base * level
end

return M
