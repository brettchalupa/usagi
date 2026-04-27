# Changelog

History of Usagi releases and what changed in each release. User-facing notes.
Doesn't contain updates relating to developing the engine itself.

## v0.1.0-dev.3 - UNRELEASED

Fixes:

- <kbd>Esc</kbd> only quits the game in dev builds, not release builds

## v0.1.0-dev.2 - Apr 27, 2026

Features:

- `gfx.rect` now draws a rectangle outline; use `gfx.rect_fill` for the
  filled variant
- `gfx.circ(x, y, r, color)` — circle outline
- `gfx.circ_fill(x, y, r, color)` — filled circle
- `gfx.line(x1, y1, x2, y2, color)` — line
- Ctrl + R and Cmd + R hard refresh in `usagi dev`

Fixes:

- Properly exit games with `0` status, don't segfault on close

## v0.1.0-dev.1 - Apr 26, 2026

Initial pre-release of Usagi. Very early days. Includes input, rectangle
drawing, sound effect playback, and rendering tiles from a single `sprites.png.`
