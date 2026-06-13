local player_size = 16
local player_speed = 180 -- px/s
local fire_delay = 0.1   -- secs
local fire_timer = 0
local bullet_speed = 420 -- px/s
local player_bullet_w = 4
local player_bullet_h = 10
local hit_flash_time = 0.2 -- secs

function _config()
  ---@type Usagi.Config
  return {
    name = "Shmup",
    game_id = "com.brettmakesgames.shmuptutorial",
    game_width = 320,
    game_height = 320,
  }
end

function init_enemy(x, y)
  return {
    x = x,
    y = y,
    hp = 12,
    w = 16,
    h = 16,
    speed = 44, -- px/s
    color = gfx.COLOR_RED,
    flash_timer = 0
  }
end

function _init()
  State = {
    player = {
      x = usagi.GAME_W / 2 - player_size / 2,
      y = usagi.GAME_H - 60,
      bullets = {}
    },
    enemies = {
      init_enemy(72, -20),
      init_enemy(usagi.GAME_W - 72, -20),
      init_enemy(usagi.GAME_W / 2, -60),
    },
  }
end

function _update(dt)
  local input_delta = { x = 0, y = 0 }
  if input.held(input.UP) then
    input_delta.y -= 1
  end
  if input.held(input.DOWN) then
    input_delta.y += 1
  end
  if input.held(input.LEFT) then
    input_delta.x -= 1
  end
  if input.held(input.RIGHT) then
    input_delta.x += 1
  end
  local normalized_input = util.vec_normalize(input_delta)
  State.player.x += normalized_input.x * player_speed * dt
  State.player.y += normalized_input.y * player_speed * dt
  State.player.x = util.clamp(State.player.x, 0, usagi.GAME_W - player_size)
  State.player.y = util.clamp(State.player.y, 0, usagi.GAME_H - player_size)

  fire_timer -= dt

  if fire_timer <= 0 and input.held(input.BTN1) then
    local bul_y = State.player.y - player_bullet_h
    -- fire 3 bullets
    table.insert(State.player.bullets,
      { x = State.player.x - player_bullet_w, y = bul_y })
    table.insert(State.player.bullets,
      { x = State.player.x + player_size / 2 - player_bullet_w / 2, y = bul_y })
    table.insert(State.player.bullets,
      { x = State.player.x + player_size, y = bul_y })
    fire_timer = fire_delay
  end

  for i = #State.player.bullets, 1, -1 do
    local bullet = State.player.bullets[i]
    -- move the bullet upward
    bullet.y -= bullet_speed * dt

    -- check if the bullet has overlapped with any of the enemies
    for _, enemy in ipairs(State.enemies) do
      if util.rect_overlap(
            { x = bullet.x, y = bullet.y,
              w = player_bullet_w, h = player_bullet_h },
            enemy) then
        bullet.dead = true
        enemy.hp -= 1
        enemy.flash_timer = hit_flash_time
      end
    end

    -- remove bullets that have flown off the top of the screen
    if bullet.y < -player_bullet_h or bullet.dead then
      table.remove(State.player.bullets, i)
    end
  end

  for i = #State.enemies, 1, -1 do
    local enemy = State.enemies[i]

    enemy.y += enemy.speed * dt

    if enemy.flash_timer > 0 then
      enemy.flash_timer = enemy.flash_timer - dt
    end

    if enemy.hp <= 0 or enemy.y > usagi.GAME_H then
      table.remove(State.enemies, i)
    end
  end

  if #State.enemies == 0 then
    table.insert(
      State.enemies,
      init_enemy(72, -20)
    )
    table.insert(
      State.enemies,
      init_enemy(usagi.GAME_W - 72, -20)
    )
    table.insert(
      State.enemies,
      init_enemy(usagi.GAME_W / 2, -60)
    )
  end
end

function _draw(dt)
  gfx.clear(gfx.COLOR_WHITE)
  gfx.rect_fill(
    State.player.x, State.player.y,
    player_size, player_size, gfx.COLOR_BLACK
  )

  for _, enemy in ipairs(State.enemies) do
    local color = enemy.color
    if enemy.flash_timer > 0 then
      color = gfx.COLOR_PINK
    end
    gfx.rect_fill(enemy.x, enemy.y, enemy.w, enemy.h, color)
  end

  for _, bullet in ipairs(State.player.bullets) do
    gfx.rect_fill(bullet.x, bullet.y,
      player_bullet_w, player_bullet_h, gfx.COLOR_LIGHT_GRAY)
  end
end
