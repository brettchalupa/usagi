# Usagi Lua API Meta Stubs

`usagi.lua` in this directory is a **type-declaration file** for
[lua-language-server][lls]. It describes every global, table, constant, and
callback that the Usagi runtime exposes to game scripts so the LSP can give
editors completion, hover docs, and type-checking — **the file is never
executed**, the `---@meta` pragma at the top marks it as declarations-only.

The repo's `.luarc.json` points the LSP at this folder via `workspace.library`,
so any `.lua` file in the workspace gets the Usagi API in scope automatically.

## When to edit this file

Any time you change the Lua-facing API in `src/main.rs`, mirror the change here.
Common cases:

| Rust change                                        | Meta change                                        |
| -------------------------------------------------- | -------------------------------------------------- |
| New global table (e.g. `lua.globals().set("x",…)`) | Add a `---@class`, annotate, then declare `x = {}` |
| New field on an existing table                     | Add a `---@field name type  description` line      |
| New function on a table                            | Add doc block + `function tbl.name(args) end`      |
| Changed function signature                         | Update the `---@param` / `---@return` lines        |
| New constant                                       | Add as `---@field NAME type` on the owning class   |

If you forget to update this file, everything still _runs_ — the LSP just warns
on the new symbol ("undefined global", "unknown field") or shows stale types on
hover.

## EmmyLua cheatsheet

Only the annotations actually used in `usagi.lua`. Full reference:
[LuaLS annotations docs][ann].

```lua
---@meta                       -- required; marks this as declarations-only

---@class Name                 -- declare a type (for tables/userdata)
---@field key type  description
---@field other type

---@param name type  description
---@return type  description
function tbl.fn(name) end      -- body is empty; only the signature matters
```

Types: `nil`, `boolean`, `number`, `integer`, `string`, `table`, `function`, a
class name (`Usagi.Gfx`), or a union (`integer|string`). Lua 5.5 distinguishes
`integer` from `number` — use `integer` for constants and key codes, `number`
for coordinates/times.

## Example: adding a new gfx function

Say you add `gfx.circle(x, y, r, color)` in `src/main.rs`. The meta entry:

```lua
---Draws a filled circle centered at (x, y) with radius r.
---@param x     number
---@param y     number
---@param r     number
---@param color integer  a gfx.COLOR_* constant
function gfx.circle(x, y, r, color) end
```

Drop it inside the `gfx` section of `usagi.lua`.

[lls]: https://luals.github.io/
[ann]: https://luals.github.io/wiki/annotations/
