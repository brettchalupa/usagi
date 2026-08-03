x = 20
y = 60
enemies = {}
enemy_spawn_timer = 0
enemy_spawn_delay = 2 -- secs

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

  enemy_spawn_timer = enemy_spawn_timer - dt
  if enemy_spawn_timer <= 0 then
    table.insert(enemies, { x = usagi.GAME_W, y = 40 })
    enemy_spawn_timer = enemy_spawn_delay
  end

  for i = 1, #enemies do
    local enemy = enemies[i]
    enemy.x -= 2
  end
end

function _draw(dt)
  gfx.clear(gfx.COLOR_BLACK)
  gfx.rect_fill(x, y, 16, 16, gfx.COLOR_GREEN)

  for i = 1, #enemies do
    local enemy = enemies[i]
    gfx.circ_fill(enemy.x, enemy.y, 8, gfx.COLOR_RED)
  end
end
