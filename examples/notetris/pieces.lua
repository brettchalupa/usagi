-- Piece data, grid rotations, and the 7-bag randomizer. Stateless.

local M = {}

M.DEFS = {
  I = {
    color = gfx.COLOR_BLUE,
    grid = {
      { 0, 0, 0, 0 },
      { 1, 1, 1, 1 },
      { 0, 0, 0, 0 },
      { 0, 0, 0, 0 },
    },
  },
  O = {
    color = gfx.COLOR_YELLOW,
    grid = {
      { 0, 1, 1, 0 },
      { 0, 1, 1, 0 },
      { 0, 0, 0, 0 },
      { 0, 0, 0, 0 },
    },
  },
  T = {
    color = gfx.COLOR_PINK,
    grid = {
      { 0, 1, 0, 0 },
      { 1, 1, 1, 0 },
      { 0, 0, 0, 0 },
      { 0, 0, 0, 0 },
    },
  },
  S = {
    color = gfx.COLOR_GREEN,
    grid = {
      { 0, 1, 1, 0 },
      { 1, 1, 0, 0 },
      { 0, 0, 0, 0 },
      { 0, 0, 0, 0 },
    },
  },
  Z = {
    color = gfx.COLOR_RED,
    grid = {
      { 1, 1, 0, 0 },
      { 0, 1, 1, 0 },
      { 0, 0, 0, 0 },
      { 0, 0, 0, 0 },
    },
  },
  J = {
    color = gfx.COLOR_PEACH,
    grid = {
      { 1, 0, 0, 0 },
      { 1, 1, 1, 0 },
      { 0, 0, 0, 0 },
      { 0, 0, 0, 0 },
    },
  },
  L = {
    color = gfx.COLOR_ORANGE,
    grid = {
      { 0, 0, 1, 0 },
      { 1, 1, 1, 0 },
      { 0, 0, 0, 0 },
      { 0, 0, 0, 0 },
    },
  },
}

M.KEYS = { "I", "O", "T", "S", "Z", "J", "L" }

local function copy_grid(g)
  local out = {}
  for r = 1, 4 do
    out[r] = { g[r][1], g[r][2], g[r][3], g[r][4] }
  end
  return out
end
M.copy_grid = copy_grid

function M.rotate_cw(g)
  local out = { { 0, 0, 0, 0 }, { 0, 0, 0, 0 }, { 0, 0, 0, 0 }, { 0, 0, 0, 0 } }
  for r = 1, 4 do
    for c = 1, 4 do
      out[c][5 - r] = g[r][c]
    end
  end
  return out
end

function M.rotate_ccw(g)
  local out = { { 0, 0, 0, 0 }, { 0, 0, 0, 0 }, { 0, 0, 0, 0 }, { 0, 0, 0, 0 } }
  for r = 1, 4 do
    for c = 1, 4 do
      out[5 - c][r] = g[r][c]
    end
  end
  return out
end

function M.new(key)
  return {
    key = key,
    color = M.DEFS[key].color,
    grid = copy_grid(M.DEFS[key].grid),
    x = 4,
    y = 1,
  }
end

function M.new_bag()
  local bag = {}
  for _, k in ipairs(M.KEYS) do
    table.insert(bag, k)
  end
  for i = #bag, 2, -1 do
    local j = math.random(1, i)
    bag[i], bag[j] = bag[j], bag[i]
  end
  return bag
end

-- Pulls the next piece key from `bag`, refilling it in place when empty.
function M.pull(bag)
  if #bag == 0 then
    for _, k in ipairs(M.new_bag()) do
      table.insert(bag, k)
    end
  end
  return table.remove(bag, 1)
end

return M
