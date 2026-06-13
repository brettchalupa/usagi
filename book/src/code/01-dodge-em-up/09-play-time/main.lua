x = 20
y = 60
enemies = {}
enemy_spawn_timer = 0
enemy_spawn_delay = 0.5 -- secs
game_over = false
play_time = 0

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
    local padding = 10
    table.insert(
      enemies,
      {
        x = usagi.GAME_W + padding,
        y = math.random(padding, usagi.GAME_H - padding),
        spd = math.random(2, 4) -- pixels per frame
      }
    )
    enemy_spawn_timer = enemy_spawn_delay
  end

  for i = 1, #enemies do
    local enemy = enemies[i]
    enemy.x -= enemy.spd

    if util.circ_rect_overlap(
          { x = enemy.x, y = enemy.y, r = 8 },
          { x = x, y = y, w = 16, h = 16 }
        ) then
      game_over = true
    end
  end

  for i = #enemies, 1, -1 do
    if enemies[i].x < -10 then
      table.remove(enemies, i)
    end
  end

  if game_over and input.pressed(input.BTN1) then
    -- reset our game data
    x = 20
    y = 60
    enemies = {}
    enemy_spawn_timer = 0
    game_over = false
    play_time = 0
  end

  if not game_over then
    play_time = play_time + dt
  end
end

function _draw(dt)
  gfx.clear(gfx.COLOR_BLACK)

  if not game_over then
    -- draw the player
    gfx.rect_fill(x, y, 16, 16, gfx.COLOR_GREEN)
  end

  for i = 1, #enemies do
    local enemy = enemies[i]
    gfx.circ_fill(enemy.x, enemy.y, 8, gfx.COLOR_RED)
  end

  if game_over then
    gfx.text("GAME OVER", 10, 10, gfx.COLOR_WHITE)
    gfx.text("Press " .. input.mapping_for(input.BTN1) .. " to restart",
      10, 30, gfx.COLOR_WHITE)
  end

  gfx.text(math.floor(play_time) .. "s", 280, 10, gfx.COLOR_WHITE)
end
