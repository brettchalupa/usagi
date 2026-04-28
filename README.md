# Usagi - Simple 2D Game Engine for Rapid Prototyping

Usagi is a simple 2D game engine for quickly making games with **Lua** 5.4. It
features live-reloading as you change your game code and assets. Its API is
clear, consistent, and familiar.

[Watch the intro video!](https://www.youtube.com/watch?v=byp3rKd626M)

Usagi is built with [Rust](https://rust-lang.org/) and
[sola-raylib](https://crates.io/crates/sola-raylib).

**WARNING:** Usagi is very early in development and not stable. APIs and
commands will change.

Usagi is made by [Brett Chalupa](https://brettmakesgames.com) and dedicated to
the public domain.

## Install

[Download the latest Usagi build for your operating
system.](https://github.com/brettchalupa/usagi/releases/latest)

**Latest Usagi release:** v0.1.0

_More ways of installing Usagi may be added in the future._

### Upgrading

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

`init` writes `main.lua` (with stubbed `_init` / `_update` / `_draw`),
`.luarc.json` for Lua LSP support, `.gitignore`, `meta/usagi.lua` (API type
stubs), and `USAGI.md` (a copy of these docs).

Edit `main.lua` and save. The Usagi runtime automatically reloads, so your
changes show up live without losing game state.

In most traditional game development environments, you would need to restart
your game's executable after making changes. Usagi lets you focus on coding and
making art without losing the current game state, allowing for much faster
iteration cycles.

Need to revise a sprite quickly? Just open it in Aseprite, tweak it, save it,
and see it update in the context of your game.

## Project Goal

Usagi does not aim to be anything more than a rapid development engine for
simple, lower res 2D games. It doesn't intend to support mobile platforms or
touch or VR. It doesn't aim to replace Love2D or Pico-8 or Picotron. It's not a
fantasy console. It's a command-line program and suite of tools to help you make
games quickly.

Usagi is great for those learning game programming. And for those who to use
something more flexible than Pico-8/Picotron but more constrained than Love2D.

**Why Lua:** Lua is a widely-used language in game programming, and it's quite
simple yet surprisingly powerful, making it a good fit for Usagi.

If you want to build a medium-to-large polished game, Usagi would not be a good
fit.

## Project Layout

A Usagi game is either a single `.lua` file or a directory with a `main.lua` in
it. Optional assets live alongside:

```
my_game/
  main.lua        -- required: your game
  sprites.png     -- optional: 16×16 sprite sheet (PNG with alpha)
  sfx/            -- optional: .wav files, file stems become sfx names
    jump.wav
    coin.wav
```

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

While developing Usagi itself, replace `usagi` with `cargo run --` (for example
`cargo run -- dev examples/hello_usagi.lua`).

## Constraints

Usagi embraces a few constraints inspired by Pico-8 and Pyxel to help focus on
prototyping rather than making polished high-resolution graphics. These may
change in the future or be configurable.

- **Resolution**: 320px by 180px - 16:9 aspect ratio that scales nicely to
  common monitor sizes
- **One Lua File**: there is no way to import other Lua files into your game
  (yet), only `main.lua` or whatever you named your file is supported
- **One Spritesheet**: `sprites.png` is the only image file for textures that
  can be loaded
- **Sprite Size**: 16px by 16px - using `gfx.spr` uses the index based on this
  sized sprite
- **Limited Colors**: the color palette for drawing are the same as Pico-8 (but
  with constants for easy reference)

You currently must bring your own sound effects and sprite editor. A sprite
editor could be nice in the future as part of the `usagi tools`.

## TODO - What's Missing

Here's what Usagi will support as it heads towards 1.0 release:

- Music playback with looping support
- Mouse functions and ability to hide cursor

## Lua API

### Callbacks

Define any of these as globals; Usagi calls them:

- `_config()` — optional. Called **once at startup, before the window opens**;
  returns a config table. Currently supports `title` (defaults to "Usagi") and
  `pixel_perfect` (defaults to `true`, set `false` to stretch the game to fill
  the window instead of integer-scaling with bars).
- `_init()` — once at start, and when the user presses **F5**. Put state setup
  here.
- `_update(dt)` — each frame, before draw. `dt` is seconds since last frame.
- `_draw(dt)` — each frame, after update. `dt` same as above.

```lua
function _config()
  return { title = "Snake", pixel_perfect = true }
end
```

`_config()` runs before the runtime is fully alive (the window doesn't exist
yet), so its return value is **read once at startup and cached**. Editing
`_config()` while the game is running won't update the title or any future
config field on save; restart the session to pick up changes.

### `gfx`

Drawing. Positions are in game-space pixels (320×180). Colors are palette
indices 0-15; use the named constants.

- `gfx.clear(color)` — fill the screen.
- `gfx.rect(x, y, w, h, color)` — rectangle outline.
- `gfx.rect_fill(x, y, w, h, color)` — filled rectangle.
- `gfx.circ(x, y, r, color)` — circle outline centered at `(x, y)`.
- `gfx.circ_fill(x, y, r, color)` — filled circle centered at `(x, y)`.
- `gfx.line(x1, y1, x2, y2, color)` — line from `(x1, y1)` to `(x2, y2)`.
- `gfx.text(text, x, y, color)` — default font, 8px tall.
- `gfx.spr(index, x, y)` — draw the 16×16 sprite at `index` (1 = top-left) from
  `sprites.png`.
- `gfx.COLOR_BLACK`, `COLOR_DARK_BLUE`, `COLOR_DARK_PURPLE`, `COLOR_DARK_GREEN`,
  `COLOR_BROWN`, `COLOR_DARK_GRAY`, `COLOR_LIGHT_GRAY`, `COLOR_WHITE`,
  `COLOR_RED`, `COLOR_ORANGE`, `COLOR_YELLOW`, `COLOR_GREEN`, `COLOR_BLUE`,
  `COLOR_INDIGO`, `COLOR_PINK`, `COLOR_PEACH` — the Pico-8 palette, indices
  0-15.

### `input`

Abstract input actions. Each action is a union over keyboard, gamepad buttons,
and the left analog stick; the first connected gamepad is used.

- `input.pressed(action)` — true only the frame the action first went down. Use
  for one-shot actions (fire, jump, menu select).
- `input.down(action)` — true while the action is held. Use for movement.

| Action    | Keyboard        | Gamepad                                          |
| --------- | --------------- | ------------------------------------------------ |
| `LEFT`    | arrow left / A  | dpad left / left stick left                      |
| `RIGHT`   | arrow right / D | dpad right / left stick right                    |
| `UP`      | arrow up / W    | dpad up / left stick up                          |
| `DOWN`    | arrow down / S  | dpad down / left stick down                      |
| `CONFIRM` | Z / J           | south + west face (Xbox A/X, PS Cross/Square)    |
| `CANCEL`  | X / K           | east + north face (Xbox B/Y, PS Circle/Triangle) |

`input.pressed` is edge-detected on keyboard and gamepad buttons but not on
analog sticks; track stick state in Lua if you need that.

### `sfx`

- `sfx.play(name)` — play `sfx/<name>.wav`. Unknown names silently no-op.
  Playing a sound while it's already playing restarts it.

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

### Indexing

Sequence-style APIs (`gfx.spr`, and any future sound/tile indexing) are
**1-based** to match Lua conventions (`ipairs`, `t[1]`, `string.sub`).
`gfx.spr(1, ...)` draws the top-left sprite.

Enum-like constants (palette colors, key codes) keep their conventional
numbering. `gfx.COLOR_RED` is 8 because that's its Pico-8 number, not because
it's the 9th color.

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

### Writing Reload-Friendly Scripts

The chunk re-executes on save, so any top-level `local` bindings get fresh `nil`
values each time — callbacks that captured them as upvalues will see `nil` and
crash. The pattern:

- **Mutable state** → globals, assigned only in `_init`.
- **Constants and module aliases** → file-scope `local`.

See `examples/hello_usagi.lua` and `examples/input.lua` for the layout.

## Tools

`usagi tools [path]` opens a 1280×720 window with a tab bar for the available
tools. The path is optional; pass a project directory (or a `.lua` file) to load
its `sprites.png` and `sfx/` assets. Without a path the tools open with empty
state.

Switch tools via the tab buttons or with **1** (Jukebox) / **2** (TilePicker).

Both tools live-reload their assets: drop a new WAV in `sfx/` or save a new
`sprites.png` and the tools pick it up on the next frame.

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
page, drop a `shell.html` next to your script and `usagi export` picks it up
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

## Web Builds

Usagi compiles to wasm via emscripten so games can run in a browser. See
[docs/web-build.md](docs/web-build.md) for setup, the build/dev loop, debugging
tips, and the (non-obvious) wasm exception ABI requirements.

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

## (Un)license

Usagi's source code is dedicated to the public domain. You can see the full
details in [UNLICENSE](./UNLICENSE).
