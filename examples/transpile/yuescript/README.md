# YueScript to Lua for Usagi Game Dev

YueScript is a programmer friendly language that compiles to Lua, similar to
MoonScript and CoffeeScript.

Website: [https://moonscript.org/](https://moonscript.org/)

## Developing

1. Download the yue CLI via `luarocks install yuescript` or
   [https://github.com/IppClub/Yuescript](https://github.com/IppClub/Yuescript)
2. Write your code in `main.yue`
3. Start up the MoonScript compiler: `yue -w .`
4. Start up the Usagi dev mode: `usagi dev`

When you make changes to `main.yue`, `yue` will automatically transpile it to
`main.lua`, which `usagi dev` picks up.

Use `export _config, _draw, _update, _init` to declare Usagi's global callbacks
that are needed for the core game setup and loop.
