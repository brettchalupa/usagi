-- Compound assignment operators are sugar for `lhs = lhs op (rhs)`.
-- Supported: +=, -=, *=, /=, %=
-- They only apply when the operator is at the start of a logical line;
-- compound ops inside `if cond then x += 1 end` are NOT rewritten.

function _config()
  return { title = "Operators" }
end

function _init()
  state = {
    score = 0,
    timer = 0,
    pulse = 1,
    bumps = 0,
  }
end

function _update(dt)
  state.timer += dt
  state.score += 1
  state.pulse *= 0.99
  if state.pulse < 0.2 then
    state.pulse = 1
  end
  if input.pressed(input.BTN1) then
    state.bumps += 1
  end
  if input.pressed(input.BTN2) then
    state.bumps = 0
  end
end

function _draw(dt)
  gfx.clear(gfx.COLOR_DARK_BLUE)
  gfx.text("score   " .. state.score, 8, 8, gfx.COLOR_WHITE)
  gfx.text("timer   " .. string.format("%.1f", state.timer), 8, 20, gfx.COLOR_WHITE)
  gfx.text("pulse   " .. string.format("%.2f", state.pulse), 8, 32, gfx.COLOR_WHITE)
  gfx.text("BTN1 bumps   " .. state.bumps, 8, 52, gfx.COLOR_YELLOW)
  gfx.text("BTN2 resets bumps", 8, 64, gfx.COLOR_YELLOW)
end
