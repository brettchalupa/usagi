# Usagi - Rapid 2D Game Prototyping Tool

Usagi is a tool for quickly prototyping simple games with Lua. It features
live-reloading as your change your game code. Usagi is built with Rust. Its API
is inspired by Pico-8.

Uses Lua 5.4.

## Live reload

Usagi watches the running script file and re-executes it when you save. The new
`_update` and `_draw` take effect on the next frame — your current game state is
**preserved** across the reload so you can tweak logic mid-play without losing
progress.

- `_init()` is **not** called on a save-triggered reload.
- Press **F5** for a hard reset: Usagi runs `_init()` to reinitialize state.

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
