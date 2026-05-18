-- Localization demo: keeps strings in JSON files under
-- `data/strings/` and swaps the active table at runtime. The pattern
-- scales to any number of languages: add a `data/strings/<code>.json`
-- file and a row to LANGS below, no engine changes needed.
--
-- The bundled monogram font covers Latin-1 Supplement and Latin
-- Extended-A, so Spanish accents (á, é, í, ó, ú, ñ, ¿, ¡) render
-- without any custom-font setup.
--
-- Strings are loaded at the top of the chunk so live reload picks up
-- edits to the JSON without F5. Try translating a value in
-- `data/strings/es.json` while the example is running. The screen
-- updates immediately.

function _config()
  return { name = "Localization" }
end

local LANGS = { "en", "es" }

-- Read every language up front. For a real game with dozens of
-- languages this would be wasteful: load just the active one and
-- re-load on switch. Two-language demo, so eager is fine.
local STRINGS = {}
for _, code in ipairs(LANGS) do
  STRINGS[code] = usagi.read_json("strings/" .. code .. ".json")
end

function _init()
  State = { lang_idx = 1 }
end

function _update(_dt)
  if input.pressed(input.BTN1) then
    State.lang_idx = (State.lang_idx % #LANGS) + 1
  end
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_DARK_BLUE)
  local code = LANGS[State.lang_idx]
  local t = STRINGS[code]

  gfx.text(t.title, 4, 6, gfx.COLOR_WHITE)
  gfx.text(t.subtitle, 4, 18, gfx.COLOR_PEACH)

  gfx.text(t.menu.play, 4, 38, gfx.COLOR_YELLOW)
  gfx.text(t.menu.options, 4, 50, gfx.COLOR_YELLOW)
  gfx.text(t.menu.quit, 4, 62, gfx.COLOR_YELLOW)

  for i, fruit in ipairs(t.fruits) do
    gfx.text("- " .. fruit, 140, 38 + (i - 1) * 12, gfx.COLOR_WHITE)
  end

  gfx.text(t.footer, 4, usagi.GAME_H - 34, gfx.COLOR_LIGHT_GRAY)
  gfx.text(t.instructions, 4, usagi.GAME_H - 22, gfx.COLOR_LIGHT_GRAY)
  gfx.text(t.lang_name .. " (" .. code .. ")", 4, usagi.GAME_H - 12, gfx.COLOR_GREEN)
end
