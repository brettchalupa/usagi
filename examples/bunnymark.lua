-- bunnymark: stress test for sprite throughput. Spawn batches of bunnies
-- (sprite 1 from sprites.png) that fall and bounce, watch the framerate
-- dive. Adapted from the Playdate SDK bunnymark.
--
-- Controls:
--   left/right       move the spawner crosshair
--   btn1 (Z / pad-A) spawn a batch  (held)
--   btn2 (X / pad-B) remove a batch (held)

local SPAWN_Y = 16
local SPAWNER_SPEED = 240 -- px/s, dpad slide rate
local BATCH_SIZE = 500
local BTN_COOLDOWN = 0.13 -- seconds between spawn ticks while held

-- Physics: per-second units; original Playdate code was per-frame at 60fps.
local GRAVITY = 1800
local DAMPING = 0.85
local MIN_SPEED = 30
local INITIAL_VX_RANGE = 600 -- px/s, +/- this around 0
local INITIAL_VY_RANGE = 600 -- px/s, biased upward (see _init)
local BOUNCE_KICK_MIN = 180  -- px/s upward impulse on floor bounce
local BOUNCE_KICK_RANGE = 240

local function spawn_batch()
  local x, y = State.spawner_x, SPAWN_Y
  for _ = 1, BATCH_SIZE do
    State.count = State.count + 1
    State.bunnies[State.count] = {
      x = x,
      y = y,
      vx = (math.random() - 0.5) * INITIAL_VX_RANGE,
      vy = (math.random() - 0.8) * INITIAL_VY_RANGE,
    }
  end
end

local function despawn_batch()
  local n = State.count - BATCH_SIZE
  if n < 0 then n = 0 end
  for i = State.count, n + 1, -1 do
    State.bunnies[i] = nil
  end
  State.count = n
end

function _config()
  return {
    name = "bunnymark",
    game_id = "com.usagi.bunnymark",
  }
end

function _init()
  State = {
    bunnies = {},
    count = 0,
    spawner_x = usagi.GAME_W / 2,
    cooldown = 0,
    bound_r = usagi.GAME_W - usagi.SPRITE_SIZE,
    bound_b = usagi.GAME_H - usagi.SPRITE_SIZE,
  }
  spawn_batch()
end

function _update(dt)
  -- Slide spawner with left/right.
  local dx = 0
  if input.held(input.LEFT) then dx = dx - SPAWNER_SPEED * dt end
  if input.held(input.RIGHT) then dx = dx + SPAWNER_SPEED * dt end
  if dx ~= 0 then
    State.spawner_x = util.clamp(State.spawner_x + dx, 0, usagi.GAME_W)
  end

  -- Spawn / despawn while held, throttled by cooldown.
  if State.cooldown > 0 then
    State.cooldown = State.cooldown - dt
  elseif input.held(input.BTN1) then
    spawn_batch()
    State.cooldown = BTN_COOLDOWN
  elseif input.held(input.BTN2) then
    despawn_batch()
    State.cooldown = BTN_COOLDOWN
  end

  -- Localize for the hot loop.
  local g = GRAVITY * dt
  local br, bb = State.bound_r, State.bound_b
  local dmp = DAMPING
  local ms = MIN_SPEED
  local abs = math.abs
  local rnd = math.random
  local bs = State.bunnies

  for i = 1, State.count do
    local b = bs[i]
    local vx = b.vx
    local vy = b.vy + g
    local x = b.x + vx * dt
    local y = b.y + vy * dt

    if x < 0 then
      x = 0
      vx = abs(vx) * dmp
      if vx < ms then vx = ms end
    elseif x > br then
      x = br
      vx = -(abs(vx) * dmp)
      if vx > -ms then vx = -ms end
    end

    if y < 0 then
      y = 0
      vy = abs(vy) * dmp
      if vy < ms then vy = ms end
    elseif y > bb then
      y = bb
      vy = -(abs(vy) * dmp)
      if rnd() < 0.5 then
        vy = vy - (BOUNCE_KICK_MIN + rnd() * BOUNCE_KICK_RANGE)
      end
    end

    b.vx = vx
    b.vy = vy
    b.x = x
    b.y = y
  end
end

function _draw(_)
  gfx.clear(gfx.COLOR_BLACK)

  local bs = State.bunnies
  local floor = math.floor
  for i = 1, State.count do
    local b = bs[i]
    gfx.spr(1, floor(b.x), floor(b.y))
  end

  -- Spawner crosshair on the horizontal track.
  local sx = math.floor(State.spawner_x)
  gfx.line(sx - 5, SPAWN_Y, sx + 5, SPAWN_Y, gfx.COLOR_WHITE)
  gfx.line(sx, SPAWN_Y - 5, sx, SPAWN_Y + 5, gfx.COLOR_WHITE)


  gfx.rect_fill(0, 0, 40, 12, gfx.COLOR_WHITE);
  gfx.rect_fill(0, usagi.GAME_H - 14, 100, 14, gfx.COLOR_WHITE)
  gfx.text("Bunnies: " .. State.count, 4, usagi.GAME_H - 12, gfx.COLOR_BLACK)
end
