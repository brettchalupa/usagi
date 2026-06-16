# MoonScript to Lua for Usagi Game Dev

MoonScript is a programmer friendly language that compiles to Lua, heavily
inspired by the syntax of CoffeeScript.

Website: [https://moonscript.org/](https://moonscript.org/)

## Developing

1. Install via luarocks: `luarocks install moonscript` or your system's package
   manager
2. Write your code in `main.moon`
3. Start up the MoonScript compiler: `moonc --watch main.moon`
4. Start up the Usagi dev mode: `usagi dev`

When you make changes to `main.moon`, `moonc` will automatically transpile it to
`main.lua`, which `usagi dev` picks up.

Use `export _config, _draw, _update, _init` to declare Usagi's global callbacks
that are needed for the core game setup and loop.
