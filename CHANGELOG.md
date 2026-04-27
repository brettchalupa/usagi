# Changelog

History of Usagi releases and what changed in each release. User-facing notes.
Doesn't contain updates relating to developing the engine itself.

## v0.1.0-dev.3 - UNRELEASED

Features:

- `usagi compile` now produces every platform from any host. Default output is
  `export/` containing zips for linux, macos, windows, web, plus the portable
  `.usagi` bundle.
- `--target` accepts `linux`, `macos`, `windows`, `web`, `bundle`, or `all`.
- Runtime templates auto-fetch by version from GitHub Releases on first use,
  cache to a per-OS directory, and verify against published `sha256` sidecars
  before extracting.
- Host platform compiles offline (fuses against the running binary, no cache
  lookup or network).
- New flags: `--template-path` (local archive), `--template-url` (custom URL,
  useful for forks and mirrors), `--no-cache` (force re-download), `--web-shell`
  (custom HTML shell for the web export).
- Custom web shell auto-pickup: `<project>/shell.html` is used if present.
- New subcommand `usagi templates {list,clear}` to inspect or wipe the cache.
- Set `USAGI_TEMPLATE_BASE` to point at a fork or mirror for offline /
  air-gapped setups.

Breaking:

- `--target exe` and `--target web` were removed. Use `--target <os>`
  (`linux`/`macos`/`windows`) instead of `exe`, and `--target web` keeps its
  name but now produces a zip rather than a directory.

Fixes:

- <kbd>Esc</kbd> only quits the game in dev builds, not release builds

## v0.1.0-dev.2 - Apr 27, 2026

Features:

- `gfx.rect` now draws a rectangle outline; use `gfx.rect_fill` for the filled
  variant
- `gfx.circ(x, y, r, color)` — circle outline
- `gfx.circ_fill(x, y, r, color)` — filled circle
- `gfx.line(x1, y1, x2, y2, color)` — line
- Ctrl + R and Cmd + R hard refresh in `usagi dev`

Fixes:

- Properly exit games with `0` status, don't segfault on close

## v0.1.0-dev.1 - Apr 26, 2026

Initial pre-release of Usagi. Very early days. Includes input, rectangle
drawing, sound effect playback, and rendering tiles from a single `sprites.png.`
