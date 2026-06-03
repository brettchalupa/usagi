# `usagi loveify` Shim

Canonical files used by `usagi loveify`.

This directory exists for two reasons:

1. `usagi_shim.lua` and `conf.lua` here are the **source of truth**. The
   `usagi loveify` CLI embeds them via `include_str!` at build time, so edits
   here ship in the next `cargo build`.
2. The README you're reading documents what's in the shim, what's not, and how
   to use it.

## What the two canonical files do

- `usagi_shim.lua`: pure Lua, ~1800 lines. Reimplements Usagi's runtime API
  (`gfx.*`, `input.*`, `sfx.*`, `music.*`, `usagi.*`, `util.*`, `effect.*`) plus
  the `font.png` / `palette.png` / `sprites.png` autoloads, against Love2D
  11.5's APIs.
- `conf.lua`: sets `t.window = false` so the shim's `love.load` can open the
  window at your `_config().game_width / game_height` in one step. Without this,
  Love opens a default 800×600 window first and the shim resizes after, causing
  a visible flash.

## How to use the shim

Run `usagi loveify`. Don't copy these files in by hand.

```sh
usagi loveify path/to/your-usagi-game path/to/your-love-port
cd path/to/your-love-port
love .
```

See the "Porting to Love2D" section of `README.md` at the Usagi repo root for
the full workflow.

## What's intentionally not in the shim

These are engine-managed conveniences. The shim's job is to keep gameplay code
(`_update` / `_draw`) working under Love. Engine scaffolding is something you
rebuild in your fork if you need it. The relevant Lua APIs exist as no-op stubs
so calls don't crash, but nothing renders or fires:

| Usagi feature                                       | Status in shim                                                     |
| --------------------------------------------------- | ------------------------------------------------------------------ |
| Pause menu (`usagi.menu_item` / `clear_menu_items`) | No-op stubs                                                        |
| Input remapping (keymap / pad-map overrides)        | Not implemented; the shim's default bindings apply unconditionally |
| Shaders (`gfx.shader_set` / `gfx.shader_uniform`)   | No-op stubs. Use `love.graphics.newShader` directly.               |
| Hot reload (F5)                                     | Not implemented. Love games ship static; rebuild and relaunch.     |
| FPS overlay                                         | Not implemented. Roll your own with `love.timer.getFPS()`.         |
| `usagi tools` / `export` / `font bake`              | N/A. Those are Usagi CLI subcommands, not runtime APIs.            |

## Lua compatibility

Love 11.5 runs on LuaJIT (Lua 5.1 plus a handful of 5.2/5.3 cherry-picks). Usagi
runs on Lua 5.5 plus a line-level preprocessor that expands compound
assignments. `usagi loveify` reuses Usagi's preprocessor on your `.lua` files so
`x += 1` ports cleanly. Things it does NOT yet transform automatically
(`loveify` warns at each occurrence so you can hand-fix them):

- `//` integer division: rewrite as `math.floor(a / b)`
- Bitwise operators (`&`, `|`, `~`, `<<`, `>>`): use LuaJIT's `bit` module
  (`bit.band` / `bor` / `bxor` / `lshift` / `rshift`)
- `string.pack` / `string.unpack`: no LuaJIT equivalent; rewrite by hand using
  `string.byte` / `string.char`
- `<const>` / `<close>` local attributes (Lua 5.4+): strip them, since LuaJIT
  rejects the syntax

## You own the shim

After `loveify`, the shim lives in your project as `usagi_shim.lua`. It's yours
now. Change it if you want. You can keep using the Usagi API or move away from
it. Replace chunks with idiomatic Love code as your game's needs diverge from
Usagi's API. Strip out modules you don't use. Add Love features the shim doesn't
expose. Future Usagi releases may iterate on this file, but once you've ported
you're on your own copy and that's the intended outcome.

## Known caveats

- **Pan on `sfx.play_ex` / `music.play_ex` / `music.mutate`** is silently
  ignored under Love 11.5 (no `setPan` API until Love 12).
- **`gfx.get_px`** is lazy: once any code calls it, a `canvas:newImageData()`
  runs every frame at draw tail. Cheap on desktop, less so on mobile or web.
- **First-frame `gfx.get_px` returns four `nil`** since no snapshot exists yet.
  Matches Usagi semantics.
- **Nintendo BTN1/BTN2 face button swap** is detected by gamepad name string
  match. Verified for Switch Pro Controller; other Nintendo pads may need their
  name added to the detector in `usagi_shim.lua`.
- **Web and mobile targets**: untested. Web specifically needs love.js or an
  equivalent compiler; the shim has no platform-specific code that should block
  it, but it's unverified.
- **Live reload** for the ported game is not built in and the shim doesn't add
  that. It's a feature of the Usagi Engine. There may be Love libraries that
  make this possible, but you're on your own.
