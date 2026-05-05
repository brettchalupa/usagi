-- Game-feel state: floating score popups + shader pulse. Screen
-- shake is handled by the engine's `effect.screen_shake` API.

local M = {}

function M.new()
  return {
    popups = {},
    pulse = 0,
    pulse_timer = 0,
    pulse_dur = 0,
    pulse_y = 0.5,
  }
end

function M.trigger_pulse(fx, y_norm, dur, strength)
  fx.pulse = strength or 1
  fx.pulse_timer = dur
  fx.pulse_dur = dur
  fx.pulse_y = y_norm
end

function M.pulse_value(fx)
  if fx.pulse_dur <= 0 then
    return 0
  end
  return fx.pulse * (fx.pulse_timer / fx.pulse_dur)
end

function M.add_popup(fx, text, cx, cy, color)
  local w = usagi.measure_text(text)
  table.insert(fx.popups, {
    text = text,
    x = cx - w / 2,
    y = cy,
    age = 0,
    ttl = 0.8,
    color = color or gfx.COLOR_WHITE,
  })
end

function M.update(fx, dt)
  if fx.pulse_timer > 0 then
    fx.pulse_timer = fx.pulse_timer - dt
    if fx.pulse_timer <= 0 then
      fx.pulse = 0
      fx.pulse_timer = 0
      fx.pulse_dur = 0
    end
  end
  for i = #fx.popups, 1, -1 do
    local p = fx.popups[i]
    p.age = p.age + dt
    if p.age >= p.ttl then
      table.remove(fx.popups, i)
    end
  end
end

function M.draw_popups(fx)
  for _, p in ipairs(fx.popups) do
    local t = p.age / p.ttl
    -- Skip every other frame in the last 25% of life so it visually "blinks out".
    if not (t > 0.75 and math.floor(p.age * 30) % 2 == 0) then
      gfx.text(p.text, p.x + 1, p.y - t * 14 + 1, gfx.COLOR_BLACK)
      gfx.text(p.text, p.x, p.y - t * 14, p.color)
    end
  end
end

return M
