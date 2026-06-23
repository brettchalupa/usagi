# Style Guide

Usagi's examples and this book (for the most part) uses the following style
guide for Lua code:

- 2 spaces for indentation. `snake_case` for locals, function names, table
  fields, and helper module names (e.g. `enemy.lua`, `local fresh_state`).
- `SCREAMING_SNAKE_CASE` for compile-time-ish constants (file-scope
  `local TICK = 0.12`, `local MAX_BULLETS = 12`, the engine's `gfx.COLOR_*`
  table). Distinguishes "tunable knob" from "runtime variable."
- Engine API is lowercase (`gfx`, `input`, `sfx`, `music`, `usagi`). It's
  declared in `meta/usagi.lua` so the LSP treats it as predefined; reads in user
  code don't trip `lowercase-global`.
- **Globals are `Capitalized`.** This includes the canonical game-state
  container (`State = { ... }` set inside `_init`) and module imports kept as
  globals (`Player = require("player")`). The capitalization signals
  "intentional global, lives across reloads"; anything lowercase at file scope
  is treated by `lowercase-global` as an accident (forgot a `local`).
- Why `State` is a global: live reload re-execs the chunk on every saved edit. A
  `local State` at module scope would get re-bound to a fresh table every save
  and obliterate the running game. Setting `State` in `_init` (and only in
  `_init`, which only runs at startup and on F5) lets the table outlive reloads.
- If you have a global need that isn't `State`, the convention scales:
  capitalize and document it. Module-bound require results (`Enemy`, `Bullet`)
  are the common second case.

This pattern (engine-API lowercase / game-state capitalized / locals snake_case)
is the same one shipped in `.luarc.json` and the `usagi init` template, and is
what every example under `examples/` follows.
