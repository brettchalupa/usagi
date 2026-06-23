# Usagi Quickstart

Usagi is a simple game engine for rapidly creating 2D games. If you're
experienced with game programming, this guide will you started with Usagi's
essentials over the course of a few minutes.

[📺 Watch a video version of the quickstart guide.](https://www.youtube.com/watch?v=0i1wIm6c6Rw)

Your first step is to install Usagi,
[following the instructions on the website](https://usagiengine.com).

Usagi is interacted with via the command line. Initialize a new project with
`usagi init`. Let's say you want to make Snake: `usagi init snake`. This command
bootstraps your game.

This creates a new `snake` folder on your computer. Within it you'll see
`main.lua` with the four key Usagi lifecycle functions:

- `_config` - defines various aspects of your game, like name, unique ID,
  resolution, etc.
- `_init` - code that's run on game start and when the game is hard reloaded in
  dev mode with <kbd>Ctrl + R</kbd>
- `_update` - where your input, simulation, etc. goes; called every frame, 60
  times a second, with optional `dt` parameter
- `_draw` - where you draw sprites, shapes, text, etc. to the screen; called
  every frame, 60 times a second, with optional `dt` parameterk

Within your folder, start you game in dev mode: `usagi dev`

The engine will then reload any changes to Lua code, `sprites.png`, music, sound
effect, and data.

If you want to run your game in release build to test out what your players will
play, use `usagi run`.

`usagi init` outputs the docs for the Usagi version used in `USAGI.md` and stubs
for the functions and constants in `meta/usagi.lua`.

All of your sprites go in `sprites.png`. Usagi defaults to 16x16 sprite size but
you can change that in `_config`. If you draw a sprite at the first grid
position, you'd draw it at x 10 and y 12 with `gfx.spr(1, 10, 12)`

Draw text with: `gfx.text("Hello!", 10, 12, gfx.COLOR_BLACK)`

Clear the screen every frame with: `gfx.clear(gfx.COLOR_WHITE)`

Check for input using Usagi's universal API that covers keyboards and gamepads:
`if input.pressed(input.BTN1)`, `if input.held(input.BTN2)`,
`if input.released(input.UP)`, etc. Usagi supports up to 3 action buttons.

Check for dev mode with `usagi.IS_DEV`

You can export your game using `usagi export`, which will create your game for
web, Linux, macOS, and Windows in the `export` folder. You can then share your
game with friends, upload it to itch, that kind of thing.

Usagi comes with a built-in Pause menu accessed with <kbd>Esc</kbd>,
<kbd>Enter</kbd>, and the gamepad's Start button. Users can control common
settings and rebind their input.

[View the full docs.](https://usagiengine.com)
