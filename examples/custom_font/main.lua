-- Custom font demo. Drops `font.png` (baked from Silver.ttf via
-- `usagi font bake`) next to main.lua; the engine loads it
-- automatically and uses it for gfx.text / gfx.text_ex. The pause
-- menu, FPS overlay, and error text keep the bundled monogram so
-- engine UI doesn't depend on the user font.
--
-- Silver is a 5x9-ish pixel font with broad European + partial CJK
-- coverage, by Poppy Works, licensed CC-BY-4.0:
-- https://poppyworks.itch.io/silver

function _config()
  return { name = "Custom Font (Silver)" }
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_DARK_BLUE)

  gfx.text("Custom Font Demo", 4, 4, gfx.COLOR_YELLOW)
  gfx.text("Silver by Poppy Works (CC-BY-4.0)", 4, 26, gfx.COLOR_LIGHT_GRAY)

  -- Multi-script lines.
  gfx.text("Hello, world!", 4, 56, gfx.COLOR_WHITE)
  gfx.text("Здравствуй, мир!", 4, 78, gfx.COLOR_WHITE)
  gfx.text("Καλημέρα κόσμε", 4, 100, gfx.COLOR_WHITE)
  gfx.text("こんにちは、世界！", 4, 122, gfx.COLOR_WHITE)

  -- Footer hint (rendered in Silver too, since gfx.text uses the user font).
  gfx.text("press esc to pause", 4, 158, gfx.COLOR_DARK_GRAY)
end
