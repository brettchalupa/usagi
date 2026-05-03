-- Music playback demo
--
--   music.play(name) -- play once, stop at end
--   music.loop(name) -- play and loop forever
--   music.stop()     -- stop the current track (no-op if nothing's playing)
--
-- Music files live in <project>/music/. Recognized extensions:
-- ogg, mp3, wav, flac. OGG is recommended.
-- File stem becomes the name passed to play/loop,
-- so music/invincible.ogg → music.loop("invincible").

local TRACK = "invincible"
local mode = "stopped"

function _config()
  return { name = "Music Demo" }
end

function _update(_dt)
  if input.pressed(input.BTN1) then
    music.loop(TRACK)
    mode = "looping"
  end
  if input.pressed(input.BTN2) then
    music.play(TRACK)
    mode = "playing once"
  end
  if input.pressed(input.BTN3) then
    music.stop()
    mode = "stopped"
  end
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_BLACK)
  gfx.text("MUSIC DEMO", 10, 10, gfx.COLOR_WHITE)
  gfx.text("track: " .. TRACK, 10, 30, gfx.COLOR_LIGHT_GRAY)
  gfx.text("mode:  " .. mode, 10, 46, gfx.COLOR_YELLOW)

  gfx.text("BTN1: loop", 10, usagi.GAME_H - 50, gfx.COLOR_LIGHT_GRAY)
  gfx.text("BTN2: play once", 10, usagi.GAME_H - 34, gfx.COLOR_LIGHT_GRAY)
  gfx.text("BTN3: stop", 10, usagi.GAME_H - 18, gfx.COLOR_LIGHT_GRAY)
end
