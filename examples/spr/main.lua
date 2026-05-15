local SPR = {
  BUNNY = 1,
  SHIP = 2,
  BULLET_LG = 3,
  BULLET_SM = 4,
}

-- Warm palette cycle for the ship's exhaust trail.
local EXHAUST_COLORS = {
  gfx.COLOR_YELLOW,
  gfx.COLOR_ORANGE,
  gfx.COLOR_RED,
  gfx.COLOR_BROWN,
}

function _config()
  return { name = "Sprites", icon = 1 }
end

local function clamp(value, min, max)
  if value > max then
    return max
  end
  if value < min then
    return min
  end
  return value
end

function _init()
  State = {
    p = {
      x = 50,
      y = 20,
      spd = 200,
      face_left = false,
    },
    sparks = {},
    -- Seconds remaining on the "got hit" tint flash; BTN1 retriggers it.
    hit_flash = 0,
  }
end

local function emit_spark()
  -- Ship is 16×16 and points up (top-down view), so the exhaust
  -- spawns at the bottom edge and trails downward.
  local tail_x = State.p.x + 6 + math.floor(math.random() * 4)
  local tail_y = State.p.y + 16
  State.sparks[#State.sparks + 1] = {
    x = tail_x,
    y = tail_y,
    vx = math.random() * 20 - 10,
    vy = 40 + math.random() * 40,
    life = 0.4 + math.random() * 0.3,
    color = EXHAUST_COLORS[1 + math.floor(math.random() * #EXHAUST_COLORS)],
  }
end

function _update(dt)
  if input.held(input.LEFT) then
    State.p.x = State.p.x - State.p.spd * dt
    State.p.face_left = true
  end
  if input.held(input.RIGHT) then
    State.p.x = State.p.x + State.p.spd * dt
    State.p.face_left = false
  end
  if input.held(input.DOWN) then
    State.p.y = State.p.y + State.p.spd * dt
  end
  if input.held(input.UP) then
    State.p.y = State.p.y - State.p.spd * dt
  end
  if input.pressed(input.BTN1) then
    State.hit_flash = 0.2
  end
  State.hit_flash = math.max(0, State.hit_flash - dt)

  State.p.x = clamp(State.p.x, 0, usagi.GAME_W)
  State.p.y = clamp(State.p.y, 0, usagi.GAME_H)

  -- Two sparks per frame, then update positions and drop dead ones.
  emit_spark()
  emit_spark()
  for i = #State.sparks, 1, -1 do
    local s = State.sparks[i]
    s.x = s.x + s.vx * dt
    s.y = s.y + s.vy * dt
    s.life = s.life - dt
    if s.life <= 0 then
      table.remove(State.sparks, i)
    end
  end
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_BLUE)

  -- gfx.spr is the simple 3-arg form. Use it for static, native-size,
  -- never-rotated, never-tinted sprites.
  gfx.spr(SPR.BULLET_SM, 20, 40)

  -- gfx.spr_ex: full power. Required params: flip_x, flip_y, rotation
  -- (radians; use math.rad(deg) for literal-degree values), tint
  -- (gfx.COLOR_TRUE_WHITE = no recolor; gfx.COLOR_WHITE is the Pico-8
  -- white and will shift colors slightly), alpha (0..1; 1 = opaque).
  --
  -- Spinning bunny: rotation is the elapsed time scaled to ~1 turn/sec.
  gfx.spr_ex(SPR.BUNNY, 20, 20, false, false, usagi.elapsed * 2, gfx.COLOR_TRUE_WHITE, 1.0)
  -- Ship tints red briefly after a BTN1 press to show the tint param.
  local ship_tint = State.hit_flash > 0 and gfx.COLOR_RED or gfx.COLOR_TRUE_WHITE
  gfx.spr_ex(SPR.SHIP, State.p.x, State.p.y, State.p.face_left, false, 0, ship_tint, 1.0)
  -- Pulsing alpha on the big bullet: sin wave between 0.2 and 1.0.
  local pulse = 0.6 + 0.4 * math.sin(usagi.elapsed * 4)
  gfx.spr_ex(SPR.BULLET_LG, 50, 40, false, false, 0, gfx.COLOR_TRUE_WHITE, pulse)

  -- gfx.sspr_ex: extended source-rect draw with flipping + rotation +
  -- tint + alpha. The new params trail the legacy ones: same identity
  -- values mean "draw it normally": 0 rotation, COLOR_TRUE_WHITE tint,
  -- 1.0 alpha.
  gfx.sspr_ex(0, 32, 32, 32, 200, 20, 32, 32, false, false, 0, gfx.COLOR_TRUE_WHITE, 1.0)
  gfx.sspr_ex(0, 32, 32, 32, 200, 62, 32, 32, true, false, 0, gfx.COLOR_TRUE_WHITE, 1.0)
  gfx.sspr_ex(0, 32, 32, 32, 240, 62, 32, 32, true, true, 0, gfx.COLOR_TRUE_WHITE, 1.0)
  -- A rotating + tinted variant to show both at once.
  gfx.sspr_ex(0, 32, 32, 32, 240, 20, 32, 32, false, false,
              usagi.elapsed, gfx.COLOR_PINK, 1.0)

  -- gfx.sspr is the simple 1:1 form for repeated tile draws.
  gfx.sspr(0, 32, 32, 32, 200, 100)
  gfx.sspr(0, 32, 32, 32, 240, 100)

  -- Ship exhaust particle emitter: each spark is one pixel via
  -- gfx.pixel, the engine's single-pixel draw.
  for _, s in ipairs(State.sparks) do
    gfx.pixel(s.x, s.y, s.color)
  end

  gfx.text("LEFT/RIGHT to flip ship   BTN1 to flash tint", 4, usagi.GAME_H - 10, gfx.COLOR_WHITE)
end
