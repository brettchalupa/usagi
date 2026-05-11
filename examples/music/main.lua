-- Music playback demo
--
--   music.play(name)                            -- play once, stop at end
--   music.loop(name)                            -- play and loop forever
--   music.stop()                                -- stop the current track
--   music.play_ex(name, vol, pitch, pan, loop)  -- play with initial params
--   music.mutate(vol, pitch, pan)               -- modulate currently playing
--
-- Music files live in <project>/music/. Recognized extensions:
-- ogg, mp3, wav, flac. OGG is recommended. File stem becomes the
-- name passed to play/loop, so music/invincible.ogg → music.loop("invincible").

local TRACK = "invincible"

function _config()
  return { name = "Music Demo" }
end

function _init()
  State = {
    mode = "stopped",
    -- Live values currently sent to music.mutate each frame.
    volume = 1.0,
    pitch = 1.0,
    pan = 0.0,
  }
end

-- All three params tween toward a target driven by held inputs and
-- ramp back to identity when nothing is held. Demonstrates that
-- music.mutate is a per-frame set — you compose your own tweens in
-- Lua state.
function _update(dt)
  if input.pressed(input.BTN1) then
    music.loop(TRACK)
    State.mode = "looping"
  end
  if input.pressed(input.BTN2) then
    music.play(TRACK)
    State.mode = "playing once"
  end

  -- Hold BTN3 to duck the volume to 0.3 (sting-style under-dialog
  -- effect). Releasing ramps back to 1.0.
  local target_volume = input.held(input.BTN3) and 0.3 or 1.0
  -- UP / DOWN warp pitch up or down.
  local target_pitch = 1.0
  if input.held(input.UP) then
    target_pitch = 1.5
  elseif input.held(input.DOWN) then
    target_pitch = 0.5
  end
  -- LEFT / RIGHT pan the music.
  local target_pan = 0.0
  if input.held(input.LEFT) then
    target_pan = -1.0
  elseif input.held(input.RIGHT) then
    target_pan = 1.0
  end

  -- Each param tweens at its own rate; pan is fast for snappy stereo
  -- pings, pitch slightly slower, volume the slowest.
  State.volume = util.approach(State.volume, target_volume, dt * 2)
  State.pitch = util.approach(State.pitch, target_pitch, dt * 4)
  State.pan = util.approach(State.pan, target_pan, dt * 6)

  music.mutate(State.volume, State.pitch, State.pan)
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_BLACK)
  gfx.text("MUSIC DEMO", 10, 10, gfx.COLOR_WHITE)
  gfx.text("track: " .. TRACK, 10, 30, gfx.COLOR_LIGHT_GRAY)
  gfx.text("mode:  " .. State.mode, 10, 46, gfx.COLOR_YELLOW)

  gfx.text(string.format("volume: %.2f", State.volume), 10, 66, gfx.COLOR_LIGHT_GRAY)
  gfx.text(string.format("pitch:  %.2f", State.pitch), 10, 78, gfx.COLOR_LIGHT_GRAY)
  gfx.text(string.format("pan:   %+.2f", State.pan), 10, 90, gfx.COLOR_LIGHT_GRAY)

  local b1 = input.mapping_for(input.BTN1) or "BTN1"
  local b2 = input.mapping_for(input.BTN2) or "BTN2"
  local b3 = input.mapping_for(input.BTN3) or "BTN3"
  local up = input.mapping_for(input.UP) or "UP"
  local down = input.mapping_for(input.DOWN) or "DOWN"
  local left = input.mapping_for(input.LEFT) or "LEFT"
  local right = input.mapping_for(input.RIGHT) or "RIGHT"

  gfx.text(b1 .. ": loop   " .. b2 .. ": play once", 10, usagi.GAME_H - 60, gfx.COLOR_LIGHT_GRAY)
  gfx.text("hold " .. b3 .. ": duck volume", 10, usagi.GAME_H - 46, gfx.COLOR_LIGHT_GRAY)
  gfx.text("hold " .. up .. " / " .. down .. ": pitch up / down", 10, usagi.GAME_H - 32, gfx.COLOR_LIGHT_GRAY)
  gfx.text("hold " .. left .. " / " .. right .. ": pan left / right", 10, usagi.GAME_H - 18, gfx.COLOR_LIGHT_GRAY)
end
