# Changelog

History of Usagi releases and what changed in each release. User-facing notes.
Doesn't contain updates relating to developing the engine itself.

## UNRELEASED

Features:

- <kbd>Enter</kbd> opens Pause Menu, like Pico-8.
- New tool: ColorPalette to quickly reference and copy a given color to
  clipboard.
- Left bumper maps to BTN1, right bumper maps to BTN2 to utilize those buttons
  and give input options.

Fixes:

- Dev mode's file walker now checks all Lua files instead of trying to be smart
  about it. Fixes a bug where if there was a syntax error in a required Lua
  file, it'd freeze the chunks and not reliably reload all files.
- Wrap Lua error message in dev overlay to increase legibility.
- Export icon at various sizes for higher res and crisp icons.

## v0.3.0 - May 1, 2026

Features:

- Mouse input. `input.mouse()` returns the cursor position as `x, y` in
  game-space pixels (so it lines up with `gfx.*` coords regardless of window
  size or pixel-perfect scaling). When the cursor sits over the letterbox bars
  the values fall outside `0..GAME_W` / `0..GAME_H`, so games can detect
  off-viewport cursors with a simple bounds check rather than getting clamped
  values. New `input.MOUSE_LEFT` / `input.MOUSE_RIGHT` constants pair with
  `input.mouse_down(button)` / `input.mouse_pressed(button)` (mirroring
  `input.down` / `input.pressed`). `input.set_mouse_visible(visible)` toggles
  the OS cursor (callable from `_init` to hide it before the first frame),
  paired with `input.mouse_visible()`. New examples: `examples/mouse` (custom
  cursor with a particle trail), `examples/mouse_ui` (a click-to-toggle button
  and a draggable box), `examples/mouse_physics` (drag a box to push others
  around with cascading AABB collision), and `examples/waypoint` (click to drop
  waypoints; a unit walks the path).
- Window icon. The Usagi bunny ships embedded as the default; games can override
  via `_config().icon = N` (1-based index into the project's `sprites.png`, same
  indexing as `gfx.spr`). Applied to the game window on Linux/Windows (Cocoa
  doesn't support per-window icons on macOS, so the title bar there always shows
  the system default). The `usagi tools` window also picks up the bunny default.
- macOS `.app` exports which include an `AppIcon.icns` in `Resources/`
  (multi-resolution: 256/512/1024 nearest-neighbor scales of the 16×16 source)
  and reference it via `CFBundleIconFile` in `Info.plist`. Source is the same
  `_config().icon` tile or the embedded default. macOS Dock and Finder show the
  game's icon starting with `usagi export --target macos` builds.
- Per-game settings stored in `settings.json` next to save data
  (`~/Library/Application Support/<game_id>/settings.json` on macOS, matching
  paths via `directories::ProjectDirs` on Linux/Windows; on web, routed through
  `localStorage` under `usagi.settings.<game_id>` like saves). First field is
  `volume` (output, `0.0..=1.0`, defaults to `0.5`). Loaded once at session boot
  and applied to the audio device before the first frame; missing or malformed
  files fall back to defaults so a fresh install Just Works.
- **Shift+M** toggles audio mute, flipping volume between `0.0` and `0.5` (the
  default). The new value is written back to `settings.json` on every toggle, so
  a muted game stays muted across quit/relaunch. Available in both dev and
  shipped builds. Shift required so a stray `M` keypress can't clobber a game
  that binds `M` to gameplay.
- Fullscreen state now persists. **Alt+Enter** still toggles borderless
  fullscreen, and the new value is written to `settings.json` so a player who
  fullscreens stays in fullscreen across relaunches. Applied before the first
  frame so a fullscreen launch doesn't flash a windowed frame. No Lua API or
  `_config` field on purpose: the player's preference owns this setting. Pause
  menu now shows `Volume: NN%` and `Fullscreen: on/off`.
- In-game GIF recording. Press **F9** or **Cmd/Ctrl + G** to start recording;
  press the same key again to save. Files land in `<cwd>/captures/` named
  `<game>-YYYYMMDD-HHMMSS.gif`, where `<game>` comes from your
  `_config().game_id` (e.g. `snake-20260101-120000.gif`). Native-only (web has
  no real filesystem). A small pulsing red "● REC" indicator shows in the
  top-right of the window while recording. Encoder streams frames to disk as
  they're captured, so memory stays bounded on long recordings, and Usagi's
  16-color palette maps directly to GIF's palette format with no quantization,
  so output is pixel-exact. Output is upscaled 2x (640×360, nearest-neighbor) so
  the gif reads cleanly.
- In-game PNG screenshots. Press **F8** or **Cmd/Ctrl + F** to save a single
  frame as `<game>-YYYYMMDD-HHMMSS.png` in the same `<cwd>/captures/` bucket as
  recordings. Same 2x upscale as the gif recorder, lossless, palette-exact.
  `usagi init` now adds `captures/` to `.gitignore`.
- Post-process shaders (advanced, **experimental**). New
  `gfx.shader_set("name")` / `gfx.shader_set(nil)` /
  `gfx.shader_uniform(name,
  value)` Lua API. Drops `shaders/<name>.fs` (and an
  optional `<name>.vs`) from the project root through raylib's GLSL pipeline as
  a final pass when the game render target blits to the window. Web targets
  prefer `<name>_es.fs` (GLSL ES 100, WebGL 1) and desktop prefers `<name>.fs`
  (GLSL 330) so one project can ship both, with a same-name fallback if only one
  variant is present. `gfx.shader_uniform` accepts a number (float) or a
  2/3/4-length numeric table (vec2/vec3/vec4). Live-reloads on save in
  `usagi dev`, with cached uniforms replayed onto the rebuilt shader; compile
  errors print to the terminal and keep the previous shader live. `usagi export`
  walks `shaders/` and bundles every `.fs`/`.vs` so shaders work the same in
  `usagi run`, `.usagi` files, and fused exes on every platform. New
  `examples/shader/` ships a CRT effect and a Game Boy palette swap, cycled with
  BTN1. **Caveats:** the API surface and dual-file convention may still change.
  F8/F9 captures (PNG screenshot, GIF recorder) read the unshaded game RT, so
  post-process effects are visible on screen but **not** in the saved file; use
  your OS's screen recorder or screenshot tool against the game window if you
  need the shader baked into a capture. See the Shaders section in
  `README_DEV.md` for the full writeup.
- More examples! notetris, shaders, mouse, etc. for all new features
- Pause menu pauses the playing music.

Fixes:

- `music.play(name)` / `music.loop(name)` / `music.stop()` are now callable from
  `_init`, not only `_update` / `_draw`. Lets games start a title track the
  moment the window opens without a one-frame gap.
- Log window is hidden on Windows exports when they're launched.

## v0.2.0 - Apr 29, 2026

Features:

- Save data: `usagi.save(t)` persists a Lua table as JSON, `usagi.load()` reads
  it back (`nil` on first run). One file per game, namespaced by a new `game_id`
  field in `_config()` (reverse-DNS, e.g. `com.you.mygame`). Native writes are
  atomic; web routes through `localStorage` so saves persist even when games are
  hosted in custom shells. New `examples/save/`.
- Sprite drawing splits into a basic and an extended form:
  - `gfx.spr(index, x, y)` — basic, already existed in v0.1.
  - `gfx.spr_ex(index, x, y, flip_x, flip_y)` — extended, all flip flags
    required.
  - `gfx.sspr(sx, sy, sw, sh, dx, dy)` — arbitrary source rect at 1:1 size.
  - `gfx.sspr_ex(sx, sy, sw, sh, dx, dy, dw, dh, flip_x, flip_y)` — extended,
    all power args required (stretch + both flips). Each function has a single
    fixed signature; no optional trailing args.
- New `gfx.pixel(x, y, color)` for single-pixel drawing.
- Music playback: `music.play(name)` plays once, `music.loop(name)` loops,
  `music.stop()` stops the current track. Files live in `<project>/music/`;
  recognized extensions are `.ogg`, `.mp3`, `.wav`, `.flac` (OGG as smaller than
  WAV and is cross-platform ). Only one track plays at a time; calling `play` or
  `loop` while another track is playing stops the old one first. Streams are
  bundled into `.usagi` exports alongside `sfx/` and `sprites.png`. New
  `examples/music`.
- Multiple Lua source files are now supported; use `require("file")` to load
  `file.lua`.
- Compound assignment operators: `+=`, `-=`, `*=`, `/=`, `%=` are rewritten to
  plain Lua before parsing, with `runtime.nonstandardSymbol` set in the shipped
  `.luarc.json` so the language server accepts them.
- Input now polls every connected gamepad slot rather than only slot 0. Any
  connected pad (Steam Deck built-in, external pad over USB/Bluetooth) triggers
  actions, and hot-swapping no longer drops input when a pad lands on a
  different slot.
- Three action buttons: `input.BTN1`, `input.BTN2`, `input.BTN3` replace the
  previous `CONFIRM` / `CANCEL` pair. Keyboard: Z/J, X/K, C/L. Gamepad: south,
  east, and (north or west) face buttons. BTN3 fires for both Xbox Y and X (PS
  Triangle and Square) so it's reachable from either side of the diamond.
- New `examples/rng.lua` demonstrates `math.random` (PRNG is auto-seeded on
  startup) and how to call `math.randomseed(n)` for deterministic sequences.
- New `usagi tools` tab: SaveInspector. Renders the current project's
  `save.json` with buttons to refresh, clear, and open the containing folder in
  the OS file manager. Press **3** to switch to it.
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
- Engine-level pause menu. **Esc**, **P**, or gamepad **Start** opens it; the
  same buttons (plus **BTN2**) close it. While open, `_update` and `_draw` are
  skipped and the screen shows a black "PAUSED" overlay (music keeps streaming).
  Foundation for a menu with volume, input remap, and game-registered hooks.
  **Shift+Esc** in dev now quits the game, replacing raylib's default
  Esc-quits-immediately default.
- Revised and improved documentation.
- More [examples](https://github.com/brettchalupa/usagi/tree/main/examples),
  including a Pico-8 shim, dialog box, save demo, music, multifile, and more.

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
