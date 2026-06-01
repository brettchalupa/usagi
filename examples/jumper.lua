-- A tiny platformer that shows the synth engine in actual gameplay: every
-- sound is generated on the audio thread (no .wav files), and the arcade
-- "blip" character comes from `slide` -- a per-sample pitch bend in
-- semitones (see sfx.synth in meta/usagi.lua).
--
--   LEFT / RIGHT : walk
--   BTN1 (Z / A) : jump
--
-- Land on platforms, grab the coins. Every sound is synthesized live:
--   jump  : square, slides UP   -> classic "boing"
--   land  : square slide DOWN + a noise burst -> a short thud
--   coin  : two quick blips a fifth apart -> pickup jingle
--   bass  : a step-sequenced square riff
--   pad   : a sustained saw chord progression (Am -> Em)

local GRAVITY = 520   -- px/s^2
local MOVE_SPEED = 90 -- px/s
local JUMP_VEL = -250 -- px/s (negative = up)

-- Equal-temperament frequency for a MIDI note (69 = A4 = 440 Hz).
local function midi_to_freq(midi)
  return 440 * 2 ^ ((midi - 69) / 12)
end

-- Semitone offset within an octave for each natural note name.
local NOTE_SEMITONES = { C = 0, D = 2, E = 4, F = 5, G = 7, A = 9, B = 11 }

-- Parse a note name like "A2", "C#4", "Eb3" into a frequency in Hz. A rest is
-- written as "-" (or nil / 0) and returns 0. Octave is scientific pitch
-- notation: C4 = middle C, A4 = 440 Hz.
local function note(name)
  if not name or name == "-" or name == 0 then return 0 end
  local letter, accidental, octave = name:match("^([A-Ga-g])([#b]?)(%-?%d+)$")
  assert(letter, "bad note name: " .. tostring(name))
  local semitone = NOTE_SEMITONES[letter:upper()]
  if accidental == "#" then
    semitone = semitone + 1
  elseif accidental == "b" then
    semitone = semitone - 1
  end
  local midi = (tonumber(octave) + 1) * 12 + semitone
  return midi_to_freq(midi)
end

-- Convert a list of note names to a list of frequencies, in order.
local function notes(names)
  local freqs = {}
  for i, name in ipairs(names) do
    freqs[i] = note(name)
  end
  return freqs
end

-- One-shot synth sfx. Each is a single sfx.synth call; `slide` is what gives
-- them their arcade shape.
local function sfx_jump()
  sfx.synth({ wave = sfx.SQUARE, freq = 220, slide = 18, slide_ms = 90, volume = 0.35, decay = 110 })
end

local function sfx_land()
  sfx.synth({ wave = sfx.SQUARE, freq = 100, slide = -12, slide_ms = 70, volume = 0.2, decay = 80 })
  sfx.synth({ wave = sfx.NOISE, freq = 180, volume = 0.25 })
end

local function sfx_coin()
  -- Two blips: root, then a fifth up a moment later -> a little jingle.
  sfx.synth({ wave = sfx.SQUARE, freq = 880, slide = 0, volume = 0.3, decay = 70 })
  sfx.synth({ wave = sfx.SQUARE, freq = 1320, slide = 7, slide_ms = 60, volume = 0.3, attack = 60, decay = 90 })
end

-- A bass riff as a step sequencer. Written as note names; "-" is a rest.
local BASS_NAMES = {
  "A2", "-", "A2", "-", "A2", "C3", "D3", "-",
  "A2", "-", "A2", "-", "C3", "D3", "E3", "-",
  "E2", "-", "E2", "-", "E3", "D3", "C3", "-",
  "E2", "-", "E2", "-", "D3", "C3", "B2", "-",
}
local BASS_STEPS = notes(BASS_NAMES)
local STEP_SECONDS = 0.18 -- time per step (~133 BPM in 8ths)
local STEPS_PER_BAR = 8

-- A sustained pad chord progression, each chord held for `bars`. The ADSR
-- voices start once and retune on each chord change, so the pad glides
-- between chords instead of re-attacking.
local PAD_PROG = {
  { names = { "A3", "C4", "E4" }, bars = 2 },
  { names = { "E3", "G3", "D4" }, bars = 2 },
}
-- Derive each chord's frequencies from its note names.
for _, entry in ipairs(PAD_PROG) do
  entry.chord = notes(entry.names)
end
local PAD_VOLUME = 0.1 -- per voice; two voices + bass must not clip the mix

-- Per-step chord-index lookup so the pad rides the same clock as the bass.
-- PAD_BY_STEP[step] = which PAD_PROG entry sounds on that step.
local PAD_BY_STEP = {}
do
  local step = 1
  for chord_index, entry in ipairs(PAD_PROG) do
    for _ = 1, entry.bars * STEPS_PER_BAR do
      PAD_BY_STEP[step] = chord_index
      step = step + 1
    end
  end
end
-- The whole song loops over the longer of bass / pad, in steps.
local SONG_STEPS = math.max(#BASS_STEPS, #PAD_BY_STEP)

-- Per-instrument activity meters: each spikes to 1 when its instrument sounds
-- and decays to 0, drawn as a pulsing bar (see draw_meters). Listed top-down.
local METERS = {
  { key = "pad",  label = "PAD",  color = gfx.COLOR_INDIGO },
  { key = "bass", label = "BASS", color = gfx.COLOR_ORANGE },
  { key = "jump", label = "JUMP", color = gfx.COLOR_GREEN },
  { key = "land", label = "LAND", color = gfx.COLOR_PEACH },
  { key = "coin", label = "COIN", color = gfx.COLOR_YELLOW },
}
local METER_DECAY = 3.5       -- units/sec; how fast a meter falls back to 0
local PAD_SUSTAIN_LEVEL = 0.5 -- the pad is sustained, so its meter rests here

-- Spike an instrument's meter to full (call when it sounds).
local function bump(key)
  State.meters[key] = 1
end

-- Play one bass note. AHD (default shape) self-terminates after attack+hold+
-- decay, so each step ends on its own -- no sfx.stop bookkeeping. `decay`
-- sets how long the note rings.
local function bass_note(freq)
  if freq <= 0 then return end -- rest
  sfx.synth({
    wave = sfx.SQUARE,
    freq = freq,
    volume = 0.25,
    attack = 4,
    hold = 40,
    decay = 120,
  })
end

local function reset_player()
  State.Player = {
    x = usagi.GAME_W / 2 - 8,
    y = usagi.GAME_H - 26,
    w = 12,
    h = 16,
    vx = 0,
    vy = 0,
    sprite_index = 1,
    land_frames = 0,
    on_ground = false,
  }
end

-- A coin sits a few px above a platform's top edge, centered.
local function coin_on(p)
  return { x = p.x + p.w / 2, y = p.y - 10, r = 4, taken = false }
end

function _config()
  return { name = "Jumper -- synth sfx in gameplay" }
end

function _init()
  State = {}

  State.Ground = { x = 0, y = usagi.GAME_H - 10, w = usagi.GAME_W, h = 10 }
  State.Platforms = {
    { x = 24,                    y = usagi.GAME_H - 44, w = 56, h = 6 },
    { x = usagi.GAME_W / 2 - 28, y = usagi.GAME_H - 80, w = 56, h = 6 },
    { x = usagi.GAME_W - 80,     y = usagi.GAME_H - 48, w = 56, h = 6 },
  }

  State.Coins = {}
  for _, p in ipairs(State.Platforms) do
    table.insert(State.Coins, coin_on(p))
  end

  State.Score = 0
  reset_player()

  -- One master song clock drives both bass and pad, so they stay locked.
  State.step = 1
  State.step_timer = 0

  -- Per-instrument activity meters, all idle to start.
  State.meters = {}
  for _, m in ipairs(METERS) do
    State.meters[m.key] = 0
  end
  State.bass_label = "-" -- last bass note played, shown in its meter row

  -- Pad: start one held ADSR voice per chord tone of the first chord. These
  -- ids persist; we retune them on chord changes rather than restarting.
  State.pad_chord = PAD_BY_STEP[1]
  State.pad_voices = {}
  for i, freq in ipairs(PAD_PROG[State.pad_chord].chord) do
    State.pad_voices[i] = sfx.synth({
      wave = sfx.SAW,
      shape = sfx.ADSR,
      freq = freq,
      volume = PAD_VOLUME,
      attack = 400, -- slow swell = pad
      decay = 200,
      sustain = 0.9,
      release = 600,
    })
  end
end

-- True when player's box overlaps a rect.
local function overlaps(px, py, pw, ph, r)
  return px < r.x + r.w and px + pw > r.x and py < r.y + r.h and py + ph > r.y
end

function _update(dt)
  -- Master song clock: every STEP_SECONDS fire this step's bass note and, on a
  -- chord change, glide the pad. Both read the same `step`, so they stay synced.
  State.step_timer = State.step_timer + dt
  if State.step_timer >= STEP_SECONDS then
    State.step_timer = State.step_timer - STEP_SECONDS

    local bass_i = (State.step - 1) % #BASS_STEPS + 1
    bass_note(BASS_STEPS[bass_i])
    if BASS_STEPS[bass_i] > 0 then
      bump("bass")
      State.bass_label = BASS_NAMES[bass_i] -- last note played, for the meter
    end

    local chord = PAD_BY_STEP[(State.step - 1) % #PAD_BY_STEP + 1]
    if chord ~= State.pad_chord then
      State.pad_chord = chord
      for i, freq in ipairs(PAD_PROG[chord].chord) do
        sfx.set_freq(State.pad_voices[i], freq)
      end
      bump("pad") -- flash only when the chord actually changes
    end

    State.step = State.step % SONG_STEPS + 1
  end

  -- Decay every meter toward 0.
  for _, m in ipairs(METERS) do
    State.meters[m.key] = math.max(0, State.meters[m.key] - METER_DECAY * dt)
  end
  -- The pad is a sustained voice, so its meter holds at a steady floor
  -- (it never decays to 0) and only flashes brighter on a chord change.
  State.meters.pad = math.max(State.meters.pad, PAD_SUSTAIN_LEVEL)

  local p = State.Player

  -- Horizontal movement.
  if input.held(input.LEFT) then
    p.vx = -MOVE_SPEED
  elseif input.held(input.RIGHT) then
    p.vx = MOVE_SPEED
  else
    p.vx = 0
  end
  p.x += p.vx * dt
  p.x = math.max(0, math.min(usagi.GAME_W - p.w, p.x))

  -- Jump (only from the ground).
  if input.pressed(input.BTN1) and p.on_ground then
    p.vy = JUMP_VEL
    p.on_ground = false
    sfx_jump()
    bump("jump")
  end

  -- Gravity + vertical integration. Remember where the feet were *before*
  -- moving: reconstructing it by subtracting (feet - vy*dt) drifts by a
  -- float ULP, which can leave a resting player a hair above the platform
  -- and drop them through a frame after landing.
  local prev_feet = p.y + p.h
  p.vy += GRAVITY * dt
  local was_airborne = not p.on_ground
  p.y += p.vy * dt
  p.on_ground = false

  -- Land on the ground.
  local g = State.Ground
  if p.y + p.h >= g.y and p.vy >= 0 then
    p.y = g.y - p.h
    p.vy = 0
    p.on_ground = true
  end

  -- Land on platform tops (only when falling onto them).
  for _, plat in ipairs(State.Platforms) do
    if p.vy >= 0 then
      local feet = p.y + p.h
      if prev_feet <= plat.y and feet >= plat.y
          and p.x + p.w > plat.x and p.x < plat.x + plat.w then
        p.y = plat.y - p.h
        p.vy = 0
        p.on_ground = true
      end
    end
  end

  if p.on_ground and was_airborne then
    sfx_land()
    bump("land")
    p.land_frames = 6 -- show the landing sprite for a few frames
  end

  -- Landing sprite: index 2 briefly after a landing, else index 1.
  if p.land_frames > 0 then
    p.land_frames -= 1
    p.sprite_index = 2
  else
    p.sprite_index = 1
  end

  -- Collect coins.
  for _, c in ipairs(State.Coins) do
    if not c.taken and overlaps(p.x, p.y, p.w, p.h, { x = c.x - c.r, y = c.y - c.r, w = c.r * 2, h = c.r * 2 }) then
      c.taken = true
      State.Score += 1
      sfx_coin()
      bump("coin")
    end
  end
end

-- A little mixer panel, top-right: one row per instrument. Each row is a
-- label, a pulsing dot, and a bar whose length tracks the meter's level.
local function draw_meters()
  local bar_w = 46
  local x = usagi.GAME_W - bar_w - 56
  local y = 6
  for _, m in ipairs(METERS) do
    local level = State.meters[m.key]
    local lit = level > 0.05
    local color = lit and m.color or gfx.COLOR_DARK_GRAY
    -- Label (brightens when active), a dot that grows with the level, and a
    -- track with a fill proportional to the level.
    gfx.text(m.label, x - 32, y - 4, color)
    gfx.circ_fill(x - 4, y + 2, 1 + level * 2, color)
    gfx.rect(x, y, bar_w, 5, gfx.COLOR_DARK_GRAY)
    if lit then
      gfx.rect_fill(x, y, math.floor(bar_w * level), 5, m.color)
    end
    -- The notes this instrument is sounding, laid out across the row: the
    -- pad shows its whole chord, the bass its last note.
    local row_notes
    if m.key == "pad" then
      row_notes = PAD_PROG[State.pad_chord].names
    elseif m.key == "bass" then
      row_notes = { State.bass_label }
    end
    if row_notes then
      for i, n in ipairs(row_notes) do
        gfx.text(n, x + bar_w + 6 + (i - 1) * 16, y - 4, color)
      end
    end
    y += 11
  end
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_DARK_BLUE)

  local g = State.Ground
  gfx.rect_fill(g.x, g.y, g.w, g.h, gfx.COLOR_BROWN)

  for _, plat in ipairs(State.Platforms) do
    gfx.rect_fill(plat.x, plat.y, plat.w, plat.h, gfx.COLOR_LIGHT_GRAY)
  end

  for _, c in ipairs(State.Coins) do
    if not c.taken then
      gfx.circ_fill(c.x, c.y, c.r, gfx.COLOR_YELLOW)
      gfx.circ(c.x, c.y, c.r, gfx.COLOR_ORANGE)
    end
  end

  local p = State.Player
  gfx.spr(p.sprite_index, p.x, p.y)

  gfx.text("score: " .. State.Score, 6, 6, gfx.COLOR_WHITE)
  gfx.text("arrows: move   Z: jump", 6, usagi.GAME_H - 12, gfx.COLOR_WHITE)

  draw_meters()
end
