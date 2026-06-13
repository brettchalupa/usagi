# Dodge 'Em Up

In this chapter we'll make a very simple game where you move a square around the
screen and dodge circles that fly at you. We'll cover all of the basics of
making a game with Usagi and then package up our game to share it with others.

## Initializing a New Game

Now that you've got Usagi installed, you'll have the `usagi` command available.
Go ahead and open your text editor. Many code editors include a way to open a
terminal/shell within it. If you're using a text editor that doesn't, then
launch your terminal or Command Prompt or PowerShell separately. Run
`usagi init hello_usagi`. This will create a folder called `hello_usagi` with a
bunch of different files in them. The most important one is `main.lua`, which is
the primary entrypoint for your game. It's where you'll start out coding.

`USAGI.md` contains the full and complete documentation for Usagi in the
Markdown format. You can open it up and browse it to learn all about what Usagi
can do. It's a user manual of sorts.

`meta/usagi.lua` is a file that helps your text editor know what functions and
variables are available from Usagi. You don't edit this file, it's read-only and
to help improve your experience writing code.

In your terminal, run `usagi dev hello_usagi`. You'll see a window pop up that
draws some text on the screen.

![Black screen with text that reads "Hello, Usagi!"](./img/01-init.png)

Then in your text editor, open `main.lua`. You'll see this:

```lua
{{#include code/01-dodge-em-up/01-init/main.lua}}
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
soon. The `_draw` function is where you can show text, draw shapes, or put your
game's art on the screen.

`dt` is short for delta-time and it's passed into `_update` and `_draw`
automatically by the game engine. We'll cover it more in depth in a future
chapter. For now, it's unused and not something to worry about.

`gfx.clear(gfx.COLOR_BLACK)` clears the screen so that all that's shown is a
black rectangle. Each frame we clear the screen so that what was drawn on the
last frame doesn't reappear. Try changing `gfx.COLOR_BLACK` to `gfx.COLOR_RED`.
The background of your game instantly updates from black to red.

The next line `gfx.text("Hello, Usagi!", 10, 10, gfx.COLOR_WHITE)` is what draws
the message on the screen.

`_update` and `_draw` are functions we define ourselves, which Usagi looks for
and _calls_. `gfx.clear` and `gfx.text` are functions that Usagi provides, which
we _call_. Calling a function makes that code run. So `gfx.text` draws text to
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

The `x` and `y` parameters of the `gfx.text` function are the pixel coordinates
on our screen of where to place the upper-left corner of the text. The
upper-left corner of our game is the 0 x position and the 0 y position. If you
increase the `x` value, the text will move to the right. If you increase the `y`
value, it will move down.

![Illustration showing x and y axis with points on them representing their position](./img/screencoordinates.png)

By default, Usagi games are 320 pixels wide and 180 pixels tall. If you set the
`x` position of your text to `400`, it won't be visible in your game.

## Greeting

Let's write our own function. It's a great way to learn how functions work.
Rather than just greeting Usagi, let's make it easy to say hello to any given
name.

At the bottom of `main.lua`, add the following code:

```lua
{{#include code/01-dodge-em-up/02-greet/main.lua:21:23}}
```

Then, in `_draw`:

```lua
{{#include code/01-dodge-em-up/02-greet/main.lua:18}}
```

Try changing the name. What our updated `gfx.text` is doing is calling our new
`greet` function. We pass in the `name` we want to greet, wrapped in quotations
(note: these are not curly quotes; those are for writing prose, not coding).
When you wrap characters in quotations, this is called a **string** and it is
not evaluated as code. It's instead data that we can use in our code. The
`return` keyword in our function is what our function spits back to wherever
calls it. In our case, it passes the returned value into `gfx.text`. It draws
`"Hello, Alucard!"` on the screen. The `..` (two periods) is Lua's syntax for
how to combine strings. It squishes together `"Hello, "`, our `name` we pass in,
and `"!"` into a new string.

Add some other greetings to try out your new function.

Here's a simple function for adding two numbers and returning the result:

```lua
function add(a, b)
  return a + b
end
```

Functions can accept all sorts of data and return something that's computed
based on those values. You can see that `+` is used to calculate the sum of two
values in this example function. While `add` isn't something we'll use in our
game, it's useful to show what functions can be like. I tend to think of
functions as _verbs_, actions we want our code to take.

**Aside:** you have have noticed some lines of text starting with `--` in
`main.lua`. The double dash in Lua creates a **comment** which is code that's
not executed and is meant to be used to document how something works. Comments
are useful for future you (or collaborators) to remember what something does.
Throughout the book you'll see comments like: `-- px/s` to mean that the number
represents pixels per second for movement speed. I'll use comments to help
explain some of the code in our games.

[View the source code for this section.](https://codeberg.org/brettchalupa/usagi/src/branch/main/book/src/code/01-dodge-em-up/02-greet/main.lua)

## Drawing a Square

Let's draw a square to represent our player. You can delete our `greet`
function. And then replace the `gfx.text` function call with this:

```lua
{{#include code/01-dodge-em-up/03-square/main.lua:18}}
```

This draws a green rectangle at the position of x: 20 and y: 40. The rectangle
is a square, with each side being 16 pixels long. The third parameter is width,
the fourth is height. And the final parameter is the color. Try changing those
values around to see what happens. If you change `gfx.rect_fill` to `gfx.rect`,
it'll draw an outline of the rectangle instead of filling it in.

![Green square drawn on black background](./img/01-square.png)

Usagi makes it easy to draw a few different shape primitives like rectangles,
circles, and triangles. We'll draw circles in an upcoming section to represent
enemies.

[View the source code for this section.](https://codeberg.org/brettchalupa/usagi/src/branch/main/book/src/code/01-dodge-em-up/03-square/main.lua)

## Player Input

When you changed the x and y parameters in the `gfx.rect_fill` function call,
the square moved around the screen. That's all that movement in a game is:
positions changing. Those positions can change due to the passage of time or in
reaction to something else or from player input.

We keep track of data that can change in what's called a **variable**. Variables
get a name so that we can reference it and change it.

At the top of your `main.lua` file, add the following:

```lua
{{#include code/01-dodge-em-up/04-input/main.lua:1:2}}
```

This creates and sets the `x` variable to the number `20` and the `y` value to
the number `40`. The `=` sign does not mean equals, as in equality. It is the
assignment operator. It sets the variable on the left side to the value on the
right side.

Now update your `gfx.rect_fill` to use the new `x` and `y` variables:

```lua
{{#include code/01-dodge-em-up/04-input/main.lua:33}}
```

Instead of using the hard-coded values we previously had to position the square,
it's now determined by our new `x` and `y` variables. If you change the values
assigned to`x` and `y`, it changes where the square is drawn.

In order to move our little green square around, we need to check if the player
has pressed input from their keyboard or gamepad. Usagi provides a simplified
input API that lets you check for input directions and up to three action
buttons. So `input.held(input.UP)` checks if the <kbd>Up</kbd> arrow key or
<kbd>W</kbd> key on your keyboard is pressed or if any connected gamepads' d-pad
up or analog stick up are held down. Usagi provides a baked-in Pause menu with
the ability for players to remap controls. So if they change the up action to
something else you don't have to change your code. Kind of nice!

We'll make use of this `input.held` check in our `_update` function:

```lua
{{#include code/01-dodge-em-up/04-input/main.lua:16:29}}
```

If you use the arrow keys, WASD, or your gamepad, you can move the green square
around the screen. How this works is that 60 times per second, our game checks
if the direction inputs are held down. If they are, we use `=` to _reassign_ the
variable value to the previous value plus 4 pixels. So if the right key is held
down, each loop of our game adds 4 pixels to the `x` variable. This causes our
square to fly across the screen to the right.

The `if ... then` code means: only run the code between this check and the
corresponding `end` if what's between the `if` and the `then` is `true`. In
programming, `true` and `false` are known as boolean values and are used for
logic checks. If the left input is held down, then decrease the `x` position by
`4` pixels. One of the nice parts about the Lua programming language is how
natural the code reads, making it easier to understand because it's a lot like
how English is spoken.

Boolean checks are used so frequently when programming games. If the player is
dead, then show game over. If the timer is up, then play a sound effect. We'll
be adding many more throughout this chapter and the entire book.

[View the source code for this section.](https://codeberg.org/brettchalupa/usagi/src/branch/main/book/src/code/01-dodge-em-up/04-input/main.lua)

## Spawning Enemy Circles

Having a moveable player character is a natural first step, but let's give the
player something to do. We're going to make circles fly at the player from the
right side of the screen, spawning them at random positions and giving them
random speeds so that there's a bit of challenge.

At the top of `main.lua`, below our `x` and `y` variable assignment, add this:

```lua
{{#include code/01-dodge-em-up/05-input/main.lua:3}}
```

This assigns `{}` to the `enemies` variable. But what do those squigly brackets
_mean_? They're the symbols that represent the beginning and end of a **table**
in Lua. So far we've worked with **strings**, which are characters within
`"hello123"`. We've used whole integer **numbers** to represent the player's
position. The third absolutely foundational type of data in Lua programs are
**tables**. They're used to store collections of other data.

**Aside:** If you've programmed in other languages, Lua's tables are a single
data structure used for arrays and for JavaScript-like objects or Ruby-like
hashes.

The data within a table can be an ordered list. Something like this:

```lua
even_nums = { 2, 4, 6, 8, 10 }
classmates = { "Simon", "Alucard", "Richter" }
```

I'll call these types of tables **array tables** or just **arrays** throughout
the book. Each entry is separated by a comma (`,`).

Or you can assign values to a specific key:

```lua
monster = {
  hp = 44,
  str = 12,
  def = 5
}
```

The `monster` table has the key `hp` (which is technically the string `"hp"`)
assigned the value of `44`, using a very similar syntax to variable assignment.
Each key must have an associated value. And each key value pair in the table is
separated by a comma (`,`), just like the array style table.

These types of tables are sometimes called **associative tables**. Throughout
the book they'll often just be referred to as **tables**.

Array tables can contain tables as entries:

```lua
monsters = { { name = "Vampire", hp = 31 }, { name = "Golem", hp = 44 } }
```

And associative tables can have tables assigned to their keys:

```lua
player = {
  items = { "Potion", "Wing" }
  hp = 44,
}
```

Tables are flexible in Lua and quite powerful. I can't think of a game I've made
that doesn't use them, as they're you store collections of data. A game is
essentially a bunch of different collections of data that respond to player
input or time or some other system in the game.

But back to our `enemies = {}` line of code. That creates an empty table with
nothing in it. We'll treat this as an array table of data that contains our
enemy positions, spawning new ones at a set interval.

Right below that line, create these two new variables that we'll use for spawn
timing:

```lua
{{#include code/01-dodge-em-up/05-input/main.lua:4:5}}
```

In our `_update` function, below where we handle player input for movement, we
need to countdown our `enemy_spawn_timer` every frame of our game and add an
enemy to our `enemies` array if the timer is less than or equal to `0`:

```lua
{{#include code/01-dodge-em-up/05-input/main.lua:33:37}}
```

Just like we do with the `x` and `y` position for player movement, we update our
`enemy_spawn_timer` but subtracting the `dt` (delta time, which is how long has
passed between frames in seconds) from its current value. Then we check if it's
`<=` (less than or equal to) `0`. If it is, then we call the `table.insert`
function, which Lua provides. The first argument is the array table we want to
insert an entry into. In our case, it's the `enemies` array. `table.insert` adds
a new entry at the end of the table. The second argument is the data we want to
append. We pass an associative table with an `x` position and a `y` position,
which represents where the enemy will spawn at. `usagi.GAME_W` is the width of
the game, so the far right of the window. And `40` is just a little bit down
from the top of the screen.

Right after the enemy is spawned, we reset the `enemy_spawn_timer` to the
`enemy_spawn_delay`, beginning the countdown to spawn another enemy again. The
game continues to loop, running `_update` 60 times per second, decreasing
`enemy_spawn_timer` each time.

Right below that new code we added in `_update`, we want to make our enemies
move across the screen from right to left. In order to do this, we need to walk
through each enemy in our `enemies` array, one by one, and update its `x`
position by subtracting a value from it. Walking through an array item by item
is done with a `for` **loop**. Here's the code:

```lua
{{#include code/01-dodge-em-up/05-input/main.lua:39:42}}
```

Let's break it down line by line:

```lua
for i = 1, #enemies do
```

The first line starts the loop. `for` is the keyword to begin that style of
loop. `i = 1` assigns a variable at the start of the loop the value of `1`. The
next argument is the ending value of the loop. In our case, it's the total
number of `enemies` in that array. In Lua, you get the number of items in an
array with `#`. The `for` loop increments `i` by `1` until it hits the upper
value. And for each iteration of the loop, it calls the code contained between
the `do` and the `end`. In our case, that's:

```lua
local enemy = enemies[i]
enemy.x -= 2
```

We assign a `local` variable `enemy`. `local` is a keyword in Lua that says:
only make this variable exist within the _scope_ it was created in. Don't worry
too much about `local` yet, we'll cover that in the future. The value we assign
to `enemy` is `enemies[i]`. For array tables in Lua, you access the values in
the list by its position. The first item has a position of `1`, the 2nd has a
position of `2`, and so on. We call our position variable `i`, which is short
for _index_. You could name it `pos`, short for position if you want. So that
line that assigns `local enemy` grabs the current enemy in the array and assigns
it to a variable so we can easily change it. Which we do on the line below by
subtracting `2` pixels from that enemy's `x` position. This will make the enemy
move from left to right off the screen.

If games are collections of data, which we store in tables, then loops are how
we enumerate through our list and check or change that data.

All that's left for this section is to actually draw our enemies. In our `_draw`
function, after we clear the screen and draw our player, we need to loop through
our enemies yet again and draw them:

```lua
{{#include code/01-dodge-em-up/05-input/main.lua:49:52}}
```

We use the same style of `for` loop. But rather than update the `enemy`'s
position, we draw a filled red cicle at the `enemy`'s position. The `8` is the
radius of the circle in pixels. **Note:** when we draw our player, the origin is
the upper left of the green rectangle. But when drawing a circle, the `x` and
`y` describe the circle's center point. This can be slightly confusing but it's
worth knowing upfront as it'll influence the code we write in the rest of the
chapter.

Save your `main.lua` and you'll see red circles fly across the screen:

![Green square with red circles flying across the screen](./img/01-dodge-em-up-spawning.gif)

Kind of neat to see something moving on its own! But we're missing a few things:
handling when a circle hits the player and spawning our enemies at different `y`
positions to make it more challenging. Also, it'd be more interesting if the
speed of each circle was random to add some variation and keep the player on
thier toes.

## Random Y and Speed

TODO

## Hit Detection

## Recycling Enemies

## Clock

TODO

## High Score

TODO: saving and loading data

## Sharing Our Game

TODO: `usagi export`

## Bonus Credits

TODO: share ideas of what would be fun to expand on here

- Make enemies fly in from different sides, not just the right side.
- Add multiple enemy types that have different sizes and colors, to make the
  game more challenging.
