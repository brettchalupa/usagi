# Changelog

History of Usagi releases and what changed in each release. User-facing notes.
Doesn't contain updates relating to developing the engine itself.

## UNRELEASED

Features:

- Engine-level pause menu. **Esc**, **P**, or gamepad **Start** opens it; the
  same buttons (plus **BTN2**) close it. While open, `_update` and `_draw` are
  skipped and the screen shows a black "PAUSED" overlay (music keeps streaming).
  Foundation for a menu with volume, input remap, and game-registered hooks.
  **Shift+Esc** in dev now quits the game, replacing raylib's default
  Esc-quits-immediately default.
- Multiple Lua source files are now supported; use `require("file")` to load
  `file.lua`.
- Compound assignment operators: `+=`, `-=`, `*=`, `/=`, `%=` are rewritten to
  plain Lua before parsing, with `runtime.nonstandardSymbol` set in the shipped
  `.luarc.json` so the language server accepts them.
- Three action buttons: `input.BTN1`, `input.BTN2`, `input.BTN3` replace the
  previous `CONFIRM` / `CANCEL` pair. Keyboard: Z/J, X/K, C/L. Gamepad: south,
  east, and (north or west) face buttons. BTN3 fires for both Xbox Y and X (PS
  Triangle and Square) so it's reachable from either side of the diamond.
- New `examples/rng.lua` demonstrates `math.random` (PRNG is auto-seeded on
  startup) and how to call `math.randomseed(n)` for deterministic sequences.
- Input now polls every connected gamepad slot rather than only slot 0. Any
  connected pad (Steam Deck built-in, external pad over USB/Bluetooth) triggers
  actions, and hot-swapping no longer drops input when a pad lands on a
  different slot.
- New `gfx.pixel(x, y, color)` for single-pixel drawing.
- Save data: `usagi.save(t)` persists a Lua table as JSON, `usagi.load()` reads
  it back (`nil` on first run). One file per game, namespaced by a new `game_id`
  field in `_config()` (reverse-DNS, e.g. `com.you.mygame`). Native writes are
  atomic; web routes through `localStorage` so saves persist even when games are
  hosted in custom shells. New `examples/save/`.
- New `usagi tools` tab: SaveInspector. Renders the current project's
  `save.json` with buttons to refresh, clear, and open the containing folder in
  the OS file manager. Press **3** to switch to it.
- Sprite drawing splits into a basic and an extended form:
  - `gfx.spr(index, x, y)` — basic, already existed in v0.1.
  - `gfx.spr_ex(index, x, y, flip_x, flip_y)` — extended, all flip flags
    required.
  - `gfx.sspr(sx, sy, sw, sh, dx, dy)` — arbitrary source rect at 1:1 size.
  - `gfx.sspr_ex(sx, sy, sw, sh, dx, dy, dw, dh, flip_x, flip_y)` — extended,
    all power args required (stretch + both flips). Each function has a single
    fixed signature; no optional trailing args.
- New `usagi.elapsed` field — wall-clock seconds since the session started,
  updated once per frame before `_update`. Frame-stable; doesn't reset on F5.
- The bundled font is now [monogram](https://datagoblin.itch.io/monogram) by
  datagoblin (CC0), replacing raylib's built-in 8 px font. Used for `gfx.text`,
  the FPS overlay, the error overlay, and the tools window. The TTF
  (`assets/monogram.ttf`, ~10 KB) is embedded in the binary at compile time, so
  no runtime filesystem dependency.
- New text-measurement helper: `usagi.measure_text(text)` returns rendered
  `(width, height)` in pixels for the bundled font. Lives on `usagi` rather than
  `gfx` because measurement has no rendering side-effect, and is callable from
  any callback (including `_init`) so layouts can be pre-computed once.
- Music playback: `music.play(name)` plays once, `music.loop(name)` loops,
  `music.stop()` stops the current track. Files live in `<project>/music/`;
  recognized extensions are `.ogg`, `.mp3`, `.wav`, `.flac` (OGG as smaller than
  WAV and is cross-platform ). Only one track plays at a time; calling `play` or
  `loop` while another track is playing stops the old one first. Streams are
  bundled into `.usagi` exports alongside `sfx/` and `sprites.png`. New
  `examples/music`.

Breaking:

- `input.CONFIRM` / `input.CANCEL` are removed; rename to `input.BTN1` /
  `input.BTN2`. The gamepad mapping also shifts: BTN1 is the south face only
  (was south + west) and BTN2 is the east face only (was east + north). The
  north and west faces are now BTN3.
- monogram has a 16 px line height vs raylib's previous 8 px default. Layouts
  that hugged `usagi.GAME_H - 8` or stacked text on 8-pixel rows will need to
  bump offsets to 16 (or read `usagi.measure_text(...)` for an exact value).
- `_config().pixel_perfect` now defaults to `false` (was `true`). At common
  fullscreen resolutions (720p, 1080p, 4K) 320×180 hits an integer multiple
  regardless, and windowed it looks good. Set `pixel_perfect = true` explicitly
  to keep the strict integer-scale-with-bars behavior. (Also fixes a related bug
  where omitting the field from a partially-populated `_config()` table silently
  set it to `false`. Now the default is preserved unless explicitly overridden.)

## v0.1.0 - Apr 27, 2026

Initial release of Usagi, introducing the CLI with `usagi dev`, `usagi run`,
`usagi export`, and `usagi run`.

Includes input, rectangle drawing, sound effect playback, and rendering tiles
from a single `sprites.png.`

Features:

- `gfx.rect` now draws a rectangle outline; use `gfx.rect_fill` for the filled
  variant
- `gfx.circ(x, y, r, color)` — circle outline
- `gfx.circ_fill(x, y, r, color)` — filled circle
- `gfx.line(x1, y1, x2, y2, color)` — line
- Ctrl + R and Cmd + R hard refresh in `usagi dev`
- `usagi export` produces every platform from any host. Default output is
  `export/` containing zips for linux, macos, windows, web, plus the portable
  `.usagi` bundle.
- `--target` accepts `linux`, `macos`, `windows`, `web`, `bundle`, or `all`.
- Runtime templates auto-fetch by version from GitHub Releases on first use,
  cache to a per-OS directory, and verify against published `sha256` sidecars
  before extracting.
- Host platform exports offline (fuses against the running binary, no cache
  lookup or network).
- New flags: `--template-path` (local archive), `--template-url` (custom URL,
  useful for forks and mirrors), `--no-cache` (force re-download), `--web-shell`
  (custom HTML shell for the web export).
- Custom web shell auto-pickup: `<project>/shell.html` is used if present.
- New subcommand `usagi templates {list,clear}` to inspect or wipe the cache.
- Set `USAGI_TEMPLATE_BASE` to point at a fork or mirror for offline /
  air-gapped setups.
- `usagi init [path]` bootstraps a new project. Writes `main.lua` with stubbed
  callbacks, `.luarc.json` for Lua LSP, `.gitignore`, `meta/usagi.lua` (API type
  stubs for editor autocomplete), and `USAGI.md` (engine docs). Defaults to the
  current directory; existing files are skipped, never overwritten.
