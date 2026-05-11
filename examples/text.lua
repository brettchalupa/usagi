-- gfx.text_ex demo: scale (big title) and rotation (wiggling
-- subtitle, static-tilted label). The plain gfx.text in the footer is
-- there for size comparison.

function _config()
  return { name = "Text" }
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_DARK_BLUE)

  -- Big title at scale 4 (integer scale = crisp pixel art).
  gfx.text_ex("USAGI", 80, 20, 4, 0, gfx.COLOR_YELLOW)

  -- Wiggling subtitle: small sinusoidal rotation around the text's
  -- center. Radians here; ~0.1 rad ≈ 6 degrees of sway.
  local wiggle = math.sin(usagi.elapsed * 4) * 0.1
  gfx.text_ex("press z to start", 96, 80, 2, wiggle, gfx.COLOR_WHITE)

  -- Static tilted label. math.rad turns literal degrees into radians.
  gfx.text_ex("v0.8", 24, 120, 2, math.rad(-45), gfx.COLOR_PINK)

  -- Plain gfx.text at native size for comparison.
  gfx.text("plain gfx.text for scale reference", 4, 168, gfx.COLOR_LIGHT_GRAY)
end
