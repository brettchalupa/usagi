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
    toasts = {},
  }
end

local TOAST_TTL = 1.0

local function toast(text)
  table.insert(State.toasts, { text = text, age = 0 })
end

function _update(dt)
  if input.key_pressed(input.KEY_1) then
    effect.hitstop(0.4)
    toast("hitstop")
  end
  if input.key_pressed(input.KEY_2) then
    effect.screen_shake(0.4, 4)
    toast("screen_shake")
  end
  if input.key_pressed(input.KEY_3) then
    effect.flash(0.4, gfx.COLOR_WHITE)
    toast("flash")
  end
  if input.key_pressed(input.KEY_4) then
    effect.slow_mo(1.5, 0.3)
    toast("slow_mo")
  end

  if input.pressed(input.BTN1) then
    effect.hitstop(0.06)
    effect.screen_shake(0.3, 4)
    effect.flash(0.1, gfx.COLOR_WHITE)
    effect.slow_mo(0.8, 0.4)
    toast("combo!")
  end

  for i = #State.toasts, 1, -1 do
    local t = State.toasts[i]
    t.age = t.age + dt
    if t.age >= TOAST_TTL then
      table.remove(State.toasts, i)
    end
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

  -- Toasts: most recent at top, right-aligned. Skip every other
  -- frame in the last 25% of life so they visually "blink out".
  for i, t in ipairs(State.toasts) do
    local life_t = t.age / TOAST_TTL
    local blinking = life_t > 0.75 and math.floor(t.age * 30) % 2 == 0
    if not blinking then
      local w = usagi.measure_text(t.text)
      local x = usagi.GAME_W - w - 6
      local y = 6 + (i - 1) * 10
      gfx.text(t.text, x + 1, y + 1, gfx.COLOR_BLACK)
      gfx.text(t.text, x, y, gfx.COLOR_YELLOW)
    end
  end
end
