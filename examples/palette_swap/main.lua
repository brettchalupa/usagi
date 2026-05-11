-- Palette swap demo using Sweetie16.
--
-- Drop a `palette.png` (any rectangular shape, one color per pixel)
-- at the project root and Usagi swaps the engine's default Pico-8
-- palette for yours. lospec.com's "1px cells" export is the canonical
-- format. Download a palette and rename it to `palette.png`.
--
-- This example uses sweetie16 (https://lospec.com/palette-list/sweetie-16).
-- Tip: load palette.png into Aseprite's palette panel to paint sprites that
-- match your engine palette.
--
-- IMPORTANT: the built-in `gfx.COLOR_*` constants are slot indices in
-- Pico-8's ordering. They keep working with a custom palette, but the
-- *names* won't match the *colors* anymore. `gfx.COLOR_RED` is slot
-- 9, which in sweetie16 is a dark navy, not red. The recommended
-- pattern is to define your own constants for your palette, like the
-- `COLOR` table below. Use `COLOR.RED` everywhere instead of
-- `gfx.COLOR_RED`, and your code stays readable when you swap palettes
-- or load this one in different projects.

local COLOR = {
  NIGHT = 1,
  PURPLE = 2,
  RED = 3,
  ORANGE = 4,
  YELLOW = 5,
  LIME = 6,
  GREEN = 7,
  TEAL = 8,
  NAVY = 9,
  BLUE = 10,
  SKY = 11,
  CYAN = 12,
  WHITE = 13,
  SILVER = 14,
  GRAY = 15,
  SHADOW = 16,
}

function _config()
  return { name = "Palette swap - sweetie16" }
end

function _draw(_dt)
  gfx.clear(COLOR.NIGHT)

  gfx.text_ex("PALETTE SWAP", 80, 12, 2, 0, COLOR.WHITE)
  gfx.text("drop a palette.png at project root to swap colors", 8, 36, COLOR.SILVER)

  -- Every slot in the active palette, numbered. Each swatch labelled
  -- with its 1-based index so you can read the palette off at a glance.
  local sw = 16
  local gap = 2
  local total_w = 16 * (sw + gap) - gap
  local x0 = math.floor((usagi.GAME_W - total_w) / 2)
  local y0 = 60
  for i = 1, 16 do
    local x = x0 + (i - 1) * (sw + gap)
    gfx.rect_fill(x, y0, sw, sw, i)
    gfx.text(tostring(i), x + 5, y0 + sw + 2, COLOR.SILVER)
  end

  -- Small scene using the named constants from the COLOR table above.
  -- This is the pattern: name your slots, then use those names.
  gfx.text_ex("named for sweetie16:", 8, 108, 1, 0, COLOR.WHITE)
  gfx.rect_fill(8, 122, 30, 12, COLOR.RED)
  gfx.text("RED", 42, 122, COLOR.WHITE)
  gfx.rect_fill(8, 138, 30, 12, COLOR.GREEN)
  gfx.text("GREEN", 42, 138, COLOR.WHITE)
  gfx.rect_fill(8, 154, 30, 12, COLOR.BLUE)
  gfx.text("BLUE", 42, 154, COLOR.WHITE)

  gfx.circ_fill(usagi.GAME_W - 40, 140, 18, COLOR.YELLOW)
  gfx.circ_ex(usagi.GAME_W - 40, 140, 22, 2, COLOR.ORANGE)
end
