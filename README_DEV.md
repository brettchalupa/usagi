<img alt="Usagi Logo: pixel art bunny, Usagi Engine - Rapid 2D Prototyping" src="/website/card-logo.png" />

# Usagi - Simple 2D Game Engine for Rapid Prototyping

Usagi is a simple 2D game engine for quickly making games with **Lua** 5.4. It
features live-reloading as you change your game code and assets. Its API is
clear, consistent, and familiar.

[Watch the intro video!](https://www.youtube.com/watch?v=byp3rKd626M)

**WARNING:** Usagi is early in development and not stable. APIs and commands are
likely to change until v1.0.0 releases.

Usagi is made by [Brett Chalupa](https://brettmakesgames.com) and dedicated to
the public domain.

[There's a chill Discord if you want to chat about the engine, share what you
make, and get help.](https://usagiengine.com/discord)

## Install

[Download the latest Usagi build for your operating
system.](https://github.com/brettchalupa/usagi/releases/latest)

**Latest Usagi release:** v0.5.0

You can keep the `usagi` executable in your project folder or install it
globally on your computer.

Other places to download Usagi:

- [itch.io](https://brettchalupa.itch.io/usagi)

## Features / Bugs

Usagi embraces a few constraints inspired by Pico-8 and Pyxel to help focus on
prototyping rather than making polished high-resolution graphics. These may
change in the future or be configurable.

- **Live Reload**: when you run `usagi dev`, your game automatically updates
  with your newest code and assets, enabling rapid development
- **Cross Platform Export**: run `usagi export` and your game is exported for
  Linux, macOS, Windows, and web
- **Limited Resolution**: 320px by 180px - 16:9 aspect ratio that scales nicely
  to common monitor sizes
- **One Spritesheet**: `sprites.png` is the only image file for textures that
  can be loaded
- **Small API**: you can't do everything with Usagi, but there's enough to make
  simple 2D games
- **Sprite Size**: 16px by 16px - using `gfx.spr` uses the index based on this
  sized sprite; you can draw larger sprites with `gfx.sspr`
- **Pico-8 Colors**: the color palette for drawing are the same as Pico-8 (but
  with constants for easy reference)

You currently must bring your own sound effects and sprite editor. A sprite
editor could be nice in the future as part of the `usagi tools`.

## Hello, Usagi

You now have the `usagi` CLI that you can run from your shell (`usagi.exe` on
Windows).

Starting development is as simple as creating `main.lua`, running `usagi dev`,
and coding:

```lua
function _draw(_dt)
  gfx.clear(gfx.COLOR_BLACK)
  gfx.text("Hello, Usagi!", 10, 10, gfx.COLOR_WHITE)
end
```

You can quickly bootstrap a new project and start it in dev mode:

```sh
usagi init my_game
cd my_game
usagi dev
```

`init` writes `main.lua` (with stubbed `_init` / `_update` / `_draw` functions),
`.luarc.json` for Lua LSP support, `.gitignore`, `meta/usagi.lua` (API type
stubs), and `USAGI.md` (a copy of these docs).

Edit `main.lua` and save. The Usagi runtime automatically reloads, so your
changes show up live without losing game state.

In most traditional game development environments, you would need to restart
your game's executable after making changes. Usagi lets you focus on coding and
making art without losing the current game state, allowing for much faster
iteration cycles.

Need to revise a sprite quickly? Just open `sprites.png` in your sprite editor,
change it, save it, and see it update in the context of your game.

## Updating Usagi

Replace the `usagi` binary with a newer release from your preferred download
source. You can also run `usagi update` to fetch the latest version if there is
one.

To refresh engine-owned files in a project (the LSP type stubs and the embedded
docs), run: `usagi refresh`. It updates `meta/usagi.lua`, `.luarc.json`, and
`USAGI.md`. Does **not** update `main.lua`. Use this after `usagi update` to get
the docs and LSP integration for the `usagi -V` you're using.

## Project Goal

Usagi does not aim to be anything more than a rapid development engine for
simple, pixel art games. It doesn't intend to support mobile platforms or mobile
or VR. It doesn't aim to replace Love2D or Pico-8 or Picotron. It's not a
fantasy console. It's a command-line program and suite of tools to help you make
games quickly.

Usagi is great for those learning game programming. And for those who to use
something more flexible than Pico-8/Picotron but more constrained than Love2D.

**Why Lua:** Lua is a widely-used language in game programming, and it's quite
simple yet surprisingly powerful, making it a good fit for Usagi.

If you want to build a medium-to-large polished game, Usagi would not be a good
fit.

## Project Layout

An Usagi game is either a single `.lua` file or a directory with a `main.lua` in
it. Additional `.lua` files anywhere under the project root can be loaded with
stock Lua's `require`. Optional assets live alongside:

```
my_game/
  main.lua        -- required: your game's entry point
  enemies.lua     -- optional: require "enemies"
  world/
    tiles.lua    -- optional: require "world.tiles"
  sprites.png    -- optional: 16×16 sprite sheet (PNG with alpha)
  sfx/           -- optional: .wav files, file stems become sfx names
    jump.wav
    coin.wav
  music/         -- optional: .ogg/.mp3/.wav/.flac, file stems become track names
    overworld.ogg
    boss.ogg
  shaders/       -- optional: post-process GLSL shaders (advanced; see Shaders)
    crt.fs       -- desktop GLSL 330
    crt_es.fs    -- web GLSL ES 100
```

`require "name"` resolves to `name.lua` in the project root, falling back to
`name/init.lua` if the first miss. Dotted names (`require "world.tiles"`) become
slash-separated paths. The same lookup works inside a fused / exported build, so
multi-file projects ship as a single binary or `.usagi` with no extra config.

Run with:

- `usagi init path/to/new_game` bootstraps a project (main.lua stub,
  `.luarc.json`, `.gitignore`, LSP stubs, `USAGI.md` docs).
- `usagi dev path/to/my_game` for live-reload development (script, sprites, and
  sfx reload on save; F5 resets state).
- `usagi run path/to/my_game` to run without live-reload.
- `usagi tools [path]` opens the Usagi tools window (jukebox, tile picker). See
  the **Tools** section below.
- `usagi export path/to/my_game` packages a game for distribution: zips for
  Linux, macOS, Windows, and the web, plus a portable `.usagi` bundle. See the
  **Export** section below.

Can you also run Usagi commands without the path to have it run in the current
directory, like `usagi dev` or `usagi export`.

## Lua API

**Philosophy:** keep it simple, name things clearly, and prefer fixed function
signatures.

**Style**: for Lua, 2 spaces indent with `snake_case` for locals, function
names, and table fields. `SCREAMING_SNAKE_CASE` for file-scope constants
(`local TICK = 0.12`, `gfx.COLOR_*`). Cross-frame globals are **`Capitalized`**
— the canonical game-state container is `State`, set inside `_init`; module
imports kept as globals are `Player = require("player")`. The shipped
`.luarc.json` enables `lowercase-global`, so any unguarded lowercase assignment
at file scope is flagged as an accidental missing `local`. Engine API (`gfx`,
`input`, `sfx`, `music`, `usagi`) stays lowercase and is exempt from the lint
via `meta/usagi.lua`.

### Compound assignment operators

Usagi runs each `.lua` source through a tiny preprocessor before handing it to
the Lua VM, adding compound assignment sugar:

| operator | rewrite     |
| -------- | ----------- |
| `+=`     | `x = x + y` |
| `-=`     | `x = x - y` |
| `*=`     | `x = x * y` |
| `/=`     | `x = x / y` |
| `%=`     | `x = x % y` |

```lua
State.score += 1
State.timer += dt
```

Limitations: the rewrite is line-anchored, so `if cond then x += 1 end` is left
as-is (use longhand). The LHS is duplicated verbatim, so `t[f()] += 1` calls
`f()` twice — same gotcha as PICO-8's preprocessor.

The shipped `.luarc.json` declares these as nonstandard symbols so the
lua-language-server stops underlining them as syntax errors.

### Callbacks

Define any of these as globals for Usagi to call them:

- `_init()` — once at start, and when the user presses **F5**. Initialize
  `State` (and any other cross-frame globals) here.
- `_update(dt)` — each frame, before draw. `dt` is seconds since last frame.
- `_draw(dt)` — each frame, after update. `dt` same as above.
- `_config()` — optional. Called **once at startup, before the window opens**;
  must return a config table.

#### `_config`

Supported fields:

- `name`: display name. Drives the window title bar, the macOS `.app` bundle
  directory (`Sprite Example.app`), the Info.plist `CFBundleName` /
  `CFBundleDisplayName`, and (after slugging to ASCII kebab-case) the archive
  filenames + Linux/Windows binary names produced by `usagi
  export`. Defaults
  to the project directory name (`examples/spr/main.lua` → "spr"); falls back to
  "Usagi" if no path is available.
- `pixel_perfect` (default `false`): when `true`, the game renders at integer
  scale multiples only (1×, 2×, 3×, ...) with black letterbox bars filling any
  leftover window space. When `false`, the game scales at any factor that fits
  the window while preserving the game's aspect ratio, so bars only appear on
  the axis with extra room, never distorting the image. The default is `false`
  because at common fullscreen resolutions (720p, 1080p, 4K) the game's 320×180
  native size lands on an integer multiple anyway, and in windowed mode it looks
  good still.
- `game_id`: reverse-DNS string like `com.brettmakesgames.snake`, namespaces
  save data and the macOS bundle identifier. Optional.
- `icon`: 1-based tile index into `sprites.png`, used as the window icon and (on
  `usagi export --target macos`) the `.app` icon.

```lua
function _config()
  return {
    name = "Snake",
    pixel_perfect = true,
    game_id = "com.example.snake",
    icon = 1,
  }
end
```

`icon` (optional) is a 1-based tile index into your `sprites.png`, same indexing
as `gfx.spr`. Omitted, the embedded Usagi bunny is used. The chosen tile is
applied to the game window on Linux/Windows (Cocoa ignores per-window icons on
macOS, so the title bar there always shows the system default). At
`usagi export --target macos` time the same tile is scaled up and packed into
`Resources/AppIcon.icns` inside the `.app`, which is what the macOS Dock/Finder
pick up.

`_config()` runs before the runtime is fully alive (the window doesn't exist
yet), so its return value is **read once at startup and cached**. Editing
`_config()` while the game is running won't update the title or any future
config field on save; restart the session to pick up changes.

### `gfx`

Draws to the screen. Positions are in game-space pixels (320×180). Colors are
palette indices 0-15; use the named constants.

- `gfx.clear(color)` — fill the screen.
- `gfx.rect(x, y, w, h, color)` — rectangle outline.
- `gfx.rect_fill(x, y, w, h, color)` — filled rectangle.
- `gfx.circ(x, y, r, color)` — circle outline centered at `(x, y)`.
- `gfx.circ_fill(x, y, r, color)` — filled circle centered at `(x, y)`.
- `gfx.line(x1, y1, x2, y2, color)` — line from `(x1, y1)` to `(x2, y2)`.
- `gfx.pixel(x, y, color)` — set a single pixel.
- `gfx.text(text, x, y, color)` — bundled monogram font (5×7 pixel font, 16 px
  line height; see Credits below). To measure text dimensions, use
  `usagi.measure_text` — it lives on `usagi` rather than `gfx` because
  measurement is a pure utility (no render side-effect) and is callable from any
  callback, including `_init`.
- `gfx.spr(index, x, y)` — draw the 16×16 sprite at `index` (1 = top-left) from
  `sprites.png`.
- `gfx.spr_ex(index, x, y, flip_x, flip_y)` — extended `spr`: requires both flip
  booleans.
- `gfx.sspr(sx, sy, sw, sh, dx, dy)` — draw an arbitrary `(sx, sy, sw, sh)`
  rectangle from `sprites.png` at `(dx, dy)` at original size.
- `gfx.sspr_ex(sx, sy, sw, sh, dx, dy, dw, dh, flip_x, flip_y)` — extended
  `sspr`: stretches to `(dw, dh)` and flips per the booleans, all required.
- `gfx.COLOR_BLACK`, `COLOR_DARK_BLUE`, `COLOR_DARK_PURPLE`, `COLOR_DARK_GREEN`,
  `COLOR_BROWN`, `COLOR_DARK_GRAY`, `COLOR_LIGHT_GRAY`, `COLOR_WHITE`,
  `COLOR_RED`, `COLOR_ORANGE`, `COLOR_YELLOW`, `COLOR_GREEN`, `COLOR_BLUE`,
  `COLOR_INDIGO`, `COLOR_PINK`, `COLOR_PEACH` — the Pico-8 palette, indices
  0-15.

The `_ex` variants pack every power-arg into one fixed signature instead of
trailing optionals. With a single `_ex` per primitive there's exactly one
decision per draw ("simple or extended?"). If you want shorter call sites, write
a thin wrapper.

### `input`

Abstract input actions. Each action is a union over keyboard, gamepad buttons,
and the left analog stick; any connected gamepad fires every action, so the
Steam Deck's built-in pad and an external pad both work, and hot-swapping is
transparent.

- `input.pressed(action)` — true only the frame the action first went down. Use
  for one-shot actions (fire, jump, menu select).
- `input.held(action)` — true while the action is held. Use for movement,
  charging meters, "hold to skip" prompts.
- `input.released(action)` — true only the frame the action first went up. Use
  for charge-and-release mechanics (jump-on-release, slingshot pull-back).

| Action  | Keyboard        | Gamepad                                          |
| ------- | --------------- | ------------------------------------------------ |
| `LEFT`  | arrow left / A  | dpad left / left stick left                      |
| `RIGHT` | arrow right / D | dpad right / left stick right                    |
| `UP`    | arrow up / W    | dpad up / left stick up                          |
| `DOWN`  | arrow down / S  | dpad down / left stick down                      |
| `BTN1`  | Z / J           | south face (Xbox A, PS Cross), LB                |
| `BTN2`  | X / K           | east face (Xbox B, PS Circle), RB                |
| `BTN3`  | C / L           | north + west face (Xbox Y/X, PS Triangle/Square) |

`BTN1`/`BTN2`/`BTN3` are abstract action buttons. BTN3 binds both the north and
west face buttons because either is easier to reach than crossing the diamond
from BTN1's south position.

**Nintendo Switch face-button swap.** When a Switch pad is connected, BTN1 fires
from the A button (east face) and BTN2 from the B button (south face), matching
Nintendo's "A confirms, B cancels" convention. Triggers (L/R) and BTN3 are
unchanged. The swap is automatic via `GetGamepadName`; from your game's
perspective `input.pressed(input.BTN1)` still means "primary action."

`input.pressed` is edge-detected on keyboard and gamepad buttons but not on
analog sticks; track stick state in Lua if you need that.

#### Control glyphs (source-aware)

For UI prompts that adapt to the device the player is using:

- `input.mapping_for(action)`: string label of the active source's primary
  binding for `action` (e.g. `"Z"` on keyboard, `"A"` on Xbox, `"Cross"` on
  PlayStation, `"A"` on Switch since Nintendo swaps BTN1 to its A button).
  Gamepad family is auto-detected via `GetGamepadName`. Honors any keymap remap
  the player has set via the pause menu's Configure Keys flow. Returns `nil` if
  `action` is unknown or the active source has no binding for it (rare; only
  after exotic remaps).
- `input.last_source()`: string `"keyboard"` or `"gamepad"`, the source that
  most recently fired any bound action. Switches only when a _bound_ input
  fires, so menu keys (Esc/Enter) and idle activity don't flip it.
- `input.SOURCE_KEYBOARD`, `input.SOURCE_GAMEPAD`: the corresponding string
  constants for comparing against `last_source()`.

```lua
local btn = input.mapping_for(input.BTN1) or "?"
gfx.text("Press " .. btn .. " to jump", 10, 10, gfx.COLOR_WHITE)
```

#### Mouse

- `input.mouse()` — returns `x, y` for the cursor in game-space pixels (so the
  values line up with `gfx.*` coords regardless of window size or pixel-perfect
  scaling). When the cursor sits over the letterbox bars the values fall outside
  `0..usagi.GAME_W` / `0..usagi.GAME_H`, so a bounds check is the idiomatic way
  to detect "cursor is off the play area." See
  [`examples/mouse`](https://github.com/brettchalupa/usagi/blob/main/examples/mouse/main.lua).
- `input.mouse_held(button)` — true while `button` is held.
- `input.mouse_pressed(button)` — true the frame `button` first went down.
- `input.mouse_released(button)` — true the frame `button` first went up.
- `input.MOUSE_LEFT`, `input.MOUSE_RIGHT` — the supported buttons. Wheel
  scrolling and middle-click aren't exposed yet.
- `input.set_mouse_visible(visible)` — show or hide the OS cursor over the game
  window. Callable from `_init` to hide the cursor before the first frame draws
  (handy for games that render their own cursor sprite).
- `input.mouse_visible()` — true when the OS cursor is currently shown. Reflects
  the latest `set_mouse_visible` call synchronously, so toggling reads
  consistently: `input.set_mouse_visible(not input.mouse_visible())`.

#### Direct keyboard (escape hatch)

For dev hotkeys (toggling debug overlays, screenshotting, F-key shortcuts) and
for keyboard-and-mouse-only games, you can read raw keyboard state by key:

- `input.key_pressed(key)` — true the frame `key` first went down.
- `input.key_held(key)` — true while `key` is held.
- `input.key_released(key)` — true the frame `key` first went up.

```lua
if usagi.IS_DEV and input.key_pressed(input.KEY_F1) then
  State.show_debug = not State.show_debug
end
```

**Use sparingly for gameplay.** These bypass the action/keymap system on
purpose, meaning they don't honor the player's pause-menu key remaps and they
don't fire from a gamepad. Anything a player should be able to remap, or that a
controller player needs to reach, belongs on `input.held` / `input.pressed` /
`input.released` with an abstract action.

Available constants (all `input.KEY_*`): letters `A`–`Z`, digits `0`–`9`,
function keys `F1`–`F12`, `SPACE`, `ENTER`, `ESCAPE`, `TAB`, `BACKSPACE`,
`DELETE`, arrows (`LEFT`, `RIGHT`, `UP`, `DOWN`), modifiers (`LSHIFT`, `RSHIFT`,
`LCTRL`, `RCTRL`, `LALT`, `RALT`), and punctuation (`BACKTICK`, `MINUS`,
`EQUAL`, `LBRACKET`, `RBRACKET`, `BACKSLASH`, `SEMICOLON`, `APOSTROPHE`,
`COMMA`, `PERIOD`, `SLASH`). Numpad and the navigation cluster
(Insert/Home/End/PgUp/PgDn) aren't exposed yet.
[Open an issue](https://github.com/brettchalupa/usagi/issues/new) or submit a PR
if you need them.

Raw gamepad reads (analog sticks, triggers, individual face buttons by index)
are intentionally not exposed. The abstract `input.held(input.BTN1)` family
covers gamepad input; if you need finer-grained control than that, you've likely
outgrown Usagi.

### `sfx`

- `sfx.play(name)` — play `sfx/<name>.wav`. Unknown names silently no-op.
  Playing a sound while it's already playing restarts it.

### `music`

Background music streamed from disk (or the fused bundle). Only one track plays
at a time; calling `play` or `loop` while another is playing stops the old one
first.

- `music.play(name)` — play `music/<name>.<ext>` once and stop at the end.
- `music.loop(name)` — play and loop forever.
- `music.stop()` — stop whatever's playing. No-op if nothing is.

All three are callable from `_init`, so a title track can start the moment the
window opens (no one-frame gap waiting for `_update`).

Recognized extensions: `.ogg`, `.mp3`, `.wav`, `.flac`. **OGG is recommended for
music as they're small and cross-platform.**

The file stem is the name; `music/intro.ogg` is `music.play("intro")`. Music
lives in a separate directory from sfx because the formats and lifetimes differ
— sfx is loaded fully into memory and one-shotted, music is decoded
incrementally on the audio thread.

### `util`

Drop-in math and geometry helpers. Pure Lua, no engine state, available as a
global `util` table.

Functions taking shaped tables (vectors `{x, y}`, rects `{x, y, w, h}`, circles
`{x, y, r}`) check their args and raise an error pointing at _your_ call site
when a field is missing, so a typo like `util.rect_overlap({x=0, y=0, w=10})`
fails with `util.rect_overlap: arg 1 table missing or non-numeric field 'h'`
instead of a confusing nil-arithmetic explosion deep inside the helper.

**Scalar math:**

- `util.clamp(v, lo, hi)` — clamps `v` into `[lo, hi]`.
- `util.sign(v)` — returns `-1`, `0`, or `1`. Lua doesn't have this built-in.
- `util.round(v)` — half-up rounding to nearest integer. Pixel-snap world
  positions on draw to keep sprites crisp.
- `util.approach(current, target, max_delta)` — moves `current` toward `target`
  by at most `max_delta`. Pass a delta scaled by `dt` for frame-rate
  independence (`util.approach(p.vx, target, accel * dt)`).
- `util.lerp(a, b, t)` — linear interpolation; `t = 0` → `a`, `t = 1` → `b`,
  values outside `[0, 1]` extrapolate.
- `util.wrap(v, lo, hi)` — wraps `v` into `[lo, hi)`. Cycle-safe for negatives.
- `util.flash(t, hz)` — boolean from time, toggles `hz` times per second.

**Vectors:**

- `util.vec_normalize({x, y})` — returns a new unit-length vector. Zero in →
  zero out (no divide-by-zero).
- `util.vec_dist(a, b)` — distance between two `{x, y}` points.
- `util.vec_dist_sq(a, b)` — squared distance, for "is X closer than Y?" hot
  loops where you don't want the sqrt. Compare against `r * r`.
- `util.vec_from_angle(angle, len?)` — vector at `angle` (radians) with
  magnitude `len` (default 1). Pair with `math.atan(dy, dx)` to convert any
  direction into a velocity.

**Geometry overlap:**

- `util.point_in_rect(p, r)` — point-in-rect hit test. Half-open `[x, x+w)` on
  each axis: top/left edges are inside, bottom/right edges are outside.
- `util.point_in_circ(p, c)` — point-in-circle hit test. Boundary is outside
  (matches `circ_overlap` convention).
- `util.rect_overlap(a, b)` — AABB overlap. Edge-adjacent rects don't overlap.
- `util.circ_overlap(a, b)` — circle-vs-circle. Tangent circles don't overlap.
- `util.circ_rect_overlap(c, r)` — circle-vs-rect via closest-point method.

### `usagi`

Engine-level info.

- `usagi.GAME_W`, `usagi.GAME_H` — game render dimensions (320, 180).
- `usagi.SPRITE_SIZE` — side length, in pixels, of one cell in `sprites.png`
  (currently 16). Use it for tile-grid math instead of hardcoding 16:
  `gfx.spr(idx, col * usagi.SPRITE_SIZE, row * usagi.SPRITE_SIZE)`.
- `usagi.IS_DEV` — `true` when running under `usagi dev`; `false` under
  `usagi run` and inside exported binaries. Useful for gating debug overlays,
  dev menus, verbose logging:

  ```lua
  if usagi.IS_DEV then
    gfx.text("debug", 0, 0, gfx.COLOR_GREEN)
  end
  ```

- `usagi.elapsed` — wall-clock seconds since the session started, updated once
  per frame before `_update`. Frame-stable (every read in one frame returns the
  same value). Doesn't reset on F5; track your own counter from `_init` if you
  need a per-run timer.
- `usagi.measure_text(text)` — returns two values, `width, height` in pixels,
  for `text` rendered in the bundled font. Pure utility (no rendering); call it
  from `_init` to pre-compute layouts, or from `_update` / `_draw` for dynamic
  strings.

  ```lua
  local w, h = usagi.measure_text("Game Over")
  gfx.text("Game Over", (usagi.GAME_W - w) / 2, (usagi.GAME_H - h) / 2,
           gfx.COLOR_WHITE)
  ```

- `usagi.save(t)` — serialize a Lua table as JSON and persist it. Saves are
  per-game (namespaced by `game_id` in `_config()`) so games made with usagi
  don't clobber each other.
- `usagi.load()` — return the previously saved table, or `nil` on first run.

  ```lua
  function _config()
    return { title = "My Game", game_id = "com.you.mygame" }
  end

  function _init()
    State = usagi.load() or { score = 0, best = 0 }
  end

  function _update(dt)
    -- ... gameplay updates State.score, State.best ...
    usagi.save(State)  -- call whenever you want to persist
  end
  ```

  Save data is one JSON file. Nest your own structure inside it (settings,
  unlocks, run state). There are no slots at the engine level.

  Where saves live:
  - Linux: `~/.local/share/<game_id>/save.json`
  - macOS: `~/Library/Application Support/<game_id>/save.json`
  - Windows: `%APPDATA%\<game_id>\save.json`
  - Web: `localStorage`, key `usagi.save.<game_id>`

  `game_id` is a reverse-DNS string like `com.brettmakesgames.snake`. It's
  required for save / load but optional for games that never persist anything.

  Native writes are atomic (`save.json.tmp` + rename), so a crash mid-write
  leaves the previous save intact. JSON values must be representable: tables,
  strings, numbers, booleans, nil. Functions, userdata, NaN, and circular tables
  raise an error.

### Effects: hitstop, screen shake, flash, slow-mo

The `effect.*` module gives you four engine-level juice primitives. Each is a
single call from anywhere in `_init` / `_update` / `_draw`; the engine decays
them once per frame and threads them into the right point in the update / render
loop, so you don't have to plumb shake offsets through your draws or gate
`_update` on a freeze flag.

```lua
effect.hitstop(0.06)                     -- freeze _update for 60 ms
effect.screen_shake(0.3, 4)              -- shake 0.3 s, up to 4 game pixels
effect.flash(0.1, gfx.COLOR_WHITE)       -- white flash, fades over 100 ms
effect.slow_mo(1.5, 0.3)                 -- 1.5 s at 30% speed
```

- **`effect.hitstop(time)`** skips the call to `_update` for `time` seconds.
  `_draw` still runs so the world stays on screen.
- **`effect.screen_shake(time, intensity)`** offsets the RT-to-window blit.
  `intensity` is a max offset in _game pixels_ (try 2-6); the magnitude decays
  linearly to zero. Overlays drawn outside the world (the engine error overlay,
  the REC indicator) stay anchored.
- **`effect.flash(time, color)`** draws a full-screen overlay of palette `color`
  on top of `_draw`'s output. Alpha decays from opaque to transparent. White on
  hits, red on damage.
- **`effect.slow_mo(time, scale)`** multiplies the `dt` passed to `_update` by
  `scale`. `scale=0.5` is half-speed, `scale=2.0` is double-speed, `scale=0`
  freezes (use `effect.hitstop` for that intent). The slow_mo timer itself
  counts down at real wall-clock, so the cinematic always ends on schedule.

**Stacking.** Across all four, longer duration wins; for the magnitude
parameter, the latest call wins. `effect.screen_shake(0.1, 2)` followed by
`effect.screen_shake(0.5, 4)` gives 0.5 s at intensity 4. Spam-calling is safe.

**Pause.** When the engine pause overlay is open, effect timers don't tick and
shake is suppressed under the "PAUSED" view, so nothing decays or rattles while
the game is held.

See
[`examples/effect.lua`](https://github.com/brettchalupa/usagi/blob/main/examples/effect.lua)
for a runnable demo (one key per primitive plus a combo button).

### Shaders (advanced, experimental)

Post-process GLSL fragment shaders run as the final pass when the game's render
target is blitted to the window. Use them for CRT effects, palette swaps,
vignettes, color grading, and so on.

**Status:** experimental. The API surface and dual-file convention may change.
Captures have a known limitation (see below).

API:

- `gfx.shader_set("name")`: activate `shaders/<name>.fs` (and an optional
  `shaders/<name>.vs`).
- `gfx.shader_set(nil)`: clear the active shader.
- `gfx.shader_uniform("u_name", v)`: queue a uniform write. `v` may be a number
  (float) or a 2/3/4-length numeric table (vec2/vec3/vec4). Call this every
  frame inside `_update` or `_draw` for animated values.

```lua
function _init() gfx.shader_set("crt") end

function _draw(_dt)
  gfx.shader_uniform("u_time", usagi.elapsed)
  gfx.shader_uniform("u_resolution", { usagi.GAME_W, usagi.GAME_H })
  -- ... your normal gfx.* calls ...
end
```

**Cross-platform shader files.** Desktop targets compile GLSL `#version 330`;
the web target uses GLSL ES `#version 100` (WebGL 1 / GLES 2). Ship two files
alongside each other to support both:

- `shaders/<name>.fs`: desktop, `#version 330`, `in`/`out`, `texture(...)`,
  custom `out vec4 finalColor`.
- `shaders/<name>_es.fs`: web, `#version 100`, `precision mediump float;`,
  `varying`, `texture2D(...)`, `gl_FragColor` output.

Web prefers `_es.fs` and falls back to `.fs`; desktop is the reverse. If only
one is shipped, every platform that loads it runs that one. The `fragTexCoord`,
`fragColor`, and `texture0` inputs are provided by raylib on both targets. See
`examples/shader/` for a runnable CRT effect plus a Game Boy palette swap with
both variants of each.

**Live reload.** Saving the active shader's `.fs` or `.vs` file rebuilds it
in-place. Cached uniforms are replayed onto the new shader. Compile errors print
to the terminal and keep the previous shader live.

**Bundling.** `usagi export` walks `shaders/` and ships every `.fs` / `.vs` in
the bundle, so shaders work the same in `usagi dev`, `usagi run`, `.usagi`
files, and fused exes on every platform.

**Captures don't include the shader.** F8 / Cmd+F screenshots and F9 / Cmd+G GIF
recording read the unshaded game render target, so post-process effects show up
on screen but not in the saved file. Tradeoff: the shader runs at window
resolution (CRT scanlines look smooth, not blocky) and captures stay at the
game's 320x180 grid for clean shareable artifacts. If you need the shader baked
into a capture, use your OS's screen recorder or screenshot tool against the
game window.

Shaders resources:

- [Raylib shaders demo](https://www.raylib.com/examples/shaders/loader.html?name=shaders_postprocessing)
- [Raylib shaders source](https://github.com/raysan5/raylib/blob/master/examples/shaders/shaders_postprocessing.c)

### Indexing

Sequence-style APIs (`gfx.spr`, and any future sound/tile indexing) are
**1-based** to match Lua conventions (`ipairs`, `t[1]`, `string.sub`).
`gfx.spr(1, ...)` draws the top-left sprite.

Enum-like constants (palette colors, key codes) keep their conventional
numbering. `gfx.COLOR_RED` is 8 because that's its Pico-8 number, not because
it's the 9th color.

### Randomness

Lua's `math.random` is available as-is. Lua auto-seeds its PRNG at startup, so
each run of `usagi dev` / `usagi run` (and each launch of an exported binary)
produces a fresh sequence. No engine call is needed before calling
`math.random()`.

```lua
local n = math.random(1, 100)   -- integer in [1, 100]
local f = math.random()         -- float in [0, 1)
```

If you want a deterministic sequence (replays, tests, repeatable level
generation) call stock Lua's `math.randomseed(n)` from `_init`. See
[`examples/rng.lua`](https://github.com/brettchalupa/usagi/blob/main/examples/rng.lua)
for a small demo.

### Coming from Pico-8?

Check out
[`./examples/pico8`](https://github.com/brettchalupa/usagi/tree/main/examples/pico8)
to see how you can drop in a `pico8.lua`, `require "pico8"`, and have a lot of
the same functions as Pico-8.

The Pico-8 shim allows you to write code like in Pico-8:

```lua
-- check for input
if btn(0) then
  State.p.x = State.p.x - State.p.spd * dt
end

-- draw a sprite from sprites.png
spr(0, 20, 30)
```

## Live Reload

Usagi watches the running script file and re-executes it when you save. The new
`_update` and `_draw` take effect on the next frame — your current game state is
**preserved** across the reload so you can tweak logic mid-play without losing
progress.

- `_init()` is **not** called on a save-triggered reload.
- Press **F5** (or **Ctrl+R** / **Cmd+R**) for a hard reset: Usagi runs
  `_init()` to reinitialize state.
- Press **~** (grave/tilde) to toggle the FPS overlay. Hidden by default in
  `dev`.
- Press **Alt+Enter** to toggle borderless fullscreen. Persists in
  `settings.json` and applies before the first frame on the next launch. No Lua
  or `_config` surface by design; the player owns this setting.
- Press **Esc**, **P**, or gamepad **Start** to pause. The same keys (plus
  **BTN2**) close the menu. While paused, `_update` and `_draw` are skipped and
  the screen shows a black "PAUSED" overlay; music keeps streaming.
- Press **Shift+Esc** in dev mode to quit the game
- Press **F9** or **Cmd/Ctrl + G** to start recording a GIF. Press the same key
  again to stop and save. Files land in `<cwd>/captures/` named
  `<game>-YYYYMMDD-HHMMSS.gif`, where `<game>` is the short form of your
  `_config().game_id` (e.g. `snake-20260101-120000.gif`). Upscaled 2x (640×360)
  so they read well when embedded online. A small pulsing red "● REC" indicator
  shows in the top-right while recording.
- Press **F8** or **Cmd/Ctrl + F** to save a PNG screenshot to the same
  `<cwd>/captures/` bucket. Same 2x upscale as the gif recorder, lossless,
  palette-exact.
- Press **Shift+M** to toggle audio mute. Master volume flips between `0.0` and
  the value in `settings.json` (defaults to `0.5`). Settings live in the same
  per-game OS data dir as `save.json`; on web they're routed through
  `localStorage` under `usagi.settings.<game_id>`.

### Writing Reload-Friendly Scripts

The chunk re-executes on save, so any top-level `local` bindings get re-bound
each time. A `local State` at module scope would get reset to a fresh table on
every save and obliterate the running game; it has to be a global. The pattern:

- **Mutable game state** → a single capitalized global, conventionally `State`,
  assigned only inside `_init`. `_init` runs once at startup and on F5, so the
  table outlives reloads. Saved edits keep your in-progress game intact.
- **Constants** → file-scope `local`. Re-binding to the same value each reload
  is harmless.
- **Required modules** → either file-scope `local Foo = require("foo")`, or a
  capitalized global `Foo = require("foo")` if you want `Foo` reachable from
  every file without re-requiring. Both work; the global form is convenient for
  engine-wide tables like `Player`, `Enemy`.

The shipped `.luarc.json` enables the `lowercase-global` diagnostic to catch the
most common footgun: forgetting `local` and accidentally creating a global named
`score`, `timer`, etc. Capitalize anything you actually mean to make global;
lowercase top-level assignments will warn.

See
[`examples/hello_usagi.lua`](https://github.com/brettchalupa/usagi/blob/main/examples/hello_usagi.lua)
and
[`examples/input.lua`](https://github.com/brettchalupa/usagi/blob/main/examples/input.lua)
for the layout.

## Examples

[View the examples on GitHub.](https://github.com/brettchalupa/usagi/tree/main/examples)

There are a variety of examples exercising the full Usagi API that you can
browse and adapt. Their source is all public domain, so do with them what you
want.

## Tools

`usagi tools [path]` opens a 1280×720 window with a tab bar for the available
tools. The path is optional; pass a project directory (or a `.lua` file) to load
its `sprites.png` and `sfx/` assets. Without a path the tools open with empty
state.

Switch tools via the tab buttons or with **1** (Jukebox), **2** (TilePicker), or
**3** (SaveInspector).

Jukebox and TilePicker live-reload their assets: drop a new WAV in `sfx/` or
save a new `sprites.png` and the tools pick it up on the next frame.

### Jukebox

Lists every `.wav` in `<project>/sfx/` and lets you audition them. Selected
sounds play automatically on selection change (Pico-8 SFX editor style), so you
can just arrow through the list to hear each one.

- **up** / **down** or **W** / **S** to select.
- **space** or **enter** to replay the current selection.
- Click a name to select + play.
- Click the **Play** button in the right pane to replay.

### TilePicker

Shows `<project>/sprites.png` with a 1-based grid overlay matching `gfx.spr`.
Click any tile to copy its index to the clipboard (paste it straight into your
Lua code).

- **WASD** to pan. **Q** / **E** to zoom out / in (0.5×–20×). **0** resets the
  view.
- **R** toggles the grid and index overlay.
- **B** cycles the viewport background color (gray / black / white) so tiles
  stay visible regardless of palette.
- Left click a tile to copy its 1-based index; a toast confirms the value.

### SaveInspector

Reads the project's `_config().game_id` and shows the current `save.json`
contents alongside the resolved file path. Useful for debugging save formats and
inspecting state between runs without leaving the editor.

- Rendered as written since engine output is already formatted, so no
  reformatting happens here.
- **R** or the **Refresh** button rereads the file from disk; the inspector
  doesn't auto-poll, so hit refresh after the running game has saved.
- **Clear** deletes the save file. The next `usagi.load()` returns `nil`.
- **Open in File Manager** reveals the containing directory in the OS default
  file manager (`xdg-open` on Linux, `open` on macOS, `explorer` on Windows).

### ColorPalette

Shows swatches for each of the 16 colors with the ability to click to copy the
Lua value to your clipboard.

### Bring Your Own Tools

Usagi does not (at least yet) include a sprite editor, sound effect generator,
or music tracker. You can find assets to use on
[opengameart.org](https://opengameart.org/) and [itch.io](https://itch.io) or
make your own. Here are some tools worth checking out that work well with Usagi:

- **Sprite Editors**:
  - [Aseprite](https://www.aseprite.org/): an excellent pixel art editor
  - [Piskel](https://www.piskelapp.com/): free, online sprite editor
- **Sound**:
  - [jsfxr](https://sfxr.me/): 8-bit sound effect generator; download WAVs
  - [1BITDRAGON](https://1bitdragon.com/): an easy-to-use music creation tool
- **Map Editors**:
  - [Tiled](https://www.mapeditor.org/): free and open source map editor with
    Lua export

## Export

`usagi export <path>` packages a game for distribution. Default output is every
platform plus a portable bundle:

```
$ usagi export examples/snake
$ tree export
export
├── snake-linux.zip      # Linux x86_64 fused exe
├── snake-macos.zip      # macOS arm64 fused exe
├── snake-windows.zip    # Windows x86_64 fused exe
├── snake-web.zip        # web export: index.html + usagi.{js,wasm} + game.usagi
└── snake.usagi          # portable bundle (usagi run snake.usagi)
```

Or pick one with `--target`:

```
$ usagi export examples/snake --target web
$ usagi export examples/snake --target windows
$ usagi export examples/snake --target bundle
```

### Cross-platform Templates

Non-host platforms come from "runtime templates" published alongside each
release. The CLI fetches them on first use, caches them per-OS, and verifies
each archive against its `sha256` sidecar before extracting.

- **Cache**: Linux `~/.cache/usagi/templates/`, macOS
  `~/Library/Caches/com.usagiengine.usagi/templates/`, Windows
  `%LOCALAPPDATA%\usagiengine\usagi\cache\templates\`.
- **Inspect / wipe**: `usagi templates list`, `usagi templates clear`.
- **Force re-download**: `--no-cache`.
- **Mirror or fork**: set `USAGI_TEMPLATE_BASE` to override the default GitHub
  Releases base URL.

The host platform always works offline. Linux x86_64 running
`usagi export --target linux` (or the linux slice of `--target all`) fuses
against the running binary directly: no cache lookup, no network. First-time
cross-platform export needs network; subsequent runs are offline.

Override the template source explicitly:

- `--template-path PATH/TO/usagi-<ver>-<os>.{tar.gz|zip}` to point at a local
  archive. Skips verification and the cache.
- `--template-url https://example.com/usagi-...` to fetch from an arbitrary URL.
  Verification still runs (the URL must have a sibling `.sha256`).

### Web Shell

The web export ships a default HTML page that hosts the canvas. To use a custom
page, drop a `shell.html` next to your `main.lua` and `usagi export` picks it up
automatically. Override per-build with `--web-shell PATH`.

### Notes

- Native zips contain a single fused executable named after the project (the
  windows zip names it `<name>.exe`). The web zip is unzip-and-serve.
- `<name>` is the project directory name (or the script's stem for flat `.lua`
  files). `-o <path>` overrides the output location.
- Live-reload is disabled in exported artifacts; F5 still resets state via
  `_init()`.
- The fuse format is simple and additive: a magic footer at the end of the exe
  points back to an appended bundle. A `.usagi` file is the same bundle bytes
  without the footer; it runs on any platform via `usagi run`.

## Developing

- `just run` - run hello_usagi example
- `just ok` - run all checks
- `just fmt` - format Rust code
- `just serve-web` - build and serve the web build at <http://localhost:3535>
  (requires `emcc` on PATH; see [docs/web-build.md](docs/web-build.md))

## Reference and Inspiration

- Pico-8
- Pyxel
- Love2D
- Playdate SDK
- DragonRuby Game Toolkit (DRGTK)

## Credits

Usagi is built with [Rust](https://rust-lang.org/) and
[sola-raylib](https://crates.io/crates/sola-raylib).

- **monogram** — the bundled font (`assets/monogram.ttf`) used by `gfx.text`,
  the FPS overlay, the error overlay, and the tools window. A 5×7 pixel font by
  [datagoblin](https://datagoblin.itch.io/monogram), released under Creative
  Commons Zero (CC0). No attribution required, but kindly given.

## (Un)license

Usagi's source code is dedicated to the public domain. You can see the full
details in [UNLICENSE](./UNLICENSE).
