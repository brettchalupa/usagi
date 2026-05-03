-- Compound assignment operators are sugar for `lhs = lhs op (rhs)`.
-- Supported: +=, -=, *=, /=, %=
-- They only apply when the operator is at the start of a logical line;
-- compound ops inside `if cond then x += 1 end` are NOT rewritten.

function _config()
  return { name = "Operators" }
end

function _init()
  State = {
    score = 0,
    timer = 0,
    pulse = 1,
    bumps = 0,
  }
end

function _update(dt)
  State.timer += dt
  State.score += 1
  State.pulse *= 0.99
  if State.pulse < 0.2 then
    State.pulse = 1
  end
  if input.pressed(input.BTN1) then
    State.bumps += 1
  end
  if input.pressed(input.BTN2) then
    State.bumps = 0
  end
end

function _draw(dt)
  gfx.clear(gfx.COLOR_DARK_BLUE)
  gfx.text("score   " .. State.score, 8, 8, gfx.COLOR_WHITE)
  gfx.text("timer   " .. string.format("%.1f", State.timer), 8, 20, gfx.COLOR_WHITE)
  gfx.text("pulse   " .. string.format("%.2f", State.pulse), 8, 32, gfx.COLOR_WHITE)
  gfx.text("BTN1 bumps   " .. State.bumps, 8, 52, gfx.COLOR_YELLOW)
  gfx.text("BTN2 resets bumps", 8, 64, gfx.COLOR_YELLOW)
end
