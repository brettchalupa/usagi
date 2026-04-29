-- Game-feel state: screenshake + floating score popups. Owns its own
-- shape; main.lua keeps an `fx` instance and threads it through.

local M = {}

function M.new()
  return {
    shake = 0,
    shake_timer = 0,
    popups = {},
  }
end

function M.trigger_shake(fx, mag, dur)
  if mag > fx.shake then
    fx.shake = mag
  end
  if dur > fx.shake_timer then
    fx.shake_timer = dur
  end
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
  if fx.shake_timer > 0 then
    fx.shake_timer = fx.shake_timer - dt
    if fx.shake_timer <= 0 then
      fx.shake = 0
      fx.shake_timer = 0
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

function M.shake_offset(fx)
  local mag = math.floor(fx.shake)
  if mag <= 0 then
    return 0, 0
  end
  return math.random(-mag, mag), math.random(-mag, mag)
end

function M.draw_popups(fx, sx, sy)
  for _, p in ipairs(fx.popups) do
    local t = p.age / p.ttl
    -- Skip every other frame in the last 25% of life so it visually "blinks out".
    if not (t > 0.75 and math.floor(p.age * 30) % 2 == 0) then
      gfx.text(p.text, p.x + sx + 1, p.y + sy - t * 14 + 1, gfx.COLOR_BLACK)
      gfx.text(p.text, p.x + sx, p.y + sy - t * 14, p.color)
    end
  end
end

return M
