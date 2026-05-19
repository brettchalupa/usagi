# _Game Programming with Usagi Engine_

The advice often given to new game developers is to make a lot of small games.
That way the new developer can learn the fundamentals of game development,
explore ideas, and figure out what games they enjoy making. The problem though
is that most game engines aren't conducive to making small games quickly. After
making small games for over twenty years, I decided to try to solve that problem
by creating **Usagi Engine**, a free and open source game engine that's
specifically focused on making small 2D games and being able to quickly share
them. Your code, art assets, audio files, and data are live reloaded as you
change them, removing the laborious step of having to constantly re-launch your
game after every change to test it. If want to see if a different color looks
better, just edit your sprite, save it, and see it update instantly. I can't
overstate how useful this is.

Usagi games are programmed with Lua, a simple and widely-used language. There
are many different game engines and libraries that use Lua, which means the
knowledge you gain from learning to make games with Usagi is useful even if you
stop using the engine. When you code a game, the engine provides functions,
which are named pieces of code that do _something_ like draw a shape on the
screen or play a sound effect. Large game engines have hundreds or thousands of
functions, requiring you to study complex manuals to find what you need. Usagi,
on the other hand, is embraces constraints and has a limited number of functions
that cover the functionality most games need.

(TODO: talk about more constraints and why)

Usagi is not the everything engine. There's a lot it doesn't do. But it excels
at being simple, approachable, and fast. When you're exploring an idea or
participating in a game jam, you don't want to spend your time coding input
mapping or where to put save data on Linux computers. You want to focus on
making your game fun to play. Usagi provides input mapping, simple ways to check
for player input via keyboard and gamepad, easy save data, and a fully-featured
Pause menu. Also, with a single command you can export your game for web, Linux,
macOS, and Windows.

This book, _Game Programming with Usagi Engine_, is written for someone just
getting started out making games. If you've coded games before, great! You'll be
able to pick up on things even quicker. But if you haven't, don't worry. We'll
go through making games step-by-step in guided tutorials. The second half of the
book contains recipes that are focused lessons on specific functionality.

## Getting Started

There are three things you need to get started with Usagi:

1. A computer running Linux, macOS, or Windows.
2. A text editor installed for writing code. I use [Zed](https://zed.dev), a
   free and open source editor. Visual Studio Code is another popular free code
   editor.
3. Usagi installed; follow the instructions at https://usagiengine.com.

Usagi is interacted with via the command line. You type in commands rather than
click buttons in a graphical user interface. On Linux and macOS, this program is
called the Terminal. On Windows, the two primary tools are called the Command
Prompt and PowerShell. While the command line can daunting at first, there are
only a few commands you'll need to know to work with Usagi most of the time.

## Hello Usagi

Once Usagi is installed, you'll have the `usagi` command available. Go ahead and
open your text editor. Many code editors include a way to open a terminal/shell
within it. If you're using a text editor that doesn't, then launch your terminal
or Command Prompt or PowerShell separately. Run `usagi init hello_usagi`. This
will create a folder called `hello_usagi` with a bunch of different files in
them. The most important one is `main.lua`, which is the primary entrypoint for
your game. It's where you'll start out coding.

`meta/usagi.lua` is a file that helps your text editor know what functions and
variables are available from Usagi. You don't edit this file, it's read-only and
to help improve your experience writing code.

In your terminal, run `usagi dev hello_usagi`. You'll see a window pop up that
draws some text on the screen.

(TODO: show screenshot)

Then in your text editor, open `main.lua`. You'll see this:

```lua
function _config()
  return { name = "Game", game_id = "com.usagiengine.YOURGAMENAME" }
end

function _init()
  -- Live reload preserves globals across saved edits but resets locals.
  -- Stash mutable game state in a capitalized global like `State` so it
  -- survives reloads; F5 calls _init again to reset.
  State = {}
end

function _update(dt)
end

function _draw(dt)
  gfx.clear(gfx.COLOR_BLACK)
  gfx.text("Hello, Usagi!", 10, 10, gfx.COLOR_WHITE)
end
```

If this is your first time seeing code, congratulations! This is Lua.
`function`s are reusable pieces of code that can be called to make whatever code
is contained within the `function` and `end` run. We'll dive more into functions
soon. But let's walk through the code a bit more first.

`_config()` is a place where you can set your game's `name` and `game_id`. The
`game_id` is used for putting your game's save data in the proper location on
your players' computers. Don't worry about this too much yet.

`_init()` is a function that gets run when your game starts (and when you press
<kbd>F5</kbd> or <kbd>Ctrl+R</kbd>). It's a good place to set up data once.

Then `_update(dt)` and `_draw(dt)` are sort of siblings. They get called 60
times per second, over and over again, automatically by Usagi. This is called
**the game loop**. Games run rapidly so that movement is smooth and the game can
react quickly to player input. Each iteration through the loop is called a
**frame**, similar to how each image in a movie is a frame. Movies are often 24
frames per second (FPS), whereas games are often 60 FPS. The `_update` function
is where you check for player input, have entities in your game react to what's
happening, and simulate the game. There's nothing there yet, but there will be
soon. The `_draw` function is where you can show text, draw hapes, or put your
game's art on the screen.

`dt` is short for delta-time and it's passed into `_update` and `_draw`
automatically by the game engine. We'll cover it more in depth in a future
chapter. For now, it's unused and not something to worry about.

`gfx.clear(gfx.COLOR_BLACK)` clears the screen so that all that's shown is a
black rectangle. Each frame we clears the screen so that what was drawn on the
last frame doesn't reappear. Try changing `gfx.COLOR_BLACK` to `gfx.COLOR_RED`.
The background of your game instantly updates from black to red.

The next line `gfx.text("Hello, Usagi!", 10, 10, gfx.COLOR_WHITE)` is what draws
the message on the screen.

`_update` and `_draw` are functions we define ourselves, which Usagi looks for
and _calls_. `gfx.clear` and `gfx.text` are functions that Usagi provides, which
we _call_. Calling a function makes that code run. so `gfx.text` draws text to
the screen. It knows which text to draw, where to place it, and what color to
make it by passing in arguments. Arguments are comma-separated values that
correspond to the parameter list of the function. `gfx.text` expects the text
message to show, the x coordinate, the y coordinate, and the color of the text
as its arguments.

Try changing a few aspects of `gfx.text` and see what happens. Update the
message, change the `10`s, and use a different color.

Next, copy that line of code and paste it below. Draw a different message to the
screen in a different position. And don't forget to save your `main.lua` file.

You're coding! And Usagi is live updating, giving you instant feedback to your
changes.

Normally, in most game engines, you'd need to change your code, save it, and run
a command to start the game again. With Usagi, you just change it and save it
and see your changes.

(TODO: explain the coordinate system + image)

The `x` and `y` parameters of the `gfx.text` function are the pixel coordinates
on our screen of where to place the upper-left corner of the text. The
upper-left corner of our game is the 0 x position and the 0 y position. If you
increase the `x` value, the text will move to the right. If you increase the `y`
value, it will move down.

By default, Usagi games are 320 pixels wide and 180 pixels tall. If you set the
`x` position of your text to `400`, it won't be visible in your game.

## Greeting

Let's write our own function. It's a great way to learn how functions work.
Rather than just greeting Usagi, let's make it easy to say hello to any given
name.

At the bottom of `main.lua`, add the following code:

```lua
function greet(name)
  return "Hello, " .. name .. "!"
end
```

Then, in `_draw`:

```lua
gfx.text(greet("Alucard"), 10, 10, gfx.COLOR_WHITE)
```

Try changing the name. What our updated `gfx.text` is doing is calling our new
`greet` function. We pass in the `name` we want to greet, wrapped quotations
(note: these are not curly quotes, those are for writing, not coding). When you
wrap characters in quotations, this is called a **string** and it is not
evaluated as code. It's instead data that we can use in our code. The `return`
keyword in our function is what our function spits back to wherever calls it. In
our case, it passes the returned value into `gfx.text`. It draws
`"Hello, Alucard!"` on the screen. The `..` (two periods) is Lua's syntax for
how to combine strings. It squishes together `"Hello, "`, our `name` we pass in,
and `"!"` into a new string.

Add some other greetings to try our your new function.

While we're not going to use this in our game yet, functions can take numbers
and return them as well. Here's a simple function for adding two numbers:

```lua
function add(a, b)
  return a + b
end
```

## Moving a Square

(TODO: drawing shapes) (TODO: player input) (TODO: State)

## Sharing Our Game

(TODO: `usagi export`)
