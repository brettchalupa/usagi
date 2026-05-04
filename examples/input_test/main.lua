function _config()
  return { name = "Input Test" }
end

function _init()
  State = {
    left_down = false,
    right_down = false,
    up_down = false,
    down_down = false,
    btn1_down = false,
    btn2_down = false,
    btn3_down = false,
  }
end

function _update(_dt)
  State.up_down = input.down(input.UP)
  State.down_down = input.down(input.DOWN)
  State.left_down = input.down(input.LEFT)
  State.right_down = input.down(input.RIGHT)
  State.btn1_down = input.down(input.BTN1)
  State.btn2_down = input.down(input.BTN2)
  State.btn3_down = input.down(input.BTN3)
end

local function label_for(action)
  return input.mapping_for(action) or "--"
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_BLACK)

  gfx.text("INPUT TEST", 10, 10, gfx.COLOR_WHITE)
  gfx.text("LAST SOURCE: " .. string.upper(input.last_source()), 192, 10, gfx.COLOR_PEACH)

  if State.up_down then
    gfx.spr(2, 60, 40)
  else
    gfx.spr(1, 60, 40)
  end
  gfx.text("UP: " .. label_for(input.UP), 60, 60, gfx.COLOR_WHITE)

  if State.down_down then
    gfx.spr(2, 60, 100)
  else
    gfx.spr(1, 60, 100)
  end
  gfx.text("DOWN: " .. label_for(input.DOWN), 60, 120, gfx.COLOR_WHITE)

  if State.left_down then
    gfx.spr(2, 20, 72)
  else
    gfx.spr(1, 20, 72)
  end
  gfx.text("LEFT: " .. label_for(input.LEFT), 20, 92, gfx.COLOR_WHITE)

  if State.right_down then
    gfx.spr(2, 100, 72)
  else
    gfx.spr(1, 100, 72)
  end
  gfx.text("RIGHT: " .. label_for(input.RIGHT), 100, 92, gfx.COLOR_WHITE)

  if State.btn1_down then
    gfx.spr(2, 180, 30)
  else
    gfx.spr(1, 180, 30)
  end
  gfx.text("BTN1: " .. label_for(input.BTN1), 180, 50, gfx.COLOR_WHITE)

  if State.btn2_down then
    gfx.spr(2, 180, 70)
  else
    gfx.spr(1, 180, 70)
  end
  gfx.text("BTN2: " .. label_for(input.BTN2), 180, 90, gfx.COLOR_WHITE)

  if State.btn3_down then
    gfx.spr(2, 180, 110)
  else
    gfx.spr(1, 180, 110)
  end
  gfx.text("BTN3: " .. label_for(input.BTN3), 180, 130, gfx.COLOR_WHITE)
end
