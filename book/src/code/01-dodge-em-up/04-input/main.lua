x = 20
y = 40

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

function _draw(dt)
  gfx.clear(gfx.COLOR_BLACK)
  gfx.rect_fill(x, y, 16, 16, gfx.COLOR_GREEN)
end
