function _config()
  return { title = "Input Test" }
end

function _init()
  left_down = false
  right_down = false
  up_down = false
  down_down = false
  btn1_down = false
  btn2_down = false
  btn3_down = false
end

function _update(_dt)
  up_down = input.down(input.UP)
  down_down = input.down(input.DOWN)
  left_down = input.down(input.LEFT)
  right_down = input.down(input.RIGHT)
  btn1_down = input.down(input.BTN1)
  btn2_down = input.down(input.BTN2)
  btn3_down = input.down(input.BTN3)
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_BLACK)

  gfx.text("INPUT TEST", 10, 10, gfx.COLOR_WHITE)

  if up_down then
    gfx.spr(2, 60, 40)
  else
    gfx.spr(1, 60, 40)
  end
  gfx.text("UP", 60, 60, gfx.COLOR_WHITE)

  if down_down then
    gfx.spr(2, 60, 80)
  else
    gfx.spr(1, 60, 80)
  end
  gfx.text("DOWN", 60, 100, gfx.COLOR_WHITE)

  if left_down then
    gfx.spr(2, 20, 60)
  else
    gfx.spr(1, 20, 60)
  end
  gfx.text("LEFT", 20, 80, gfx.COLOR_WHITE)

  if right_down then
    gfx.spr(2, 100, 60)
  else
    gfx.spr(1, 100, 60)
  end
  gfx.text("RIGHT", 100, 80, gfx.COLOR_WHITE)

  if btn1_down then
    gfx.spr(2, 180, 30)
  else
    gfx.spr(1, 180, 30)
  end
  gfx.text("BTN1", 180, 50, gfx.COLOR_WHITE)

  if btn2_down then
    gfx.spr(2, 180, 70)
  else
    gfx.spr(1, 180, 70)
  end
  gfx.text("BTN2", 180, 90, gfx.COLOR_WHITE)

  if btn3_down then
    gfx.spr(2, 180, 110)
  else
    gfx.spr(1, 180, 110)
  end
  gfx.text("BTN3", 180, 130, gfx.COLOR_WHITE)
end
