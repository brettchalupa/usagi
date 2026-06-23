# Level Editors

There are lots of great level editing tools that exist for designing maps and
levels in your games. Here's an overview of how to use the most common ones.

## Tiled

[Tiled](https://www.mapeditor.org/) is a long-standing map editing software. You
can export your maps as Lua files, which you can then `require` in your Usagi
game code. When you then re-export your level as Lua, it will live update in
your game.

Steps for using Tiled:

1. Set up your project and tileset; the tileset should point to `sprites.png`
2. Save your project in Tiled's tmx format
3. Export your map using <kbd>Ctrl + Shift + E</kbd> and save it as `map.lua` or
   whatever you want to name it
4. Then press <kbd>Ctrl + E</kbd> after making future changes to quickly
   re-export `map.lua`

Then in your Usagi game code, you'll `local map = require("map")` which gives
you the Lua table of your exported map. You can loop through the data to check
for collisions, draw your map, etc.

[View the Tiled example.](https://codeberg.org/brettchalupa/usagi/src/branch/main/examples/tiled)
It shows how to loop through the layers of a level and draw them with camera
scrolling. The example provides a
[`tiled.lua`](https://codeberg.org/brettchalupa/usagi/src/branch/main/examples/tiled/tiled.lua)
that you can drop into your project and use like: `tiled.draw_map(map, camera)`.

Alternatively, you can also save your Tiled projects using the JSON format
instead of the TMX format in your project's `data` directory and import them in
a way similar to LDtk with `usagi.read_json`.

## LDtk

[LDtk](https://ldtk.io) is a newer map editor. LDtk files contain all the
project's maps/levels and uses the JSON format. Here's how to use LDtk with
Usagi Engine:

1. Set up your LDtk project; set up your tileset to poin to `sprites.png`
2. Save your LDtk project in `data/maps.ldtk` or `data/levels.ldtk`
3. Use `usagi.read_json` to read your LDtk data:
   `local ldtk_project = usagi.read_json("level.ldtk")`

You can then loop through the maps and layers and check for collisions and draw
your map.

[View the LDtk example.](https://codeberg.org/brettchalupa/usagi/src/branch/main/examples/ldtk)
It shows how to loop through the layers of a level and draw them with camera
scrolling. The example provides a
[`ldtk.lua`](https://codeberg.org/brettchalupa/usagi/src/branch/main/examples/ldtk/ldtk.lua)
that you can drop into your project and use like:
`ldtk.draw_level(ldtk_project.levels[1], camera)`
