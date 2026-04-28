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
  return { title = "Sprites" }
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
  state = {
    p = {
      x = 50,
      y = 20,
      spd = 200,
      face_left = false,
    },
    sparks = {},
  }
end

local function emit_spark()
  -- Ship is 16×16 and points up (top-down view), so the exhaust
  -- spawns at the bottom edge and trails downward.
  local tail_x = state.p.x + 6 + math.floor(math.random() * 4)
  local tail_y = state.p.y + 16
  state.sparks[#state.sparks + 1] = {
    x = tail_x,
    y = tail_y,
    vx = math.random() * 20 - 10,
    vy = 40 + math.random() * 40,
    life = 0.4 + math.random() * 0.3,
    color = EXHAUST_COLORS[1 + math.floor(math.random() * #EXHAUST_COLORS)],
  }
end

function _update(dt)
  if input.down(input.LEFT) then
    state.p.x = state.p.x - state.p.spd * dt
    state.p.face_left = true
  end
  if input.down(input.RIGHT) then
    state.p.x = state.p.x + state.p.spd * dt
    state.p.face_left = false
  end
  if input.down(input.DOWN) then
    state.p.y = state.p.y + state.p.spd * dt
  end
  if input.down(input.UP) then
    state.p.y = state.p.y - state.p.spd * dt
  end
  if input.pressed(input.BTN1) then
    print("fire!")
  end

  state.p.x = clamp(state.p.x, 0, usagi.GAME_W)
  state.p.y = clamp(state.p.y, 0, usagi.GAME_H)

  -- Two sparks per frame, then update positions and drop dead ones.
  emit_spark()
  emit_spark()
  for i = #state.sparks, 1, -1 do
    local s = state.sparks[i]
    s.x = s.x + s.vx * dt
    s.y = s.y + s.vy * dt
    s.life = s.life - dt
    if s.life <= 0 then
      table.remove(state.sparks, i)
    end
  end
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_BLUE)

  -- gfx.spr / gfx.spr_ex: basic vs extended sprite draw. `spr_ex` takes
  -- both flip booleans (required) so one art asset covers both facings.
  gfx.spr(SPR.BUNNY, 20, 20)
  gfx.spr_ex(SPR.SHIP, state.p.x, state.p.y, state.p.face_left, false)
  gfx.spr(SPR.BULLET_SM, 20, 40)
  gfx.spr(SPR.BULLET_LG, 50, 40)

  -- gfx.sspr_ex: extended source-rect draw with flipping
  gfx.sspr_ex(0, 32, 32, 32, 200, 20, 32, 32, false, false)
  gfx.sspr_ex(0, 32, 32, 32, 200, 62, 32, 32, true, false)
  gfx.sspr_ex(0, 32, 32, 32, 240, 62, 32, 32, true, true)

  -- gfx.sspr is the simple 1:1 form for repeated tile draws.
  gfx.sspr(0, 32, 32, 32, 200, 100)
  gfx.sspr(0, 32, 32, 32, 240, 100)

  -- Ship exhaust particle emitter: each spark is one pixel via
  -- gfx.pixel, the engine's single-pixel draw.
  for _, s in ipairs(state.sparks) do
    gfx.pixel(s.x, s.y, s.color)
  end

  gfx.text("LEFT/RIGHT to flip the ship", 4, usagi.GAME_H - 10, gfx.COLOR_WHITE)
end
