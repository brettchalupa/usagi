function _init()
  -- Live reload preserves globals across saved edits but resets locals.
  -- Stash mutable game state in a capitalized global like `State` so it
  -- survives reloads; F5 calls _init again to reset.
  State = {}
end

function _update(dt)
end

function _draw(dt)
  gfx.clear(gfx.COLOR_BLACK)
  -- ANCHOR: draw_greet
  gfx.text(greet("Alucard"), 10, 10, gfx.COLOR_WHITE)
  -- ANCHOR_END: draw_greet
end

-- ANCHOR: greet
function greet(name)
  return "Hello, " .. name .. "!"
end
-- ANCHOR_END: greet
