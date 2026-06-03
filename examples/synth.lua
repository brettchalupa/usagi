-- Programmatic sound: a tiny 3-voice instrument, synthesized and mixed on
-- the audio thread (no .wav files). Three rows, each an editable voice:
--
--   arrows : move the cursor (up/down rows, left/right columns)
--   Q / E  : change the selected cell's value (down / up)
--   1/2/3  : play row 1/2/3 -- hold to sustain (ADSR), tap for one-shots
--            (AHD/DRUM). Press several at once for a chord.
--
-- Editing the NOTE of a sounding row glides its pitch live (synth.set_freq);
-- waveform/envelope bake at note-on, so they're heard on the next press.
-- synth.sfx returns a voice id; synth.stop releases a held one.

local NOTE_NAMES = { "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B" }
local WAVES = { synth.SINE, synth.SAW, synth.SQUARE, synth.NOISE, synth.TRIANGLE }
local WAVE_NAMES = { "SINE", "SAW", "SQUARE", "NOISE", "TRIANGLE" }
local SHAPES = { synth.AHD, synth.ADSR, synth.DRUM }
local SHAPE_NAMES = { "AHD", "ADSR", "DRUM" }

local MIDI_MIN, MIDI_MAX = 36, 96 -- C2..C7
local COLS = 3                    -- 1 = note, 2 = waveform, 3 = envelope

-- Equal-temperament frequency for a MIDI note (69 = A4 = 440 Hz).
local function midi_to_freq(midi)
  return 440 * 2 ^ ((midi - 69) / 12)
end

-- Display name like "A4" for a MIDI note.
local function note_name(midi)
  return NOTE_NAMES[(midi % 12) + 1] .. tostring(math.floor(midi / 12) - 1)
end

function _init()
  State = {
    cursor_row = 1,
    cursor_col = 1,
    rows = {
      { midi = 60, wave = 5, shape = 2, action = input.BTN1 }, -- C4, TRIANGLE, ADSR
      { midi = 64, wave = 5, shape = 2, action = input.BTN2 }, -- E4, TRIANGLE, ADSR
      { midi = 67, wave = 5, shape = 2, action = input.BTN3 }, -- G4, TRIANGLE, ADSR
    },
  }
end

-- Add `delta` (+1 / -1) to the cursor's current cell, clamping/wrapping.
local function alter(delta)
  local row = State.rows[State.cursor_row]
  if State.cursor_col == 1 then
    row.midi = math.max(MIDI_MIN, math.min(MIDI_MAX, row.midi + delta))
    -- If this row is sounding, glide its pitch live (click-free).
    if row.id then synth.set_freq(row.id, midi_to_freq(row.midi)) end
  elseif State.cursor_col == 2 then
    row.wave = (row.wave - 1 + delta) % #WAVES + 1
  else
    row.shape = (row.shape - 1 + delta) % #SHAPES + 1
  end
end

function _update()
  -- Move the cursor (clamped to the grid).
  if input.pressed(input.UP) then State.cursor_row = math.max(1, State.cursor_row - 1) end
  if input.pressed(input.DOWN) then State.cursor_row = math.min(#State.rows, State.cursor_row + 1) end
  if input.pressed(input.LEFT) then State.cursor_col = math.max(1, State.cursor_col - 1) end
  if input.pressed(input.RIGHT) then State.cursor_col = math.min(COLS, State.cursor_col + 1) end

  -- Change the selected cell's value. E = up, Q = down.
  if input.key_pressed(input.KEY_E) then alter(1) end
  if input.key_pressed(input.KEY_Q) then alter(-1) end

  -- Play: each row's button starts a voice on press, releases it on
  -- key-up. ADSR sustains while held; AHD/DRUM self-terminate.
  for _, row in ipairs(State.rows) do
    if input.pressed(row.action) then
      row.id = synth.sfx({
        -- 0.3 keeps headroom so a 3-note chord won't clip the mix.
        wave = WAVES[row.wave],
        freq = midi_to_freq(row.midi),
        volume = 0.3,
        shape = SHAPES[row.shape],
        attack = 8,
        decay = 140,
        sustain = 0.8,
        release = 120,
      })
    elseif input.released(row.action) and row.id then
      synth.stop(row.id)
      row.id = nil
    end
  end
end

function _draw()
  gfx.clear(gfx.COLOR_DARK_BLUE)
  gfx.text("3-voice synth -- 1/2/3 play (chord!)", 8, 6, gfx.COLOR_WHITE)

  -- Column header.
  gfx.text("NOTE", 56, 22, gfx.COLOR_LIGHT_GRAY)
  gfx.text("WAVE", 104, 22, gfx.COLOR_LIGHT_GRAY)
  gfx.text("ENV", 184, 22, gfx.COLOR_LIGHT_GRAY)

  local xs = { 56, 104, 184 }
  local y = 38
  for ri, row in ipairs(State.rows) do
    local playing = input.held(row.action)
    gfx.text((ri) .. ":", 8, y, playing and gfx.COLOR_GREEN or gfx.COLOR_LIGHT_GRAY)
    local cells = { note_name(row.midi), WAVE_NAMES[row.wave], SHAPE_NAMES[row.shape] }
    for ci = 1, COLS do
      local selected = (ri == State.cursor_row and ci == State.cursor_col)
      local col = selected and gfx.COLOR_YELLOW or (playing and gfx.COLOR_GREEN or gfx.COLOR_WHITE)
      local label = selected and ("[" .. cells[ci] .. "]") or cells[ci]
      gfx.text(label, xs[ci] - (selected and 6 or 0), y, col)
    end
    y = y + 14
  end

  gfx.text("arrows: move   Q/E: change", 8, y + 6, gfx.COLOR_LIGHT_GRAY)
  gfx.text("note glides live; wave/env next press", 8, y + 20, gfx.COLOR_DARK_GRAY)
end
