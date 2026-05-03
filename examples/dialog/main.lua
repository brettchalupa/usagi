-- Typewriter dialog box demo

local CPS = 40        -- typewriter speed in chars/sec
local CLICK_EVERY = 3 -- play sfx once per N revealed chars
local PAD = 6         -- inner padding for the frames
local LINE_H = 12     -- monogram line height; see usagi.measure_text
local SPEAKER_H = 18
local MSG_H = 56
local MSG_MARGIN = 6

-- Speakers carry their own color so each line reads at a glance.
local SPEAKER = {
  SNAKE   = { name = "Snake", color = gfx.COLOR_LIGHT_GRAY },
  OTACON  = { name = "Otacon", color = gfx.COLOR_PEACH },
  COLONEL = { name = "Colonel", color = gfx.COLOR_YELLOW },
  MEILING = { name = "Mei Ling", color = gfx.COLOR_PINK },
  KAZ     = { name = "KAZ", color = gfx.COLOR_ORANGE },
}

-- The script. Each entry: { speaker, text }. \n wraps a line.
local SCRIPT = {
  { SPEAKER.COLONEL, "Snake, this is Colonel Campbell.\nDo you read me?" },
  { SPEAKER.SNAKE,   "Loud and clear, Colonel." },
  { SPEAKER.OTACON,  "Snake, you have to find the\ncardboard box." },
  { SPEAKER.SNAKE,   "..." },
  { SPEAKER.SNAKE,   "Kept you waiting, huh?" },
  { SPEAKER.MEILING, "A man's life is not measured\nin years..." },
  { SPEAKER.MEILING, "...but in the deeds of his heart." },
  { SPEAKER.OTACON,  "Snake! Watch out for the\nCyborg Ninja!" },
  { SPEAKER.SNAKE,   "Metal Gear?!" },
  { SPEAKER.KAZ,     "They played us like a damn fiddle!" },
}

function _config()
  return { title = "Typewriter Dialog" }
end

function _init()
  State = {
    idx = 1,        -- current entry in SCRIPT
    revealed = 0,   -- how many chars of the current line are visible
    elapsed = 0,    -- seconds spent typing the current line
    last_click = 0, -- # of chars at last click sfx (for rate-limiting)
    blink_t = 0,    -- accumulator for the prompt indicator
  }
end

local function current()
  return SCRIPT[State.idx]
end

local function full_text()
  local entry = current()
  return entry and entry[2] or ""
end

local function is_complete()
  return State.revealed >= #full_text()
end

local function complete_line()
  State.revealed = #full_text()
end

local function advance()
  State.idx = State.idx + 1
  State.revealed = 0
  State.elapsed = 0
  State.last_click = 0
  -- Loop the script for an endless demo.
  if State.idx > #SCRIPT then
    State.idx = 1
  end
end

function _update(dt)
  local entry = current()
  if entry == nil then return end

  State.blink_t = State.blink_t + dt

  if input.pressed(input.BTN1) then
    if is_complete() then
      advance()
    else
      complete_line()
    end
    return
  end

  if not is_complete() then
    State.elapsed = State.elapsed + dt
    local target = math.min(math.floor(State.elapsed * CPS), #full_text())
    if target > State.revealed then
      State.revealed = target
      -- Subtle typewriter audio: rate-limited by CLICK_EVERY chars,
      -- and skipped on whitespace so spaces don't tick.
      if State.revealed - State.last_click >= CLICK_EVERY then
        local last_char = full_text():sub(State.revealed, State.revealed)
        if last_char:match("%S") then
          sfx.play("click")
        end
        State.last_click = State.revealed
      end
    end
  end
end

-- Two-color frame: filled black plus a 1-pixel inset outline. Mirrors
-- 1_bit_fantasy's DrawFrame, scaled for the smaller usagi canvas.
local function draw_frame(x, y, w, h)
  gfx.rect_fill(x, y, w, h, gfx.COLOR_BLACK)
  gfx.rect(x + 1, y + 1, w - 2, h - 2, gfx.COLOR_WHITE)
end

local function visible_text()
  return full_text():sub(1, State.revealed)
end

-- Wrap-aware draw: text holds explicit \n line breaks, so split and
-- draw each line on its own row.
local function draw_lines(text, x, y, color)
  local cursor_y = y
  for line in (text .. "\n"):gmatch("([^\n]*)\n") do
    gfx.text(line, x, cursor_y, color)
    cursor_y = cursor_y + LINE_H
  end
end

local function draw_message_frame()
  local box_x = MSG_MARGIN
  local box_y = usagi.GAME_H - MSG_H - MSG_MARGIN
  local box_w = usagi.GAME_W - 2 * MSG_MARGIN
  draw_frame(box_x, box_y, box_w, MSG_H)
  draw_lines(
    visible_text(),
    box_x + PAD,
    box_y + PAD,
    gfx.COLOR_WHITE
  )

  -- Active indicator
  if is_complete() and (State.blink_t * 2) % 2 < 1 then
    gfx.circ_fill(
      box_x + box_w - PAD - 6,
      box_y + MSG_H - PAD - LINE_H / 2,
      3,
      gfx.COLOR_WHITE
    )
  end
end

local function draw_speaker_frame()
  local entry = current()
  if entry == nil then return end
  local sp = entry[1]
  local label = sp.name
  -- Size the speaker box to the label width plus a comfortable margin.
  local label_w = usagi.measure_text(label)
  local box_w = label_w + PAD * 2
  local box_x = MSG_MARGIN + PAD
  local box_y = usagi.GAME_H - MSG_H - MSG_MARGIN - SPEAKER_H + 2
  draw_frame(box_x, box_y, box_w, SPEAKER_H)
  gfx.text(label, box_x + PAD, box_y + 4, sp.color)
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_DARK_BLUE)

  -- Stage background
  for i = 1, 30 do
    local x = (i * 23) % usagi.GAME_W
    local y = (i * 17) % (usagi.GAME_H - MSG_H - 24)
    gfx.pixel(x, y, gfx.COLOR_DARK_GRAY)
  end

  -- Hint
  gfx.text(
    "BTN1: skip / advance",
    200,
    usagi.GAME_H - MSG_H - SPEAKER_H,
    gfx.COLOR_LIGHT_GRAY
  )

  draw_message_frame()
  draw_speaker_frame()
end
