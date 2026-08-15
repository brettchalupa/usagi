-- ANCHOR: vars
x = 20
y = 40
-- ANCHOR_END: vars

function _init()
  -- Live reload preserves globals across saved edits but resets locals.
  -- Stash mutable game state in a capitalized global like `State` so it
  -- survives reloads; F5 calls _init again to reset.
  State = {}
end

-- ANCHOR: update_input
function _update(dt)
  if input.held(input.LEFT) then
    x = x - 4
  end
  if input.held(input.RIGHT) then
    x = x + 4
  end
  if input.held(input.UP) then
    y = y - 4
  end
  if input.held(input.DOWN) then
    y = y + 4
  end
end
-- ANCHOR_END: update_input

function _draw(dt)
  gfx.clear(gfx.COLOR_BLACK)
  -- ANCHOR: draw_player
  gfx.rect_fill(x, y, 16, 16, gfx.COLOR_GREEN)
  -- ANCHOR_END: draw_player
end
