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
    }
  }
end

function _update(dt)
  if input.down(input.LEFT) then
    state.p.x = state.p.x - state.p.spd * dt
  end
  if input.down(input.RIGHT) then
    state.p.x = state.p.x + state.p.spd * dt
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
  gfx.spr(SPR.BUNNY, 20, 20)
  gfx.spr(SPR.SHIP, state.p.x, state.p.y)
  gfx.spr(SPR.BULLET_SM, 20, 40)
  gfx.spr(SPR.BULLET_LG, 50, 40)
end
