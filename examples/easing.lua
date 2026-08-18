-- name = easing functions demo

-- Demonstrates and compares all easing functions in usagi.

function _init()
  Functions = {
    "lerp",
    "sine_in",
    "sine_out",
    "sine_in_out",
    "circ_in",
    "circ_out",
    "circ_in_out",
    "expo_in",
    "expo_out",
    "expo_in_out",
  }

  State = {
    time = 0,
    reverse = false,
  }
end

function _update()
  local t = usagi.elapsed % 5
  if t < 2 then
    State.time = t / 2
    State.reverse = false
  elseif t < 2.5 then
    State.time = 1
    State.reverse = false
  elseif t < 4.5 then
    State.time = (t - 2.5) / 2
    State.reverse = true
  else
    State.time = 0
    State.reverse = false
  end
end

function _draw()
  gfx.clear(gfx.COLOR_DARK_BLUE)
  gfx.text(util.round(usagi.elapsed * 100) / 100, 0, 0, gfx.COLOR_WHITE)
  gfx.text(util.round(State.time * 100) / 100, 0, 10, gfx.COLOR_WHITE)
  for i, v in ipairs(Functions) do
    local x
    if v == "lerp" then
      x = util.lerp(80, 240, State.time)
    else
      x = util.ease[v](80, 240, State.time)
    end

    if State.reverse then
      x = 320 - x
    end
    gfx.text(string.upper(v), 1, 13 + (i * 15), gfx.COLOR_WHITE)
    gfx.rect(x, 15 + (i * 15), 10, 10, gfx.COLOR_WHITE)
  end
end
