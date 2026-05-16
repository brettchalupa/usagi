-- Visual QWERTY keyboard. Each key rect lights up while held. Demos
-- the direct-keyboard escape hatch: input.key_held(input.KEY_*) reads
-- raw keys (bypassing the abstract action / keymap system), which is
-- what makes a literal keyboard test like this possible.

function _config()
  return {
    name = "Keyboard",

    -- This option is needed, in order for the game to be able
    -- to capture the [Esc], [P] and [Enter] keypresses
    pause_menu = false,
  }
end

local KH = 14
local GAP = 1
local KEYS = {}

local function row(y, x0, defs)
  local x = x0
  for _, d in ipairs(defs) do
    local w = d[3] or 14
    KEYS[#KEYS + 1] = { label = d[1], key = d[2], x = x, y = y, w = w, h = KH }
    x = x + w + GAP
  end
end

-- F-row (Esc + F1-F12). F-keys are 18px wide so the 3-char labels
-- (F10-F12) don't spill out of their boxes.
row(18, 8, {
  { "ESC", input.KEY_ESCAPE, 22 },
  { "F1",  input.KEY_F1,     20 }, { "F2", input.KEY_F2, 20 }, { "F3", input.KEY_F3, 20 },
  { "F4", input.KEY_F4, 20 }, { "F5", input.KEY_F5, 20 }, { "F6", input.KEY_F6, 20 },
  { "F7", input.KEY_F7, 20 }, { "F8", input.KEY_F8, 20 }, { "F9", input.KEY_F9, 20 },
  { "F10", input.KEY_F10, 20 }, { "F11", input.KEY_F11, 20 }, { "F12", input.KEY_F12, 20 },
})

-- Number row
row(36, 8, {
  { "~", input.KEY_BACKTICK },
  { "1", input.KEY_1 }, { "2", input.KEY_2 }, { "3", input.KEY_3 }, { "4", input.KEY_4 },
  { "5", input.KEY_5 }, { "6", input.KEY_6 }, { "7", input.KEY_7 }, { "8", input.KEY_8 },
  { "9", input.KEY_9 }, { "0", input.KEY_0 },
  { "-",  input.KEY_MINUS }, { "=", input.KEY_EQUAL },
  { "<-", input.KEY_BACKSPACE, 22 },
})

-- Top row (Q-P + brackets)
row(51, 8, {
  { "TAB", input.KEY_TAB, 22 },
  { "Q",   input.KEY_Q }, { "W", input.KEY_W }, { "E", input.KEY_E }, { "R", input.KEY_R },
  { "T", input.KEY_T }, { "Y", input.KEY_Y }, { "U", input.KEY_U }, { "I", input.KEY_I },
  { "O", input.KEY_O }, { "P", input.KEY_P },
  { "[", input.KEY_LBRACKET }, { "]", input.KEY_RBRACKET }, { "\\", input.KEY_BACKSLASH },
})

-- Home row (A-L + ; ' + Enter). Offset 8px since Caps Lock isn't exposed.
row(66, 16, {
  { "A", input.KEY_A }, { "S", input.KEY_S }, { "D", input.KEY_D }, { "F", input.KEY_F },
  { "G", input.KEY_G }, { "H", input.KEY_H }, { "J", input.KEY_J }, { "K", input.KEY_K },
  { "L", input.KEY_L },
  { ";", input.KEY_SEMICOLON }, { "'", input.KEY_APOSTROPHE },
  { "ENT", input.KEY_ENTER, 32 },
})

-- Bottom row (Z-M + , . / + shifts)
row(81, 8, {
  { "LSH", input.KEY_LSHIFT, 22 },
  { "Z",   input.KEY_Z }, { "X", input.KEY_X }, { "C", input.KEY_C }, { "V", input.KEY_V },
  { "B", input.KEY_B }, { "N", input.KEY_N }, { "M", input.KEY_M },
  { ",", input.KEY_COMMA }, { ".", input.KEY_PERIOD }, { "/", input.KEY_SLASH },
  { "RSH", input.KEY_RSHIFT, 22 },
})

-- Space row (modifiers + space)
row(96, 8, {
  { "LC",    input.KEY_LCTRL, 18 },
  { "LA",    input.KEY_LALT,  18 },
  { "SPACE", input.KEY_SPACE, 130 },
  { "RA",    input.KEY_RALT,  18 },
  { "RC",    input.KEY_RCTRL, 18 },
})

-- Arrow cluster on the right: Up above the Left/Down/Right trio.
row(81, 232, { { "^", input.KEY_UP } })
row(96, 216, {
  { "<", input.KEY_LEFT },
  { "v", input.KEY_DOWN },
  { ">", input.KEY_RIGHT },
})

local function draw_key(k)
  local held = input.key_held(k.key)
  local label_color
  if held then
    gfx.rect_fill(k.x, k.y, k.w, k.h, gfx.COLOR_YELLOW)
    label_color = gfx.COLOR_BLACK
  else
    gfx.rect(k.x, k.y, k.w, k.h, gfx.COLOR_LIGHT_GRAY)
    label_color = gfx.COLOR_WHITE
  end
  local mw, mh = usagi.measure_text(k.label)
  gfx.text(k.label, k.x + (k.w - mw) // 2, k.y + (k.h - mh) // 2, label_color)
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_DARK_PURPLE)
  gfx.text("KEYBOARD - PRESS ANY KEY", 8, 2, gfx.COLOR_PEACH)
  for _, k in ipairs(KEYS) do
    draw_key(k)
  end
  gfx.text("Uses input.key_held(input.KEY_*).", 8, 122, gfx.COLOR_LIGHT_GRAY)
end
