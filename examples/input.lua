-- Live reload preserves globals but re-runs the chunk, so locals get
-- fresh nil bindings each save. Keep mutable game state in a
-- capitalized global (assigned only in _init); keep constants local.
-- F5 calls _init to reset.

function _config()
  return { name = "Input" }
end

function _init()
  State = {
    x = 32,
    y = 32,
    spd = 200,
  }
end

function _update(dt)
  if input.down(input.LEFT) then
    State.x = State.x - State.spd * dt
  end
  if input.down(input.RIGHT) then
    State.x = State.x + State.spd * dt
  end
  if input.down(input.DOWN) then
    State.y = State.y + State.spd * dt
  end
  if input.down(input.UP) then
    State.y = State.y - State.spd * dt
  end
end

function _draw(dt)
  gfx.clear(gfx.COLOR_WHITE)
  gfx.text("Move the square around", 4, 4, gfx.COLOR_DARK_GRAY)
  gfx.rect_fill(State.x, State.y, 16, 16, gfx.COLOR_DARK_GRAY)
end
