-- gfx.text_ex demo: scale (big title) and rotation (wiggling
-- subtitle, static-tilted label). The plain gfx.text lines at the
-- bottom show extended-character coverage (accented Latin, Cyrillic,
-- Greek, punctuation) at native size.

function _config()
  return { name = "Text" }
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_DARK_BLUE)

  -- Big title at scale 4 (integer scale = crisp pixel art).
  gfx.text_ex("USAGI", 80, 8, 4, 0, gfx.COLOR_YELLOW)

  -- Wiggling subtitle: small sinusoidal rotation around the text's
  -- center. Radians here; ~0.1 rad ≈ 6 degrees of sway.
  local wiggle = math.sin(usagi.elapsed * 4) * 0.1
  gfx.text_ex("press z to start", 96, 60, 2, wiggle, gfx.COLOR_WHITE)

  -- Static tilted label. math.rad turns literal degrees into radians.
  gfx.text_ex("v0.8", 24, 92, 2, math.rad(-45), gfx.COLOR_PINK)

  -- Extended character coverage at native size.
  gfx.text("café naïve jalapeño façade", 4, 120, gfx.COLOR_LIGHT_GRAY)
  gfx.text("Здравствуй, мир!", 4, 132, gfx.COLOR_LIGHT_GRAY)
  gfx.text("Καλημέρα κόσμε", 4, 144, gfx.COLOR_LIGHT_GRAY)
  gfx.text("¡Hola, señor! «extra»", 4, 156, gfx.COLOR_LIGHT_GRAY)

  -- Plain gfx.text at native size for size reference.
  gfx.text("plain gfx.text for scale reference", 4, 168, gfx.COLOR_DARK_GRAY)
end
