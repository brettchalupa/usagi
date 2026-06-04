# Usagi Versus X

Picking what game engine to use is a big choice! There are lots of options out
there, all with their own pros and cons. In this recipe, I'll go over what the
differences are compared to Usagi Engine to hopefully help you decide. I've only
included ones I've used, and this recipe is primarily focused on tools of
similar scope (2D games for PC and web).

My comparisons attempt to be as objective as possible based on my experience.
I've been making games for over 20 years, and I don't benefit financially from
you using Usagi Engine, so I really have no ulterior motive. There are many
times when using an engine other than Usagi is the right choice!

## Usagi Versus Pico-8 & Picotron

Pico-8 and Picotron are fantasy consoles made by Lexaloffle that include
fully-featured sprite, map, and sound editors. Games are coded in Lua, just like
Usagi. Pico-8 and Picotron are major influences on Usagi Engine. They export
their games to PC and web, which is great.

You should use Pico-8 or Picotron if you:

- Value having robust tools built into the engine.
- Want to be part of a community using and sharing games using that technology,
  as they both have excellent communities.

You should use Usagi Engine instead if you:

- Prefer free and open source software, as Pico-8 and Picotron are closed
  source.
- Want you game to automatically update when you change your code and assets.
- Don't want to worry about token limits.
- Prefer using a standalone sprite editor and using sound effects and music in
  common formats that you made or have permission to use.
- Want a bit more flexibility and power.
- Need to be able to change the underlying aspects of the engine.

I built Usagi Engine and use it instead of these tools because of those reasons.
The live reload in particular is fantastic. I love that Pico-8 and Picotron
exist and are incredible tools.

## Usagi Versus Pyxel

Pyxel is a kind-of-sort-of fantasy console where you code games with Python.
It's less integrated than Pico-8 and Picotron, as you bring your own code
editor. But it does have more robust tools than Usagi. Pyxel is also free and
open source.

You should use Pyxel if you:

- Want to code your games in Python.
- Want more tools than what `usagi tools` gives you.

You should use Usagi Engine instead if you:

- Want to use Lua to code your games.
- Want simpler cross-platform game export, as it's a little bit more tricky with
  Pyxel for desktop exports.

## Usagi Versus TIC-80

TIC-80 is a free and open source fantasy console, most similar to Pico-8. It
supports a bunch of different languages: lua, ruby, js, moon, fennel, scheme,
squirrel, wren, wasm, janet or python.

The same reasons to use Usagi over Pico-8 apply to TIC-80. Usagi has live reload
and fewer constraints.

## Usagi Versus Love2D

Love2D is a game programming library for Lua. It's much more complex than Usagi
and therefore much more powerful. It has some downsides though: it's not as easy
to export your game for web, Linux, macOS, and Windows like it is with Usagi.
Also, live reload isn't a built-in feature.

You should use Love2D if you:

- Want a much bigger API that let's you do pretty much everything you'd want to
  do in a 2D game.
- Want to make games for mobile devices.
- Want to code input bindings, pause menu, settings, save data, and more
  yourself.

You should use Usagi instead if you:

- Value the live reload and single-command export functionality.
- Don't want to mess with Love.js to get web builds.
- Would like a smaller, simpler API that does most things 2D games does and are
  okay with some limitations.

In a lot of ways, I think if someone makes a robust prototype with Usagi and
they want to expand it into a bigger commercial game, Love2D is a great fit.
Porting from Usagi to Love2D is not difficult because of the shared language.
And you'd then get a lot more power (but with a worse develper experience).

I hope to build out an Usagi -> Love2D shim in the future to make it as easy as
possible to port an Usagi game to Love2D.

## Usagi Versus Godot

Godot is an open source game engine for making 2D and 3D games. It's mature,
robust, and fully featured. It has its own scripting language called GDScript.
Or you can use other languages with it via various toolchains, with C# being a
popular one. Most people using Godot are making their games in the Godot Editor,
which is a fully-featured graphical user interface. Godot is a great engine, but
it's a lot to dig into and learn. Usagi is much simpler, for better and worse.
If you want to make a commercial game, Godot is likely a great fit! If you want
to make simple 2D prototypes rapidly, both would work well. Godot features
cross-platform export like Usagi does. But the web performance, at least the
last time I used it, was quite poor.

You should use Godot if you:

- Want to make a 3D game.
- Want to use a tool with a large community and lots of resources.
- Have commercial aspirations from the get-go.
- Prefer all-in-one graphic user interfaces.

You should use Usagi instead if you:

- Want to code games in Lua instead of GDScript.
- Prefer having more control of your game loop.
- Want to learn coding.
- Prefer simpler tools.
- Prefer command-line interfaces.

## Usagi Versus Defold

I haven't used Defold, so I honestly have no idea how it compares!

## Usagi Versus DragonRuby Game Toolkit

DragonRuby Game Toolkit is a commercial game engine for making 2D games with
Ruby. It's a key inspiration for Usagi. It's closed source. It has a lot of
similar ergonomics.

You should use DragonRuby Game Toolkit if you:

- Prefer Ruby over Lua.
- Want to target mobile platforms.
- Want a more powerful API.

You should use Usagi instead if you:

- Want to use a free and open source where you can modify the source.
- Prefer a simpler API and some constraints.
- Want to make lower resolution pixel art games.

## Usagi Versus Raylib

Raylib is a game programming library in C with lots of bindings for other
languages. Raylib supports 2D and 3D games. It's also what powers Usagi Engine
under the hood! Raylib is great, but it's got a lot going on. Coding in C is
also quite a bit more complex than Lua.

You should use Raylib if you:

- Want to completely own your game loop.
- Need more performance.
- Want to code your game in a lower level language like C, C#, Rust, etc.
- Want to code 3D games.
- Need a more powerful API.

You should use Usagi instead if you:

- Prefer simpler languages like Lua.
- Want live reload of code and art assets as you change them.
- Prefer a command-line driven workflow with cross-platform exports in a single
  command.

## Usagi Versus MonoGame / FNA / LibGDX / Bevy / Karl2D / SDL / etc.

There are a lot of different open source 2D and 3D game engines and libraries
out there. You can find one for most programming languages. Which you should use
is very similar to the Raylib thoughts above. You'll miss out on the simplicity
and rapid prototyping speed of Usagi, but you gain much more power and
flexibility.

## How to Decide

It's ultimately your decision! That's part of what's fun about making games.
There's no single best choice or an engine that does everything for everyone.
You could go and spend a few months or years evaluating all of the options.
That's fun in and of itself and you'll learn a lot. But if you want to focus on
making games and growing as a developer, I'd pick one or two and get really
comfortable with using it. Much like if you buy some tools for home improvement.
There are so many tools out there, but once you find the ones that feel great in
your hand and you know how to use, you can get to work on improvements instead
of constantly evaluating tools. The same applies to games. Your time is better
spent learning a tool deeply and growing as a game developer rather than surface
level hopping around engines.

My toolkit and thinking is quite simple:

- For small 2D games (like in a game jam or something I make over a couple of
  weeks), I use Usagi Engine. It's perfect for this.
- For larger games, if I wanted to stay in Lua, I'd use Love2D. But I honestly
  would prefer to not use Lua for larger games because after 15 years of coding
  Ruby in large codebases, I'd much prefer to use a statically typed language
  for anything remotely complex. Large codebases that's pure scripting languages
  are a nightmare to work in and change without excessive automated tests.
- So, for larger games, I'd use sola-raylib, the Rust bindings for Raylib, that
  I maintain. That way I have full control, better tooling, a compiler, a
  linter, and the ability to use Lua if I want in various places.
