# Changelog

History of Usagi releases and what changed in each release. User-facing notes.
Doesn't contain updates relating to developing the engine itself.

## UNRELEASED

Breaking:

- Color slot indices are now **1-based** instead of 0-based, matching `gfx.spr`
  and Lua's array convention. The `gfx.COLOR_*` constants shift up by one.
  `gfx.COLOR_BLACK` is now `1` (was `0`), `gfx.COLOR_PEACH` is `16` (was `15`).
  Code that uses the named constants is unaffected; code that passes literal
  integers (`gfx.clear(0)`, `gfx.rect_fill(..., 7)`) needs to bump each literal
  by 1 or switch to the named constants. Slot `0` and any index above the active
  palette's length now render as the magenta out-of-range sentinel.
- `gfx.spr_ex` and `gfx.sspr_ex` gained three required trailing params:
  `rotation` (radians), `tint` (palette color), and `alpha` (`0..1`). Use
  `0, gfx.COLOR_WHITE, 1.0` for the identity values to preserve the old
  behavior. The simple `gfx.spr` / `gfx.sspr` signatures are unchanged. See the
  README's "Scaling sprites" subsection for wrapper recipes if you find the
  verbose call sites painful.

Features:

- Custom color palettes via `palette.png`. Drop a PNG at your project root and
  Usagi swaps the default Pico-8 palette for yours. Pixels read in row-major
  order (left-to-right, top-to-bottom) so any rectangular shape works. 16x1
  strips, 16x2 grids, 4x4 grids, etc. Color count = `width ×
  height`.
  lospec.com's "1px cells" exports are the canonical source. Hot-reloads like
  `sprites.png`, ships in `usagi export` bundles. The `gfx.COLOR_*` constants
  stay as slot indices, so they keep resolving through the same slots, but the
  RGB at each slot is whatever you painted. Slots beyond your palette's range
  render as magenta. The ColorPalette tool reflects the active palette. New
  `examples/palette_swap` ships a sweetie16 palette; delete its `palette.png` to
  see the same Lua in Pico-8 colors.
- New `sfx.play_ex(name, volume, pitch, pan)`,
  `music.play_ex(name, volume, pitch, pan, loop)`, and
  `music.mutate(volume, pitch, pan)` for programmatic audio control. `play_ex`
  is fire-and-forget per-call params (use it for random-pitch footsteps, panned
  UI cues, attenuated dialogue beeps). `music.mutate` modulates the
  currently-playing track in place with replace semantics. This is useful for
  ducking music under dialog, pitch-warping during hitstun, and fade-outs. Pan
  is `-1..1` (left to right), volume `0..1`, pitch a raw multiplier (`1.0` =
  identity). The `examples/sound` demo gets a random-pitch jump on BTN3;
  `examples/music` ducks the track while LEFT is held.
- New `gfx.text_ex(text, x, y, scale, rotation, color)` for scaled and rotated
  text. Scale is a font-size multiplier (use integers for crisp pixel-art text;
  fractional values blur). Rotation is in radians (use `math.rad(deg)` for
  literal degrees) and pivots around the text's center. New `examples/text`
  shows a big scaled title, a sin-wave wiggling subtitle, and a static tilted
  label.
- New `gfx.rect_ex(x, y, w, h, thickness, color)`,
  `gfx.circ_ex(x, y, r, thickness, color)`, and
  `gfx.line_ex(x1, y1, x2, y2, thickness, color)` for thick-stroke shape
  outlines. `circ_ex` strokes are centered on the nominal radius so concentric
  rings at adjacent radii sit flush instead of leaving rounding gaps. The
  `examples/shapes` demo now includes a small concentric-rings showcase.
- `gfx.spr_ex` / `gfx.sspr_ex` now support rotation, tint, and alpha. Rotation
  is in radians (use `math.rad(deg)` for literal-degree values) and pivots
  around the sprite's center. Tint is a palette color multiplied over the sprite
  (`gfx.COLOR_WHITE` is the identity; other colors recolor for hit flashes
  etc.). Alpha is `0..1` for fade-in/out. The `examples/spr` demo now exercises
  all three (spinning bunny, tint-flashing ship via BTN1, pulsing-alpha bullet).
- New `input.mouse_scroll()` returns the per-frame vertical scroll delta
  (positive up, negative down, `0` when no scroll). Works the same on a mouse
  wheel or a trackpad two-finger swipe. The mouse example now uses it to cycle
  the spark color.
- Upgraded the embedded Lua runtime from 5.4 to 5.5. No game-code changes are
  expected for typical Usagi scripts; the bundled `.luarc.json` (shipped via
  `usagi init` / `usagi refresh`) now pins `runtime.version` to `Lua 5.5` so the
  LSP matches.
- TilePicker: LMB on a tile copies its `spr` index (existing behavior). RMB
  click-and-drag now selects a tile-aligned rectangle on the sheet and copies
  `sx,sy,sw,sh` ready for `sspr`. The current selection stays visible: a
  highlight box on the sheet plus a readout in the header showing both the `spr`
  index (for single tiles) and the `sspr` source rect. A live preview rect
  tracks the drag.
- `sprites.png` is now explicitly loaded with POINT (nearest-neighbor) texture
  filtering, matching the bundled font, so the pixel-art intent is pinned in the
  engine rather than relying on a default.
- TilePicker: hold middle mouse and drag, or hold space and drag with the left
  mouse, to pan the sheet. Use the scroll wheel to zoom (anchored on the cursor
  so the pixel under the mouse stays put). The header also shows the sheet pixel
  coords under the cursor.
- New `usagi.dump(v)` helper: pretty-prints any Lua value to a string, recursing
  into tables with sorted keys and cycle detection. Pair with `print` for
  terminal debugging, or feed into `gfx.text` to draw on screen.
- New `gfx.px(x, y)` reads a pixel from the most recently rendered frame and
  returns `(r, g, b, palette_index)` as multiple values. The palette index is
  the 1-based slot for an exact RGB match, or `nil` for off-palette colors. All
  four returns are `nil` for off-screen coordinates or on the very first frame
  before any drawing has happened. Reads reflect the previous frame's finished
  image, so they don't see in-progress draws inside the current `_draw`. Useful
  for collision-by-color, fog-of-war reveals, palette-swap effects, and water
  reflections.
- New `gfx.spr_px(index, x, y)` reads a pixel from `sprites.png`. `index` is a
  1-based sprite slot (same shape as `gfx.spr`); `(x, y)` is the offset inside
  that cell. Returns the same `(r, g, b, palette_index)` shape as `gfx.px`. All
  four returns are `nil` for an out-of-range index, out-of-cell coordinates, a
  project with no `sprites.png`, or a fully transparent source pixel (so the
  ergonomic `if r then ...` check covers both "no sheet" and "alpha hole"
  cases). Useful for pixel-perfect sprite collision and for data-baked levels
  where you paint the layout into the sheet and scan it at startup.
- New `examples/px` cart demonstrates both reads side-by-side: a small maze
  where movement consults `gfx.px` for collision-by-color, plus a `gfx.spr_px`
  scan that re-renders sprite 1 pixel-by-pixel next to its `gfx.spr` original.
- Bundled font upgraded from monogram (95 ASCII glyphs) to monogram-extended
  (504 glyphs covering full Basic Latin, Latin-1 Supplement, Latin Extended-A,
  partial Greek and Cyrillic). Text like `café naïve jalapeño`,
  `Здравствуй, мир!`, and `Καλημέρα κόσμε` now render. Same look, same line
  height, just more codepoints. Updated `examples/text` shows the new chars.
- Custom font support: drop `font.png` at your project root and Usagi uses it
  for `gfx.text` / `gfx.text_ex` / `usagi.measure_text`. Engine UI (FPS overlay,
  pause menu, error text) keeps the bundled font so layout stays predictable
  regardless of what you ship. The font's natural line height drives
  `font.base_size()` at runtime, so larger or smaller fonts render at their
  design size with no scaling.
- New `usagi font bake <font.ttf> <size>` subcommand bakes a TTF/OTF into the
  custom-font format (a single PNG with glyph metadata embedded as a zTXt
  chunk). Defaults to writing `font.png` in the current directory, so the output
  is immediately a project drop-in. Includes the CJK Unified Ideographs block by
  default for fonts that cover it (kanji/hanzi/hanja); pass `--no-cjk` to skip.
  Pass the font's natural design size for crispest output (e.g., `15` for
  monogram-style 5×7 fonts, `18` for Silver, `8` for Misaki Gothic). New
  `examples/custom_font` ships a Silver-baked demo with multi-script text.
- Per-game gamepad remapping for BTN1/BTN2/BTN3 alongside the existing keyboard
  remapping. New "Configure Gamepad" entry under the pause menu's Input
  sub-menu, mirroring the Pico-8-style sequential capture flow used by
  "Configure Keys": face buttons and shoulder/trigger positions are bindable;
  dpad, Select/Start/Home, and stick clicks are reserved. Bksp or Select undoes
  the previous capture; Del or Start resets every override. Overrides persist
  per game as `pad_map.json` next to `keymap.json`. Pause titles renamed to
  `KEYBOARD CONFIG` and `GAMEPAD CONFIG`.
- Pause menu reorganized: a new Settings sub-menu now holds Music, SFX,
  Fullscreen, and Input, so the Top stays focused on Continue plus the
  destructive actions (Clear Save Data, Reset Game, Quit). The Input sub-menu is
  reached via Top > Settings > Input.
- Pause menu lays out more reasonably at low resolutions. Item left-margin and
  the Input Tester's binding columns scale with the game's resolution instead of
  fixed pixel offsets, so games running at non-default sizes (vertical
  orientations, 128x128 prototypes) no longer push columns past the right edge.
  The volume meter's `xx%` readout is omitted when it wouldn't fit.
- New `usagi.menu_item(label, callback)` registers up to 3 custom rows on the
  pause menu's Top view, between Continue and Settings. Use cases: jumping back
  to a title screen, bumping a level counter, granting resources for testing.
  The callback fires on selection; the menu closes by default but stays open if
  the callback returns Lua `true` (handy for repeatable in-game tweaks). Items
  auto-clear before each `_init` re-run so fresh registrations always land on a
  clean slate. `usagi.clear_menu_items()` wipes them manually, which is what
  `examples/menu_item.lua` uses to swap the registered set when transitioning
  between its title and gameplay scenes.
- New `usagi.toggle_fullscreen()` flips fullscreen state from Lua and returns
  the new state as a bool. Paired with `usagi.is_fullscreen()` for reading
  current state without flipping. Both persist to `settings.json` the same way
  the pause-menu Fullscreen row and the Alt+Enter shortcut do, so the three
  toggle paths stay in sync. Intended for games that ship a custom pause menu or
  settings screen and need to drive fullscreen from script.
- New `usagi.PLATFORM` string reports the build target the binary was compiled
  for: `"web"`, `"macos"`, `"linux"`, `"windows"`, or `"unknown"` (for builds on
  uncovered targets like BSDs). Lets games gate code paths by platform without
  parsing user-agent strings or shelling out, e.g.
  `if usagi.PLATFORM ~= "web" then ... end` for desktop-only features.
- New `usagi.quit()` terminates the main loop the same way the pause-menu Quit
  row and Shift+Esc do, intended for custom in-game pause / title menus. On web
  the call still flips the internal flag but the emscripten main loop owns
  lifetime, so the canvas freezes on the last frame rather than tearing down the
  page. Gate with `usagi.PLATFORM` if your custom menu shouldn't expose a quit
  option on web.

## v0.7.2 - May 10, 2026

Fixes:

- `input.mapping_for` now properly returns the key string instead of `"?"` for
  custom mappings.
- VSync enabled for the engine to fix screen tearing. See
  [#132](https://github.com/brettchalupa/usagi/issues/132)

## v0.7.1 - May 10, 2026

Features:

- Usagi version is logged on game start.
- Raylib log level is set to warning to reduce noise. Set env var
  `USAGI_VERBOSE=1` to get full Raylib logs.

Fixes:

- Web games don't crash when loading due to fullscreen error. See
  [#154](https://github.com/brettchalupa/usagi/issues/154)
- Web games now load with default shell. They no longer display a black screen.
  Fixes CSS error introduced in v0.7.0.

## v0.7.0 - May 9, 2026

Features:

- `MOUSE_MIDDLE` (a.k.a. scroll wheel click) support for mouse input checks.
- `effect.stop()` to end all currently running effects.

Fixes:

- `input.pressed` and `input.released` now edge-detect the analog stick the same
  way they do the d-pad, so menus (including the engine pause menu) can be
  navigated with the left stick.
- Closing and opening the menu swallows input so that it doesn't accidentally
  trigger presses. See [#130](https://github.com/brettchalupa/usagi/issues/130)
- Live reload with `usagi dev main.lua` or `usagi dev game.lua` now works again.
  Nested paths or `usagi dev` worked as expected in v0.6.1 but passing a
  filename within the current directory would break live reload. See
  [#136](https://github.com/brettchalupa/usagi/issues/136)
- <kbd>Enter</kbd> no longer closes the Pause menu when it's open but instead is
  used to confirm the selection. It was awkward as a toggle.
- Toggle fullscreen now works on web.
- "Quit" Pause menu option is now hidden on web since it didn't do anything.
- Up and down page navigation keyboard keys no longer scroll the page for web
  builds. Before this up and down, etc. were scroll page, not registering as
  game input. See [#112](https://github.com/brettchalupa/usagi/issues/112)
- Effects reset when the game resets. Before they'd keep running, shaking the
  screen or hitstopping when they shouldn't.

## v0.6.1 - May 6, 2026

Fixes:

- No longer crash on Windows when `require`d Lua files have a syntax error. See
  [#105](https://github.com/brettchalupa/usagi/issues/105).
- No longer crash on Windows when invalid args are passed. See
  [#103](https://github.com/brettchalupa/usagi/issues/103).

## v0.6.0 - May 5, 2026

Features:

- Experimental `usagi update` command to update the binary in place when new
  versions are released. Won't be useful and fully testable until the next
  release comes out.
- `usagi refresh` command to update the ancillary engine files when a new
  version is released. Currently updates `meta/usagi.lua`, `.luarc.json`, and
  `USAGI.md`. Does **not** update `main.lua`. Use this after `usagi update` to
  get the docs and LSP integration for the `usagi -V` you're using.
- New `effect.*` Lua module for engine-level juice. Four primitives, all decay
  automatically once per frame:
  - `effect.hitstop(time)` freezes `_update` for `time` seconds.
  - `effect.screen_shake(time, intensity)` shakes the blit, magnitude in game
    pixels, decays linearly.
  - `effect.flash(time, color)` full-screen palette-color overlay that fades
    from opaque to transparent.
  - `effect.slow_mo(time, scale)` scales the `dt` passed to `_update`;
    `scale=0.5` is half-speed, `scale=0` freezes (use `hitstop` for that).
    Stacking rule across all four: longer duration wins, latest magnitude wins;
    spam-calling is safe. See `examples/effect.lua` for a runnable demo. The
    `notetris` example now uses `effect.screen_shake` in place of its bespoke
    shake.
- New `usagi.SPRITE_SIZE` constant (default `16`) for tile-grid math without
  hardcoding the cell size. Same value the engine uses internally for `gfx.spr`
  indexing, the tilepicker tool, and the window-icon slicer. Override the
  default by setting `_config().sprite_size`; the new value flows through every
  consumer (Lua draws, icon slice on session and `usagi export --target macos`,
  tilepicker grid in `usagi tools`).
- `_config()` can override the game's render resolution via `game_width` and
  `game_height` (defaults 320 and 180). The internal RT is sized to those dims;
  `usagi.GAME_W` / `GAME_H` reflect the active values. Tested band is roughly
  320x180 to 640x360; pause-menu and tools UI are pixel-fixed and may overflow
  at very small sizes or look sparse at very large ones. The web export
  templates the canvas backing-store and aspect ratio from the configured
  resolution, so non-16:9 / non-default games ship correctly with the default
  shell (no `--web-shell` needed) and embed cleanly in itch at any iframe size.
  Sprite size and bundled font are still fixed at 16 and 5x7.

  ```lua
  function _config()
    return { game_width = 480, game_height = 270 }
  end
  ```

Fixes:

- A persistent error in `_update` or `_draw` no longer spams stderr 60x/sec.
  `record_err` now logs only when the message changes; the on-screen overlay
  still updates every frame, so users see live changes when they edit and save.

## v0.5.0 - May 4, 2026

Breaking:

- `settings.json` replaces the single `volume` key with `music_volume` and
  `sfx_volume`. Existing settings are auto-migrated on load: the old `volume`
  value is copied into both new fields the first time the engine reads them. No
  action required.
- `input.down(action)` is renamed to `input.held(action)` and
  `input.mouse_down(button)` to `input.mouse_held(button)`. The old names
  collided with directional input names (`input.down(input.DOWN)` was ambiguous)
  and "held" reads more naturally as the level-state pair to the edge-state
  `pressed` / `released`. Update calls; behavior is unchanged.

Features:

- Pause Menu is now navigable. Up/Down moves between items, Left/Right adjusts
  values, BTN1 (<kbd>Z</kbd>/gamepad-A) confirms, BTN2 (<kbd>X</kbd>/gamepad-B)
  goes back. Items: Continue, Music volume, SFX volume, Fullscreen, Input, Clear
  Save Data (with a confirm dialog), Reset Game, Quit.
- Music and SFX have separate persisted volume levels, each rendered as a 5-bar
  meter (steps of 20%). Default is 80% for both.
- <kbd>Shift</kbd>+<kbd>M</kbd> now mutes both channels at once and a second
  press restores both to their defaults.
- Pause Menu's Input view shows a live tester (D-pad + button rects light up
  while pressed) and a "Configure Keys >" entry that opens a Pico-8-style
  keyconfig flow: highlights one action at a time, captures one key, advances.
  <kbd>Esc</kbd> cancels, <kbd>Delete</kbd> resets all overrides. Keyboard-only
  for now; gamepad bindings stay fixed. The new mappings persist per-game in
  `keymap.json` next to `settings.json` (web: localStorage
  `usagi.keymap.<game_id>`). Override semantics are "replace": once you map LEFT
  to W, the default arrow Left no longer fires LEFT.
- Switch face buttons follow Nintendo convention: BTN1 fires from A (east) and
  BTN2 from B (south), so "A confirms, B cancels" feels native on Switch. Xbox
  and PlayStation are unchanged (BTN1=south, BTN2=east). Triggers (LB/RB) and
  BTN3 stay put across all families.
- New Lua API for source-aware control glyphs: `input.mapping_for(action)`
  returns the label of the active source's primary binding (e.g. `"Z"` while the
  player is on keyboard; `"A"` on Xbox, `"Cross"` on PlayStation, `"B"` on
  Switch when the active source is gamepad). Gamepad family is auto-detected via
  `GetGamepadName` and falls back to Xbox for unknown / generic / Steam Deck
  pads. The engine tracks the most recent source automatically, switching only
  when a _bound_ input fires so stray keys don't flip it. `input.last_source()`
  returns `"keyboard"` or `"gamepad"`; matching constants are
  `input.SOURCE_KEYBOARD` and `input.SOURCE_GAMEPAD`. Examples that previously
  hardcoded `BTN1`/`BTN2`/`BTN3` in their on-screen prompts (sound, music, save,
  shader, rng, snake, dialog, operators, mouse) now use `input.mapping_for` so
  the prompts adapt to the active device.
- New input functions: `input.released(action)` and
  `input.mouse_released(button)` fire the frame the input transitions from held
  to up. Mirrors `pressed` for the release edge; useful for charge-and-release
  mechanics (jump-on-release, slingshot pull-back).
- New `util` global with drop-in math/geometry helpers: `util.clamp`,
  `util.sign`, `util.round`, `util.approach`, `util.lerp`, `util.wrap`,
  `util.flash`, `util.vec_normalize`, `util.vec_dist`, `util.vec_dist_sq`,
  `util.vec_from_angle`, `util.point_in_rect`, `util.point_in_circ`,
  `util.rect_overlap`, `util.circ_overlap`, `util.circ_rect_overlap`. Pure Lua,
  available without `require`. Source is at `runtime/util.lua` for forkability.
  Open it to read the implementations or override individual functions in your
  own `_init`. Functions that take shaped tables check the shape and raise an
  error pointing at your call site (e.g.
  `util.rect_overlap: arg 1 table
missing or non-numeric field 'h'`) instead of
  failing deep inside the helper. `util.min` / `util.max` aren't included since
  Lua's `math.min` / `math.max` already do the job.
- Direct keyboard reads via `input.key_pressed(key)`, `input.key_held(key)`, and
  `input.key_released(key)`, paired with `input.KEY_*` constants for letters,
  digits, F1–F12, arrows, modifiers, common punctuation, and a few specials
  (Space, Enter, Escape, Tab, Backspace, Delete). Documented as an escape hatch
  — these bypass the keymap override and gamepad bindings, so they're intended
  for dev hotkeys (e.g. <kbd>F1</kbd> to toggle a debug overlay) and
  keyboard-and-mouse-only games. Anything a player should be able to remap or
  reach with a controller still belongs on the abstract `input.held` /
  `input.pressed` / `input.released` actions. Raw gamepad reads remain
  intentionally unexposed.
- CLI output is now color-coded: success and reload messages render in green,
  warnings (graceful fallbacks like a missing keymap or skipped export target)
  in yellow, and errors in red. The `[usagi]` prefix is dimmed so the message
  itself reads as the foreground content. Reload messages no longer look like
  errors at a glance. Color is auto-disabled when stdout isn't a terminal
  (piping to a file, CI logs) or when `NO_COLOR` is set, per
  <https://no-color.org>.

Fixes:

- <kbd>Alt</kbd>+<kbd>Enter</kbd> no longer opens the Pause Menu while toggling
  fullscreen. Pause Menu only opens on <kbd>Enter</kbd> when Alt isn't held.
- `usagi` commands now correctly log the output on Windows and the exported game
  window does not show them, see #79.

## v0.4.0 - May 3, 2026

Features:

- <kbd>Enter</kbd> opens Pause Menu, like Pico-8.
- New tool: ColorPalette to quickly reference and copy a given color to
  clipboard.
- Left bumper maps to BTN1, right bumper maps to BTN2 to utilize those buttons
  and give input options.

Breaking:

- `_config().title` is renamed to `_config().name`. `name` is the canonical
  display name across the engine (window title, macOS `.app` directory,
  Info.plist, slugged for archive/binary filenames on `usagi export`). The old
  `title` key is no longer read; rename to `name` in your `_config()` table. All
  shipped examples and the `usagi init` template were updated.

Fixes:

- Dev mode's file walker now checks all Lua files instead of trying to be smart
  about it. Fixes a bug where if there was a syntax error in a required Lua
  file, it'd freeze the chunks and not reliably reload all files.
- Wrap Lua error message in dev overlay to increase legibility.
- Export icon at various sizes for higher res and crisp icons.

Tweaks:

- The web shell's colors are slightly revised to use the engine's color palette.
- The `.luarc.json` from `usagi init` no longer disables the `lowercase-global`
  rule to help prevent the accidental creation of globals. In the examples and
  my own games, this has happened multiple times and is a serious footgun. So
  the default is revised. Example styles updated accordingly. Feel free to
  change your `.luarc.json`, it's your project!
- `usagi export` now uses `_config().name` to drive archive filenames, the
  Linux/Windows binary names (slugged to ASCII kebab-case, e.g. `Sprite Example`
  → `sprite-example-linux.zip`), and the macOS bundle directory
  (`Sprite Example.app`). Falls back to the project directory name when `name`
  isn't set.
- Docs improvements to suggest external tools to use.

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
