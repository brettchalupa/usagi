-- Demonstrates `gfx.shader_set` + `gfx.shader_uniform` for full-screen
-- post-processing.
--
-- - BTN1: cycle to the next shader (off, CRT, gameboy)
-- - BTN2: cycle CRT scanline intensity (only matters for CRT)

local SHADERS = { nil, "crt", "gameboy", }
local LABELS = { "off", "crt", "gameboy" }
local SCANLINE_LEVELS = { 0.0, 0.25, 0.5, 0.75 }

function _config()
  return { title = "Shader demo", pixel_perfect = false }
end

function _init()
  state = {
    shader_idx = 2,
    scan_idx = 3,
    t = 0,
    bunny_x = 40,
    bunny_dir = 1,
  }
  gfx.shader_set(SHADERS[state.shader_idx])
end

function _update(dt)
  state.t = state.t + dt

  if input.pressed(input.BTN1) then
    state.shader_idx = (state.shader_idx % #SHADERS) + 1
    gfx.shader_set(SHADERS[state.shader_idx])
  end
  if input.pressed(input.BTN2) then
    state.scan_idx = (state.scan_idx % #SCANLINE_LEVELS) + 1
  end

  state.bunny_x = state.bunny_x + state.bunny_dir * 40 * dt
  if state.bunny_x > usagi.GAME_W - 30 then
    state.bunny_dir = -1
  elseif state.bunny_x < 10 then
    state.bunny_dir = 1
  end
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_DARK_BLUE)

  -- Uniforms only matter for the CRT shader; gameboy ignores them.
  -- Setting them every frame is fine: shader_uniform on a uniform the
  -- active shader doesn't declare is a silent no-op.
  gfx.shader_uniform("u_time", state.t)
  gfx.shader_uniform("u_scanline", SCANLINE_LEVELS[state.scan_idx])
  gfx.shader_uniform("u_resolution", { usagi.GAME_W, usagi.GAME_H })

  for y = 0, usagi.GAME_H, 20 do
    gfx.line(0, y, usagi.GAME_W, y, gfx.COLOR_DARK_PURPLE)
  end
  for x = 0, usagi.GAME_W, 20 do
    gfx.line(x, 0, x, usagi.GAME_H, gfx.COLOR_DARK_PURPLE)
  end

  -- Color swatches so palette-style shaders have something to chew on.
  local sw_y = 40
  local colors = {
    gfx.COLOR_RED, gfx.COLOR_ORANGE, gfx.COLOR_YELLOW, gfx.COLOR_GREEN,
    gfx.COLOR_BLUE, gfx.COLOR_INDIGO, gfx.COLOR_PINK, gfx.COLOR_PEACH,
  }
  for i, c in ipairs(colors) do
    gfx.rect_fill(40 + (i - 1) * 30, sw_y, 24, 16, c)
  end

  gfx.rect_fill(state.bunny_x, 100, 20, 14, gfx.COLOR_PEACH)
  gfx.rect_fill(state.bunny_x + 4, 96, 4, 6, gfx.COLOR_PEACH)
  gfx.rect_fill(state.bunny_x + 12, 96, 4, 6, gfx.COLOR_PEACH)
  gfx.circ_fill(state.bunny_x + 7, 106, 1, gfx.COLOR_BLACK)
  gfx.circ_fill(state.bunny_x + 13, 106, 1, gfx.COLOR_BLACK)

  gfx.text("SHADER DEMO", 4, 4, gfx.COLOR_YELLOW)
  gfx.text("BTN1: cycle  BTN2: scanline", 4, 14, gfx.COLOR_LIGHT_GRAY)
  gfx.text("shader: " .. LABELS[state.shader_idx], 4, usagi.GAME_H - 20, gfx.COLOR_GREEN)
  gfx.text("scanline: " .. SCANLINE_LEVELS[state.scan_idx], 4, usagi.GAME_H - 10, gfx.COLOR_GREEN)
end
