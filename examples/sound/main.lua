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
end

function _draw(dt)
  gfx.clear(gfx.COLOR_BLACK)
  local btn1 = input.mapping_for(input.BTN1) or "BTN1"
  local btn2 = input.mapping_for(input.BTN2) or "BTN2"
  local btn3 = input.mapping_for(input.BTN3) or "BTN3"
  local up = input.mapping_for(input.UP) or "UP"
  local left = input.mapping_for(input.LEFT) or "LEFT"
  gfx.text("Press " .. btn1 .. " for jump.wav", 20, 20, gfx.COLOR_WHITE)
  gfx.text("Press " .. btn2 .. " for explosion.wav", 20, 40, gfx.COLOR_WHITE)
  gfx.text("Press " .. btn3 .. " for jump.wav with random pitch", 20, 60, gfx.COLOR_WHITE)
  gfx.text("Hold " .. up .. " to layer jump w/ random pitch", 20, 90, gfx.COLOR_YELLOW)
  gfx.text("Tap " .. left .. " for a 6-shot burst", 20, 110, gfx.COLOR_YELLOW)
end
