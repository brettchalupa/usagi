function _config()
  return { name = "Mouse" }
end

function _init()
  -- Hide the OS cursor so we can draw a custom one. Toggle back with
  -- BTN3 (C / L) to compare.
  input.set_mouse_visible(false)

  State = {
    sparks = {},
    -- Index into SPARK_COLORS. Scroll-wheel cycles this.
    selected = 1,
  }
end

local SPARK_COLORS = {
  gfx.COLOR_YELLOW,
  gfx.COLOR_ORANGE,
  gfx.COLOR_PINK,
  gfx.COLOR_RED,
}

local function emit_spark(x, y, speed, life)
  local angle = math.random() * math.pi * 2
  State.sparks[#State.sparks + 1] = {
    x = x,
    y = y,
    vx = math.cos(angle) * speed,
    vy = math.sin(angle) * speed,
    life = life,
    color = SPARK_COLORS[State.selected],
  }
end

function _update(dt)
  local mx, my = input.mouse()
  -- Mouse coords can land outside the game viewport when the cursor is
  -- over a letterbox bar. Skip emitting in that case so sparks don't
  -- spawn off-screen.
  local in_bounds = mx >= 0 and mx < usagi.GAME_W and my >= 0 and my < usagi.GAME_H

  if in_bounds then
    -- Steady trickle while moving the cursor inside the play area.
    emit_spark(mx, my, 30 + math.random() * 30, 0.4 + math.random() * 0.4)
  end

  if in_bounds and input.mouse_pressed(input.MOUSE_LEFT) then
    -- Left click: burst.
    for _ = 1, 32 do
      emit_spark(mx, my, 60 + math.random() * 80, 0.6 + math.random() * 0.6)
    end
  end

  if input.mouse_pressed(input.MOUSE_RIGHT) then
    -- Right click: wipe.
    State.sparks = {}
  end

  if input.pressed(input.BTN3) then
    input.set_mouse_visible(not input.mouse_visible())
  end

  -- Scroll up / down to cycle spark color. Match on > 0 / < 0 rather
  -- than equality with 1 / -1 since trackpads emit fractional per-frame
  -- values.
  local scroll = input.mouse_scroll()
  if scroll > 0 then
    State.selected = State.selected - 1
    if State.selected < 1 then State.selected = #SPARK_COLORS end
  elseif scroll < 0 then
    State.selected = State.selected + 1
    if State.selected > #SPARK_COLORS then State.selected = 1 end
  end

  for i = #State.sparks, 1, -1 do
    local s = State.sparks[i]
    s.x = s.x + s.vx * dt
    s.y = s.y + s.vy * dt
    -- Light gravity for a fountain-y feel.
    s.vy = s.vy + 60 * dt
    s.life = s.life - dt
    if s.life <= 0 then
      table.remove(State.sparks, i)
    end
  end
end

local function draw_cursor(x, y)
  -- Tiny crosshair: a center pixel with four arms, palette colors
  -- chosen to read against the dark background.
  gfx.pixel(x, y, gfx.COLOR_WHITE)
  gfx.pixel(x - 2, y, gfx.COLOR_LIGHT_GRAY)
  gfx.pixel(x + 2, y, gfx.COLOR_LIGHT_GRAY)
  gfx.pixel(x, y - 2, gfx.COLOR_LIGHT_GRAY)
  gfx.pixel(x, y + 2, gfx.COLOR_LIGHT_GRAY)
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_DARK_BLUE)

  for _, s in ipairs(State.sparks) do
    gfx.pixel(s.x, s.y, s.color)
  end

  local mx, my = input.mouse()
  -- Only draw the custom cursor when the OS cursor is hidden, so the
  -- two don't overlap and double-render.
  if not input.mouse_visible() then
    draw_cursor(mx, my)
  end

  gfx.text("Mouse: " .. mx .. ", " .. my, 4, 4, gfx.COLOR_WHITE)
  gfx.text("Left click: burst   Right click: clear", 4, 14, gfx.COLOR_LIGHT_GRAY)
  local b3 = input.mapping_for(input.BTN3) or "BTN3"
  gfx.text(b3 .. " toggles OS cursor", 4, 24, gfx.COLOR_LIGHT_GRAY)
  gfx.text("Scroll: spark color", 4, 34, gfx.COLOR_LIGHT_GRAY)

  -- Swatch row: one filled square per color, white outline around the
  -- currently-selected one so the wheel feels connected to something.
  local sw = 8
  local gap = 2
  local sx = usagi.GAME_W - (#SPARK_COLORS * (sw + gap)) - 4
  local sy = 4
  for i, c in ipairs(SPARK_COLORS) do
    local x = sx + (i - 1) * (sw + gap)
    gfx.rect_fill(x, sy, sw, sw, c)
    if i == State.selected then
      gfx.rect(x - 1, sy - 1, sw + 2, sw + 2, gfx.COLOR_WHITE)
    end
  end
end
