-- Demonstrates all four engine juice primitives:
--   1 -> effect.hitstop      freezes _update for a beat
--   2 -> effect.screen_shake offsets the blit, decays linearly
--   3 -> effect.flash        full-screen color overlay, fades out
--   4 -> effect.slow_mo      scales dt for cinematic moments
--   Z (BTN1) -> all four together, the classic "big hit" combo
--
-- A bouncing dot makes hitstop and slow_mo visible: the dot pauses
-- entirely under hitstop, glides under slow_mo. Shake rattles the
-- whole view; flash flashes over it.

function _config()
  return { name = "effect demo" }
end

function _init()
  State = {
    x = 40,
    y = 40,
    vx = 90,
    vy = 60,
  }
end

function _update(dt)
  if input.key_pressed(input.KEY_1) then effect.hitstop(0.4) end
  if input.key_pressed(input.KEY_2) then effect.screen_shake(0.4, 4) end
  if input.key_pressed(input.KEY_3) then effect.flash(0.4, gfx.COLOR_WHITE) end
  if input.key_pressed(input.KEY_4) then effect.slow_mo(1.5, 0.3) end

  if input.pressed(input.BTN1) then
    effect.hitstop(0.06)
    effect.screen_shake(0.3, 4)
    effect.flash(0.1, gfx.COLOR_WHITE)
    effect.slow_mo(0.8, 0.4)
  end

  State.x = State.x + State.vx * dt
  State.y = State.y + State.vy * dt
  if State.x < 4 or State.x > usagi.GAME_W - 4 then State.vx = -State.vx end
  if State.y < 4 or State.y > usagi.GAME_H - 4 then State.vy = -State.vy end
end

function _draw(dt)
  gfx.clear(gfx.COLOR_DARK_BLUE)
  gfx.circ_fill(State.x, State.y, 4, gfx.COLOR_YELLOW)

  gfx.text("effect demo", 6, 6, gfx.COLOR_WHITE)
  gfx.text("1  hitstop", 6, 24, gfx.COLOR_LIGHT_GRAY)
  gfx.text("2  screen_shake", 6, 34, gfx.COLOR_LIGHT_GRAY)
  gfx.text("3  flash", 6, 44, gfx.COLOR_LIGHT_GRAY)
  gfx.text("4  slow_mo", 6, 54, gfx.COLOR_LIGHT_GRAY)
  local btn = input.mapping_for(input.BTN1)
  gfx.text(btn .. "  combo", 6, 68, gfx.COLOR_PINK)
end
