-- Live reload preserves globals but re-runs the chunk, so locals get
-- fresh nil bindings each save. Keep mutable state in globals (assigned
-- only in _init); keep constants local. F5 calls _init to reset.

function _config()
  return { title = "Input" }
end

function _init()
  state = {
    x = 32,
    y = 32,
    spd = 200,
  }
end

function _update(dt)
  if input.down(input.LEFT) then
    state.x = state.x - state.spd * dt
  end
  if input.down(input.RIGHT) then
    state.x = state.x + state.spd * dt
  end
  if input.down(input.DOWN) then
    state.y = state.y + state.spd * dt
  end
  if input.down(input.UP) then
    state.y = state.y - state.spd * dt
  end
end

function _draw(dt)
  gfx.clear(gfx.COLOR_WHITE)
  gfx.text("Move the square around", 4, 4, gfx.COLOR_DARK_GRAY)
  gfx.rect_fill(state.x, state.y, 16, 16, gfx.COLOR_DARK_GRAY)
end
