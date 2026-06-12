function _config()
  ---@type Usagi.Config
  return { name = "Game", game_id = "com.usagiengine.YOURGAMENAME" }
end

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
  gfx.rect_fill(20, 40, 16, 16, gfx.COLOR_GREEN)
end
