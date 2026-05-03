function _config()
  return { name = "Sound" }
end

function _update(dt)
  if input.pressed(input.BTN1) then
    sfx.play("jump")
  end
  if input.pressed(input.BTN2) then
    sfx.play("explosion")
  end
end

function _draw(dt)
  gfx.clear(gfx.COLOR_BLACK)
  gfx.text("Press BTN1 for jump.wav", 20, 20, gfx.COLOR_WHITE)
  gfx.text("Press BTN2 for explosion.wav", 20, 40, gfx.COLOR_WHITE)
end
