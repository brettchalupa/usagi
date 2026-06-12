local player_size = 16
local player_speed = 180 -- px/s
local fire_delay = 0.1   -- s
local fire_timer = 0
local bullet_speed = 420 -- px/s
local player_bullet_w = 4
local player_bullet_h = 10

function _config()
  ---@type Usagi.Config
  return {
    name = "Shmup",
    game_id = "com.brettmakesgames.shmuptutorial",
    game_width = 320,
    game_height = 320,
  }
end

function _init()
  State = {
    player = {
      x = usagi.GAME_W / 2 - player_size / 2,
      y = usagi.GAME_H - 60,
      bullets = {}
    }
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

    -- remove bullets that have flown off the top of the screen
    if bullet.y < -player_bullet_h then
      table.remove(State.player.bullets, i)
    end
  end
end

function _draw(dt)
  gfx.clear(gfx.COLOR_WHITE)
  gfx.rect_fill(
    State.player.x, State.player.y,
    player_size, player_size, gfx.COLOR_BLACK
  )

  for _, bullet in ipairs(State.player.bullets) do
    gfx.rect_fill(bullet.x, bullet.y,
      player_bullet_w, player_bullet_h, gfx.COLOR_LIGHT_GRAY)
  end
end
