-- Demo of `gfx.px` (screen pixel read) and `gfx.spr_px` (sprite sheet
-- pixel read).
--
-- The walls are drawn into the framebuffer in a specific palette
-- color. Movement consults `gfx.px` on the proposed destination
-- pixels and blocks the move when any of them is wall-colored. The
-- painted screen IS the collision data, which is the classic
-- fantasy-console trick: no tilemap, no separate collision layer.
--
-- Note that `gfx.px` reads the previous frame's finished image, so
-- mid-`_draw` reads don't see what the current frame has drawn yet.
-- For collision logic in `_update` that doesn't matter; the previous
-- frame's walls are exactly what you want to consult before moving.

local WALL_COLOR   = gfx.COLOR_RED
local FLOOR_COLOR  = gfx.COLOR_DARK_BLUE
local PLAYER_COLOR = gfx.COLOR_YELLOW

local PLAYER_SIZE  = 3

local player       = { x = 20, y = 100 }

function _config()
  return { name = "Pixel Reads" }
end

local function draw_walls()
  -- Outer frame.
  gfx.rect_fill(0, 0, 200, 4, WALL_COLOR)
  gfx.rect_fill(0, 0, 4, 180, WALL_COLOR)
  gfx.rect_fill(0, 176, 200, 4, WALL_COLOR)
  gfx.rect_fill(196, 0, 4, 180, WALL_COLOR)
  -- A few interior walls to bump into.
  gfx.rect_fill(40, 32, 80, 4, WALL_COLOR)
  gfx.rect_fill(80, 36, 4, 80, WALL_COLOR)
  gfx.rect_fill(40, 116, 60, 4, WALL_COLOR)
  gfx.rect_fill(140, 40, 4, 100, WALL_COLOR)
end

-- Returns true if any pixel of the proposed player rectangle is
-- wall-colored.
local function blocked_at(x, y)
  for dy = 0, PLAYER_SIZE - 1 do
    for dx = 0, PLAYER_SIZE - 1 do
      local _, _, _, slot = gfx.px(x + dx, y + dy)
      if slot == WALL_COLOR then return true end
    end
  end
  return false
end

function _update(_dt)
  local dx, dy = 0, 0
  if input.held(input.LEFT) then dx = dx - 1 end
  if input.held(input.RIGHT) then dx = dx + 1 end
  if input.held(input.UP) then dy = dy - 1 end
  if input.held(input.DOWN) then dy = dy + 1 end

  -- Step each axis independently so the player can slide along a wall
  -- instead of stopping dead when one axis is blocked.
  if dx ~= 0 and not blocked_at(player.x + dx, player.y) then
    player.x = player.x + dx
  end
  if dy ~= 0 and not blocked_at(player.x, player.y + dy) then
    player.y = player.y + dy
  end
end

-- Re-renders the first sprite cell pixel-by-pixel by scanning the
-- sheet with `gfx.spr_px`. Should be visually identical to the plain
-- `gfx.spr(1, ...)` next to it.
local function draw_spr_px_scan(dx, dy)
  local size = usagi.SPRITE_SIZE
  for y = 0, size - 1 do
    for x = 0, size - 1 do
      local _, _, _, slot = gfx.spr_px(1, x, y)
      if slot then
        gfx.pixel(dx + x, dy + y, slot)
      end
    end
  end
end

function _draw(_dt)
  gfx.clear(FLOOR_COLOR)
  draw_walls()
  gfx.rect_fill(player.x, player.y, PLAYER_SIZE, PLAYER_SIZE, PLAYER_COLOR)

  gfx.text("gfx.px example", 4, 8, gfx.COLOR_WHITE)

  -- Right panel: sprite-sheet read demo. Original via `gfx.spr`,
  -- then re-rendered by `gfx.spr_px` scanning, to show the same
  -- data is reachable as pixels.
  gfx.text("gfx.spr_px scan", 220, 8, gfx.COLOR_WHITE)
  gfx.spr(1, 224, 20)
  draw_spr_px_scan(264, 20)
  gfx.text("spr", 228, 40, gfx.COLOR_LIGHT_GRAY)
  gfx.text("spr_px", 264, 40, gfx.COLOR_LIGHT_GRAY)
end
