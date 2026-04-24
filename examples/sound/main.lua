function _update(dt)
  if input.pressed(input.A) then
    sfx.play("jump")
  end
  if input.pressed(input.B) then
    sfx.play("explosion")
  end
end

function _draw(dt)
  gfx.clear(gfx.COLOR_BLACK)
  gfx.text("Press (Z) to play jump.wav", 20, 20, gfx.COLOR_WHITE)
  gfx.text("Press (X) to play explosion.wav", 20, 40, gfx.COLOR_WHITE)
end
