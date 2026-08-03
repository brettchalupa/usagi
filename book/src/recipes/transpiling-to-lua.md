# Transpiling to Lua

There are a whole host of languages out there that compile to Lua, allowing you
to write your Usagi games in something other than vanilla Lua 5.5. Those
languages might have other features or a style or syntax that you prefer. This
recipe breaks down and shows some examples of how to go about this.

Transpiling works by taking the source language and spitting out a `main.lua`
file that `usagi` then uses. What's nice about this is that you can read through
the Lua code that gets generated to help see what's really happening under the
hood and debug potential issues.

## TypeScript

Using the [TypeScriptToLua](https://typescripttolua.github.io/) library, you can
write your Usagi games in TypeScript, which is a typed version of JavaScript
that's widely used in web development. You can use it with the `npm` package
manager, possibly even with Bun or Deno too (but that hasn't been tested).

Here's an example of a TypeScript Usagi game:
[https://codeberg.org/brettchalupa/usagi/src/branch/main/examples/transpile/ts_to_lua](https://codeberg.org/brettchalupa/usagi/src/branch/main/examples/transpile/ts_to_lua)

**Note:** the typedefs are currently very minimal and incomplete. Someone
maintaining them as an external package would be a good community project. Or
just write them by hand as you need them.

Usagi relies a lot upon global functions, tables like `gfx` and `input`, and
does not use objects or `self`. So when writing the TS, you'll need to set
special parameters or add comments for the TypeScript compiler. Useful
references:

- [https://typescripttolua.github.io/docs/the-self-parameter](https://typescripttolua.github.io/docs/the-self-parameter)
- [https://typescripttolua.github.io/docs/assigning-global-variables](https://typescripttolua.github.io/docs/assigning-global-variables)

## Teal

Teal is a statically-typed dialect of Lua. Teal is to Lua what TypeScript is to
JavaScript. It allows you to add types for your code and check those types.

Website: [https://teal-language.org/](https://teal-language.org/)

You install the `tl` binary to check your types and compile your Teal code into
Lua. `tl check main.tl` checks the types. `tl gen main.tl` outputs the
`main.lua` that Usagi uses.

You have to handwrite the Usagi types that you use if you don't want the type
checker to fail. It'd be nice if in the future there was a community-maintained
typedef if people end up using Teal. Docs on typedef:
[https://teal-language.org/book/latest/declaration_files.html](https://teal-language.org/book/latest/declaration_files.html)

Here's an example of a Teal usagi game:
[https://codeberg.org/brettchalupa/usagi/src/branch/main/examples/transpile/teal](https://codeberg.org/brettchalupa/usagi/src/branch/main/examples/transpile/teal)

Here's an example of `main.tl`:

```tl
-- barebones typedef example for Usagi's `gfx` functions and constants
global record gfx
  clear: function(color: number)
  text: function(text: string, x: number, y: number, color: number)
  COLOR_BLACK: number
  COLOR_WHITE: number
  COLOR_PEACH: number
end

global function _draw(dt: number)
  gfx.clear(gfx.COLOR_BLACK)
  gfx.text("Hello, Teal!", 10, 10, gfx.COLOR_WHITE)
  gfx.text("dt: " .. dt, 10, 32, gfx.COLOR_PEACH)
end
```

## YueScript

YueScript is a programmer friendly language that compiles to Lua, heavily
inspired by the indentation-based syntax of CoffeeScript. It's a fork of
MoonScript (see below).

In order to use YueScript with Usagi, you need the `yue` executables installed.
You then compile your `main.yue` into `main.lua` with `yue main.yue`.

Website: [https://yuescript.org](https://yuescript.org)

Here's a very simple working example:
[https://codeberg.org/brettchalupa/usagi/src/branch/main/examples/transpile/yuescript](https://codeberg.org/brettchalupa/usagi/src/branch/main/examples/transpile/yuescript)

Here's an example of `main.yue`:

```yue
export _config, _draw

global _config = -> { name: "YueScript Ex" }

global _draw = (dt) ->
  gfx.clear(gfx.COLOR_BLACK)
  gfx.text("Hello YueScript!", 10, 10, gfx.COLOR_WHITE)
  gfx.text("dt: " .. dt, 10, 32, gfx.COLOR_PEACH)
```

## MoonScript

MoonScript is a programmer friendly language that compiles to Lua, heavily
inspired by the indentation-based syntax of CoffeeScript. The syntax is fairly
different than Lua.

In order to use MoonScript with Usagi, you need the `moon` and `moonc`
executables installed. You then compile your `main.moon` into `main.lua` with
`moonc main.moon`.

Website: [https://moonscript.org/](https://moonscript.org/)

Here's a very simple working example:
[https://codeberg.org/brettchalupa/usagi/src/branch/main/examples/transpile/moonscript](https://codeberg.org/brettchalupa/usagi/src/branch/main/examples/transpile/moonscript)

Here's an example of `main.moon`:

```moon
export _config, _draw

_config = -> { name: "MoonScript Ex" }

_draw = (dt) ->
  gfx.clear(gfx.COLOR_BLACK)
  gfx.text("Hello MoonScript!", 10, 10, gfx.COLOR_WHITE)
  gfx.text("dt: " .. dt, 10, 32, gfx.COLOR_PEACH)
```

## Browse the Examples

The Usagi source code repository has a folder of transpiled examples:
[https://codeberg.org/brettchalupa/usagi/src/branch/main/examples/transpile](https://codeberg.org/brettchalupa/usagi/src/branch/main/examples/transpile)
