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
- `usagi tools` will open the Usagi tools (jukebox, tile picker). Not yet
  implemented.

While developing Usagi itself, replace `usagi` with `cargo run --` (for example
`cargo run -- dev examples/hello_usagi.lua`).

## Lua API

### Callbacks

Define any of these as globals; Usagi calls them:

- `_init()` — once at start, and when the user presses **F5**. Put state setup
  here.
- `_update(dt)` — each frame, before draw. `dt` is seconds since last frame.
- `_draw(dt)` — each frame, after update. `dt` same as above.

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

Keyboard. Key codes are the `input.LEFT`/etc. constants; don't pass raw
keycodes.

- `input.pressed(key)` — true only the frame the key first went down. Use for
  one-shot actions (fire, jump, menu select).
- `input.down(key)` — true while the key is held. Use for movement.
- `input.LEFT`, `RIGHT`, `UP`, `DOWN`, `A` (Z key), `B` (X key).

### `sfx`

- `sfx.play(name)` — play `sfx/<name>.wav`. Unknown names silently no-op.
  Playing a sound while it's already playing restarts it.

### `usagi`

Engine-level info.

- `usagi.GAME_W`, `usagi.GAME_H` — game render dimensions (320, 180).

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

## Developing

- `just run` - run hello_usagi example
- `just ok` - run all checks
- `just fmt` - format Rust code

## Reference and Inspiration

- Pico-8
- Pyxel
- Love2D
- Playdate SDK
- DragonRuby Game Toolkit (DRGTK)
