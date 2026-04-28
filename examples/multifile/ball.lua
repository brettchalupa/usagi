-- Ball module: returns a table of constructors and per-frame updaters.
-- Top-level `local`s here stay local to this chunk; only what's added to
-- `M` (and returned) leaks into callers via require.

local M = {}

local RADIUS = 12

function M.new(x, y, vx, vy, color)
  return { x = x, y = y, vx = vx, vy = vy, color = color }
end

function M.update(b, dt)
  b.x = b.x + b.vx * dt
  b.y = b.y + b.vy * dt
  if b.x < RADIUS then
    b.x = RADIUS
    b.vx = -b.vx
  elseif b.x > usagi.GAME_W - RADIUS then
    b.x = usagi.GAME_W - RADIUS
    b.vx = -b.vx
  end
  if b.y < RADIUS then
    b.y = RADIUS
    b.vy = -b.vy
  elseif b.y > usagi.GAME_H - RADIUS then
    b.y = usagi.GAME_H - RADIUS
    b.vy = -b.vy
  end
end

function M.draw(b)
  gfx.circ_fill(b.x, b.y, RADIUS, b.color)
end

return M
