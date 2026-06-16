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

## Teal

TODO: https://teal-language.org/

## YueScript

TODO: https://yuescript.org/

## Browse the Examples

The Usagi source code repository has a folder of transpiled examples:
[https://codeberg.org/brettchalupa/usagi/src/branch/main/examples/transpile](https://codeberg.org/brettchalupa/usagi/src/branch/main/examples/transpile)
