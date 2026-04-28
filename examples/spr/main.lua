local SPR = {
  BUNNY = 1,
  SHIP = 2,
  BULLET_LG = 3,
  BULLET_SM = 4,
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
    }
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

  -- gfx.pixel: single-pixel draw. Drives a small sparkle field that
  -- drifts with usagi.elapsed so the screen feels alive.
  for i = 1, 24 do
    local x = (i * 13 + math.floor(usagi.elapsed * 30)) % usagi.GAME_W
    local y = 80 + (i * 7) % 40
    gfx.pixel(x, y, gfx.COLOR_WHITE)
  end

  gfx.text("LEFT/RIGHT to flip the ship", 4, usagi.GAME_H - 10, gfx.COLOR_WHITE)
end
