-- Lua's `math.random` is already wired up: usagi's Lua State auto-seeds
-- the PRNG at startup, so a fresh launch produces a fresh sequence.
-- Press BTN1 to reroll the scene from the current PRNG. Press BTN2
-- to pin `math.randomseed(42)` so you can watch the same sequence
-- replay across runs.

local DOTS = 180
local SAMPLES = 6

function _config()
  return { title = "RNG" }
end

local function reroll()
  State.dots = {}
  for i = 1, DOTS do
    State.dots[i] = {
      x = math.random(0, usagi.GAME_W - 1),
      y = math.random(20, usagi.GAME_H - 1),
      c = math.random(1, 15),
      r = math.random(1, 3),
    }
  end

  State.samples = {}
  for i = 1, SAMPLES do
    State.samples[i] = math.random(0, 999)
  end
end

function _init()
  State = { pinned = false }
  reroll()
end

function _update(_dt)
  if input.pressed(input.BTN1) then
    reroll()
  end
  if input.pressed(input.BTN2) then
    math.randomseed(42)
    State.pinned = true
    reroll()
  end
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_BLACK)

  for _, d in ipairs(State.dots) do
    gfx.circ_fill(d.x, d.y, d.r, d.c)
  end

  gfx.text("rng demo", 4, 4, gfx.COLOR_WHITE)

  local label = "samples:"
  for _, n in ipairs(State.samples) do
    label = label .. " " .. n
  end
  gfx.text(label, 4, 12, gfx.COLOR_LIGHT_GRAY)

  local hint = "BTN1: reroll  BTN2: seed(42)"
  if State.pinned then
    hint = hint .. "  [pinned]"
  end
  gfx.text(hint, 4, usagi.GAME_H - 10, gfx.COLOR_PEACH)
end
