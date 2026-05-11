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
  if input.pressed(input.BTN3) then
    -- sfx.play_ex: per-call volume / pitch / pan. Random pitch around
    -- 1.0 gives one .wav file a "varied" feel without committing
    -- 3 different .wav files to the project.
    local pitch = 0.8 + math.random() * 0.6
    sfx.play_ex("jump", 1.0, pitch, 0.0)
  end
end

function _draw(dt)
  gfx.clear(gfx.COLOR_BLACK)
  local btn1 = input.mapping_for(input.BTN1) or "BTN1"
  local btn2 = input.mapping_for(input.BTN2) or "BTN2"
  local btn3 = input.mapping_for(input.BTN3) or "BTN3"
  gfx.text("Press " .. btn1 .. " for jump.wav", 20, 20, gfx.COLOR_WHITE)
  gfx.text("Press " .. btn2 .. " for explosion.wav", 20, 40, gfx.COLOR_WHITE)
  gfx.text("Press " .. btn3 .. " for jump.wav with random pitch", 20, 60, gfx.COLOR_WHITE)
end
