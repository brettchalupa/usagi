# Usagi - Rapid 2D Game Prototyping Tool

Usagi is a tool for quickly prototyping simple games with Lua. It features
live-reloading as your change your game code. Usagi is built with Rust. Its API
is inspired by Pico-8.

Uses Lua 5.4.

## Project layout

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

- `usagi dev path/to/my_game` for live-reload development (script, sprites, and
  sfx reload on save; F5 resets state).
- `usagi run path/to/my_game` to run without live-reload.
- `usagi tools [path]` opens the Usagi tools window (jukebox, tile picker). See
  the **Tools** section below.
- `usagi compile path/to/my_game` produces a standalone executable for the host
  platform with the game's script, sprites, and sfx fused into the binary. No
  Usagi install needed to run the output. See the **Compile** section below.

While developing Usagi itself, replace `usagi` with `cargo run --` (for example
`cargo run -- dev examples/hello_usagi.lua`).

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
- `gfx.rect(x, y, w, h, color)` — filled rectangle.
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
  `usagi
  run` and inside compiled binaries. Useful for gating debug overlays,
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

## Live reload

Usagi watches the running script file and re-executes it when you save. The new
`_update` and `_draw` take effect on the next frame — your current game state is
**preserved** across the reload so you can tweak logic mid-play without losing
progress.

- `_init()` is **not** called on a save-triggered reload.
- Press **F5** for a hard reset: Usagi runs `_init()` to reinitialize state.
- Press **~** (grave/tilde) to toggle the FPS overlay. On by default in `dev`,
  off in `run`.
- Press **Alt+Enter** to toggle borderless fullscreen.

### Writing reload-friendly scripts

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

## Compile

`usagi compile <path>` packages a game for distribution. By default it produces
all artifacts in one export directory:

```
$ usagi compile examples/snake
[usagi] compiled snake-export/snake (3 file(s), 37125 bytes bundled)
[usagi] wrote snake-export/snake.usagi (3 file(s), 37125 bytes)
[usagi] wrote snake-export/web/ (3 game file(s), 37125 bundle bytes; runtime from embedded)
[usagi] export ready at snake-export/

$ tree snake-export
snake-export
├── snake             # native fused executable (./snake to run)
├── snake.usagi       # portable bundle (usagi run snake.usagi)
└── web/              # zip and upload to itch.io
    ├── index.html
    ├── usagi.{js,wasm}
    └── game.usagi
```

Or pick one with `--target {all,exe,bundle,web}`:

```
$ usagi compile examples/snake --target bundle
$ usagi compile examples/snake --target web
$ usagi compile examples/snake --target exe
```

Notes:

- Output defaults: `./<name>-export/` for `all`, `./<name>` for `exe`,
  `./<name>.usagi` for `bundle`, `./<name>-web/` for `web`. `<name>` is the
  project directory name (or the script's stem for flat `.lua` files).
  `-o <path>` overrides.
- A fused exe is cross-platform only insofar as the Usagi binary that produced
  it is: a Linux `usagi` produces a Linux executable, a Windows one produces a
  `.exe`, etc. `.usagi` bundles are platform-agnostic.
- `--target web` (and the `web/` slice of `--target all`) needs the wasm
  runtime. **Release builds of `usagi` embed it at compile time**, so an
  installed `usagi` Just Works. Debug builds (`cargo run`) fall back to reading
  the runtime from `target/web/`. See [docs/web-build.md](docs/web-build.md) for
  the build dance.
- Live-reload is disabled in compiled artifacts; F5 still resets state via
  `_init()`.
- The fuse format is simple and additive: a magic footer at the end of the exe
  points back to an appended bundle. A `.usagi` file is the same bundle bytes
  without the footer.

## Web build

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
