# Usagi - Simple 2D Game Engine for Rapid Prototyping

Usagi is a simple 2D game engine for quickly making games with **Lua** 5.4. It
features live-reloading as you change your game code and assets. Its API is
clear, consistent, and familiar.

[Watch the intro video!](https://www.youtube.com/watch?v=byp3rKd626M)

**WARNING:** Usagi is very early in development and not stable. APIs and
commands will change.

Usagi is made by [Brett Chalupa](https://brettmakesgames.com) and dedicated to
the public domain.

[There's a chill Discord if you want to chat about the engine, share what you
make, and get help.](https://discord.gg/a92ZjE4NUx)

## Install

[Download the latest Usagi build for your operating
system.](https://github.com/brettchalupa/usagi/releases/latest)

**Latest Usagi release:** v0.2.0

You can keep the `usagi` executable in your project folder or install it
globally on your computer.

_More ways of installing Usagi may be added in the future._

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

## Upgrading Usagi

Replace the `usagi` binary with a newer release. **NOTE:** Usagi is pre-v1.0,
meaning there's no guarantee of API compatibility between releases right now.

To refresh engine-owned files in a project (the LSP type stubs and the embedded
docs), delete them and re-run `usagi init` from the project root:

```sh
rm meta/usagi.lua USAGI.md
usagi init .
```

`init` skips files that already exist, so your `main.lua`, `.luarc.json`, and
`.gitignore` stay untouched. The version stamp at the top of `meta/usagi.lua`
and `USAGI.md` tells you which `usagi` produced them.

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

## Constraints

Usagi embraces a few constraints inspired by Pico-8 and Pyxel to help focus on
prototyping rather than making polished high-resolution graphics. These may
change in the future or be configurable.

- **Resolution**: 320px by 180px - 16:9 aspect ratio that scales nicely to
  common monitor sizes
- **One Spritesheet**: `sprites.png` is the only image file for textures that
  can be loaded
- **Sprite Size**: 16px by 16px - using `gfx.spr` uses the index based on this
  sized sprite; you can draw larger sprites with `gfx.sspr`
- **Limited Colors**: the color palette for drawing are the same as Pico-8 (but
  with constants for easy reference)

You currently must bring your own sound effects and sprite editor. A sprite
editor could be nice in the future as part of the `usagi tools`.

## Roadmap

Here's what Usagi will support as it heads towards 1.0 release:

- Mouse functions and ability to hide cursor
- macOS `.app` bundles in export

## Lua API

**Philosophy:** keep it simple, name things clearly, and prefer fixed function
signatures.

**Style**: for Lua, 2 spaces indent with snake_case is used throughout.

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
state.score += 1
state.timer += dt
```

Limitations: the rewrite is line-anchored, so `if cond then x += 1 end` is left
as-is (use longhand). The LHS is duplicated verbatim, so `t[f()] += 1` calls
`f()` twice — same gotcha as PICO-8's preprocessor.

The shipped `.luarc.json` declares these as nonstandard symbols so the
lua-language-server stops underlining them as syntax errors.

### Callbacks

Define any of these as globals for Usagi to call them:

- `_init()` — once at start, and when the user presses **F5**. Put state setup
  here.
- `_update(dt)` — each frame, before draw. `dt` is seconds since last frame.
- `_draw(dt)` — each frame, after update. `dt` same as above.
- `_config()` — optional. Called **once at startup, before the window opens**;
  must return a config table.

#### `_config`

Currently supports `title` (defaults to "Usagi") and `pixel_perfect` (defaults
to `false`). When `true`, the game renders at integer scale multiples only (1×,
2×, 3×, ...) with black letterbox bars filling any leftover window space. When
`false`, the game scales at any factor that fits the window while preserving the
game's aspect ratio, so bars only appear on the axis with extra room, never
distorting the image. The default is `false` because at common fullscreen
resolutions (720p, 1080p, 4K) the game's 320×180 native size lands on an integer
multiple anyway, and in windowed mode it looks good still.

```lua
function _config()
  return { title = "Snake", pixel_perfect = true, game_id = "com.example.snake" }
end
```

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
- `input.down(action)` — true while the action is held. Use for movement.

| Action  | Keyboard        | Gamepad                                          |
| ------- | --------------- | ------------------------------------------------ |
| `LEFT`  | arrow left / A  | dpad left / left stick left                      |
| `RIGHT` | arrow right / D | dpad right / left stick right                    |
| `UP`    | arrow up / W    | dpad up / left stick up                          |
| `DOWN`  | arrow down / S  | dpad down / left stick down                      |
| `BTN1`  | Z / J           | south face (Xbox A, PS Cross)                    |
| `BTN2`  | X / K           | east face (Xbox B, PS Circle)                    |
| `BTN3`  | C / L           | north + west face (Xbox Y/X, PS Triangle/Square) |

`BTN1`/`BTN2`/`BTN3` are abstract action buttons. BTN3 binds both the north and
west face buttons because either is easier to reach than crossing the diamond
from BTN1's south position.

`input.pressed` is edge-detected on keyboard and gamepad buttons but not on
analog sticks; track stick state in Lua if you need that.

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

Recognized extensions: `.ogg`, `.mp3`, `.wav`, `.flac`. **OGG is recommended for
music as they're small and cross-platform.**

The file stem is the name; `music/intro.ogg` is `music.play("intro")`. Music
lives in a separate directory from sfx because the formats and lifetimes differ
— sfx is loaded fully into memory and one-shotted, music is decoded
incrementally on the audio thread.

### `usagi`

Engine-level info.

- `usagi.GAME_W`, `usagi.GAME_H` — game render dimensions (320, 180).
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
    state = usagi.load() or { score = 0, best = 0 }
  end

  function _update(dt)
    -- ... gameplay updates state.score, state.best ...
    usagi.save(state)  -- call whenever you want to persist
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
  state.p.x = state.p.x - state.p.spd * dt
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
- Press **Alt+Enter** to toggle borderless fullscreen.
- Press **Esc**, **P**, or gamepad **Start** to pause. The same keys (plus
  **BTN2**) close the menu. While paused, `_update` and `_draw` are skipped and
  the screen shows a black "PAUSED" overlay; music keeps streaming.
- Press **Shift+Esc** in dev mode to quit the game

### Writing Reload-Friendly Scripts

The chunk re-executes on save, so any top-level `local` bindings get fresh `nil`
values each time — callbacks that captured them as upvalues will see `nil` and
crash. The pattern:

- **Mutable state** → globals, assigned only in `_init`.
- **Constants and module aliases** → file-scope `local`.

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

### Tile Picker

Shows `<project>/sprites.png` with a 1-based grid overlay matching `gfx.spr`.
Click any tile to copy its index to the clipboard (paste it straight into your
Lua code).

- **WASD** to pan. **Q** / **E** to zoom out / in (0.5×–20×). **0** resets the
  view.
- **R** toggles the grid and index overlay.
- **B** cycles the viewport background color (gray / black / white) so tiles
  stay visible regardless of palette.
- Left click a tile to copy its 1-based index; a toast confirms the value.

### Save Inspector

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
