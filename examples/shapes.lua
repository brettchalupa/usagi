-- Kitchen-sink demo of every gfx shape primitive.

local STAR_X_MIN, STAR_X_MAX = 172, 312
local STAR_Y_MIN, STAR_Y_MAX = 30, 88

function _config()
  return { name = "Shapes" }
end

local stars = {}

function _init()
  for i = 1, 8 do
    stars[i] = {
      x = math.random(STAR_X_MIN, STAR_X_MAX),
      y = math.random(STAR_Y_MIN, STAR_Y_MAX),
      visible = true,
      -- Stagger initial timers so they don't all blink in unison.
      timer = 0.5 + math.random() * 2.5,
    }
  end
end

function _update(dt)
  for _, s in ipairs(stars) do
    s.timer = s.timer - dt
    if s.timer <= 0 then
      if s.visible then
        s.visible = false
        s.timer = 0.15 + math.random() * 0.35
      else
        s.visible = true
        s.x = math.random(STAR_X_MIN, STAR_X_MAX)
        s.y = math.random(STAR_Y_MIN, STAR_Y_MAX)
        s.timer = 1.0 + math.random() * 2.0
      end
    end
  end
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_BLACK)

  gfx.text("usagi shape primitives", 4, 4, gfx.COLOR_WHITE)

  -- gfx.rect (outline)
  gfx.text("gfx.rect", 4, 18, gfx.COLOR_LIGHT_GRAY)
  gfx.rect(4, 28, 24, 16, gfx.COLOR_RED)
  gfx.rect(32, 28, 16, 16, gfx.COLOR_GREEN)
  gfx.rect(52, 28, 32, 8, gfx.COLOR_BLUE)
  gfx.rect(52, 40, 40, 4, gfx.COLOR_YELLOW)
  gfx.rect(96, 28, 4, 16, gfx.COLOR_PINK)

  -- gfx.rect_fill
  gfx.text("gfx.rect_fill", 4, 50, gfx.COLOR_LIGHT_GRAY)
  gfx.rect_fill(4, 60, 24, 16, gfx.COLOR_RED)
  gfx.rect_fill(32, 60, 16, 16, gfx.COLOR_GREEN)
  gfx.rect_fill(52, 60, 32, 8, gfx.COLOR_BLUE)
  gfx.rect_fill(52, 72, 40, 4, gfx.COLOR_YELLOW)
  gfx.rect_fill(96, 60, 4, 16, gfx.COLOR_PINK)

  -- gfx.circ (outline)
  gfx.text("gfx.circ", 4, 82, gfx.COLOR_LIGHT_GRAY)
  gfx.circ(14, 100, 8, gfx.COLOR_ORANGE)
  gfx.circ(34, 100, 4, gfx.COLOR_PEACH)
  gfx.circ(58, 100, 12, gfx.COLOR_INDIGO)
  gfx.circ(82, 100, 2, gfx.COLOR_WHITE)
  gfx.circ(100, 100, 6, gfx.COLOR_DARK_GREEN)

  -- gfx.circ_fill
  gfx.text("gfx.circ_fill", 4, 114, gfx.COLOR_LIGHT_GRAY)
  gfx.circ_fill(14, 132, 8, gfx.COLOR_ORANGE)
  gfx.circ_fill(34, 132, 4, gfx.COLOR_PEACH)
  gfx.circ_fill(58, 132, 12, gfx.COLOR_INDIGO)
  gfx.circ_fill(82, 132, 2, gfx.COLOR_WHITE)
  gfx.circ_fill(100, 132, 6, gfx.COLOR_DARK_GREEN)

  -- gfx.line
  gfx.text("gfx.line", 4, 146, gfx.COLOR_LIGHT_GRAY)
  gfx.line(4, 158, 100, 158, gfx.COLOR_WHITE)       -- horizontal
  gfx.line(4, 164, 4, 176, gfx.COLOR_BROWN)         -- vertical
  gfx.line(12, 176, 60, 160, gfx.COLOR_DARK_PURPLE) -- diagonal up
  gfx.line(64, 160, 112, 176, gfx.COLOR_PINK)       -- diagonal down

  -- Right column: a small composed scene exercising all five primitives
  -- together, so users can see them combine.
  gfx.text("composed", 168, 18, gfx.COLOR_LIGHT_GRAY)
  -- ground + sky band
  gfx.rect_fill(168, 28, 148, 100, gfx.COLOR_DARK_BLUE)
  gfx.rect_fill(168, 110, 148, 18, gfx.COLOR_DARK_GREEN)
  -- sun
  gfx.circ_fill(296, 46, 10, gfx.COLOR_YELLOW)
  gfx.circ(296, 46, 14, gfx.COLOR_ORANGE)
  -- moon
  gfx.circ_fill(186, 46, 6, gfx.COLOR_LIGHT_GRAY)
  -- house body + roof + door + window
  gfx.rect_fill(212, 80, 40, 30, gfx.COLOR_BROWN)
  gfx.line(212, 80, 232, 64, gfx.COLOR_RED)
  gfx.line(232, 64, 252, 80, gfx.COLOR_RED)
  gfx.rect_fill(228, 92, 8, 18, gfx.COLOR_DARK_GRAY)
  gfx.rect(216, 86, 8, 8, gfx.COLOR_PEACH)
  gfx.rect(240, 86, 8, 8, gfx.COLOR_PEACH)
  -- stars: each blinks out after a moment and respawns at a new spot.
  for _, s in ipairs(stars) do
    if s.visible then
      gfx.circ_fill(s.x, s.y, 1, gfx.COLOR_WHITE)
    end
  end

  -- circ_ex: three concentric thickness-1 rings at adjacent radii.
  -- Layering plain `gfx.circ` here would leave rasterizer-rounding
  -- gaps; `circ_ex` draws real rings so the bands sit flush.
  gfx.text("circ_ex rings", 168, 146, gfx.COLOR_LIGHT_GRAY)
  gfx.circ_ex(196, 168, 10, 1, gfx.COLOR_WHITE)
  gfx.circ_ex(196, 168, 8, 1, gfx.COLOR_RED)
  gfx.circ_ex(196, 168, 6, 1, gfx.COLOR_WHITE)

  -- One thick example of each _ex variant so the thickness param is
  -- visibly doing something.
  gfx.text("thick", 260, 146, gfx.COLOR_LIGHT_GRAY)
  gfx.rect_ex(240, 160, 18, 14, 3, gfx.COLOR_PINK)
  gfx.circ_ex(280, 168, 7, 3, gfx.COLOR_ORANGE)
  gfx.line_ex(298, 168, 314, 172, 3, gfx.COLOR_GREEN)
end
