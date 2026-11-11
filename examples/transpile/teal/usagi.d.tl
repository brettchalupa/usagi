-- usagi.d.tl
--
-- Barebones Teal type declarations example for Usagi's `gfx` functions and constants
-- Written against the v1.3.x documentation
--
-- a .d.tl file does not generate Lua code, so the engine itself provides `gfx`,
-- `input`, `sfx`, `music`, `usagi`, `effect`, and `util`. This file only tells
-- the Teal compiler what they look like

-------------------------------------------------------------------------------
-- gfx
-------------------------------------------------------------------------------

global record gfx
  -- Fill the whole screen with a color
  clear: function(color: integer)

  -- Draw text
  -- Non-string values are converted by the engine
  text: function(text: string | number, x: number, y: number, color: integer, alpha?: number)

  -- Palette
  COLOR_BLACK: integer
  COLOR_WHITE: integer
  COLOR_PEACH: integer
end
