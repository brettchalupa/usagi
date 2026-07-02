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
    local pitch = 0.8 + math.random() * 0.6
    sfx.play_ex("jump", 1.0, pitch, 0.0)
  end
  if input.held(input.UP) then
    local pitch = 0.6 + math.random() * 0.9
    sfx.play_ex("jump", 1.0, pitch, 0.0)
  end
  if input.pressed(input.LEFT) then
    for _ = 1, 6 do
      local pitch = 0.9 + math.random() * 0.3
      sfx.play_ex("jump", 1.0, pitch, 0.0)
    end
  end
  -- Stop a single sound mid-playback (explosion is the longest sample).
  if input.pressed(input.RIGHT) then
    sfx.stop("explosion")
  end
  -- Stop every voice of every sound at once.
  if input.pressed(input.DOWN) then
    sfx.stop_all()
  end
end

function _draw(dt)
  gfx.clear(gfx.COLOR_BLACK)
  local btn1 = input.mapping_for(input.BTN1) or "BTN1"
  local btn2 = input.mapping_for(input.BTN2) or "BTN2"
  local btn3 = input.mapping_for(input.BTN3) or "BTN3"
  local up = input.mapping_for(input.UP) or "UP"
  local down = input.mapping_for(input.DOWN) or "DOWN"
  local left = input.mapping_for(input.LEFT) or "LEFT"
  local right = input.mapping_for(input.RIGHT) or "RIGHT"
  gfx.text("Press " .. btn1 .. " for jump.wav", 20, 14, gfx.COLOR_WHITE)
  gfx.text("Press " .. btn2 .. " for explosion.wav", 20, 34, gfx.COLOR_WHITE)
  gfx.text("Press " .. btn3 .. " for jump.wav with random pitch", 20, 54, gfx.COLOR_WHITE)
  gfx.text("Hold " .. up .. " to layer jump w/ random pitch", 20, 74, gfx.COLOR_YELLOW)
  gfx.text("Tap " .. left .. " for a 6-shot burst", 20, 94, gfx.COLOR_YELLOW)
  gfx.text("Press " .. right .. " to stop explosion.wav", 20, 114, gfx.COLOR_GREEN)
  gfx.text("Press " .. down .. " to stop all sounds", 20, 134, gfx.COLOR_GREEN)

  -- Live playback state via sfx.is_playing.
  local playing = sfx.is_playing("explosion")
  local status = playing and "playing" or "idle"
  local color = playing and gfx.COLOR_GREEN or gfx.COLOR_DARK_GRAY
  gfx.text("explosion.wav: " .. status, 20, 158, color)
end
