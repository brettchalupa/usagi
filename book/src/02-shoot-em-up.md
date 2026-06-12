# Shoot 'Em Up

Shoot 'em ups (a.k.a. shmups, STGs, shooters) are games where you pilot a ship
and fire bullets. Your goal is simple: survive and defeat enemies. But with that
simple goal, there's challenge, fun, and depth. Many shmups have scoring
systems, adding even more replayability to game. Shmups go back decades. I'm
talking about games like _Galaga_, _R-Type_, _Dodonpachi_. Intense action games
with an arcade lineage. They also happen to be my favorite type of game to play
and make.

TODO: screenshot showing an example of the game type or what we'll make

Shmups are great for learning how to make games because you can build something
fun and challenging and begin iterating on it quickly, experimenting with
systems and enemy behaviors. The core of the game is having a playable ship that
can fire bullets, enemies that spawn and attack, and some sort of win state.
These simple basics can be expanded upon endlessly.

For our shoot 'em up, we're going to make a game where enemies spawn in waves.
You have to defeat them (or they have to exit the screen) before the next wave
spawns. You'll have 60 seconds survive, defeat as many enemies as possible, and
get the high score. Some enemies will dive down the screen, others will fire
bullets at the player. By the end of this chapter, we'll have made a shmup that
you can tune, expand, and make your own. We'll also dive deep on collision
detection and finding a balance between challenging gameplay and enjoyable
dodging.

This chapter builds upon the foundations from
[the Dodge 'Em Up](/01-dodge-em-up.html) chapter, so if you're new to
programming and Usagi Engine, read that first.

## Moveable Player

Ensure you have [Usagi Engine](https://usagiengine.com) installed. Run
`usagi init shmup` to create your new project. Open your new project folder in
your code editor.

We'll start by drawing a square to represent our player that can be moved around
the screen. In the Dodge 'Em Up chapter, you may have noticed that if you press
Up and Right (or any diagonal combination), the player moved faster than they
did when moving in the cardinal directions. In a lot of shmups, this isn't
ideal, as you want movement to be precise. In order to make the distanced
traveled in all 8 possible directions the same, we need to **normalize** our
input.

Here's the starting place for our game in `main.lua`:

```lua
{{#include code/02-shoot-em-up/01-moveable-player/main.lua}}
```

We set `player_size` and `player_speed` variables. The `local` keyword is new
and worth explaining a bit, as it impacts how Usagi's live reload works and
what's accessible in your game's source code as it expands into multiple files.

By default, when you create a variable in Lua, like `x = 10`, it is a **global**
variable. That means that any part of your game's source code can read its value
and even change it. This is powerful but risky. It's easy to accidentally
sometimes create global variables and accidentally change them when you didn't
intend to. When Usagi live reloads your games code, it **does not** update
global variables unless you press <kbd>Ctrl + R</kbd> or <kbd>F5</kbd>. On the
other hand, the `local` keyword says: only within this file or function or chunk
of code is this variable accessible. Usagi **does** re-evaluate `local`
variables when you change them. For our `player_size` and `player_speed`, if you
change them and save `main.lua`, the engine will re-evaluate your new values.
This is helpful for tuning speed and trying out different values to see what
feels right.

In our `_config()` function, we set the `name` of our game and the `game_id`.
Change the `game_id` to `com.usagiengine.YOURUSERNAME.shmup`, where you actually
put in your username/handle. This should be a unique identifier for your game,
which is used for the save data location on people's computers. The `game_width`
and `game_height` tell Usagi Engine to make our game field those specified
sizes. You can change these values to whatever you want, but for our game, a
square field feels good since you don't have to worry about covering a wide
distance to reach enemies on the other side of the screen. Enemies will fly in
from the top, which will make our shoot 'em up a vertically-oriented game.

In `_init()`, we create a global `State` table with our `player`'s position.
`State` is a common way in Usagi games to have a global to contain all of the
game's data, allowing for easy access. Since `State` is global, it doesn't
change when the game is live reloaded, which is what we want. This lets our
player stay in the same position when our game code changes. You could change
`player_speed` and instantly test that new value without the entire game
reseting. The math in the `player` `x` and `y` value centers our player
horizontally and places the `y` value 60 pixels up from the bottom of the game.
The values of `usagi.GAME_W` and `usagi.GAME_H` correspond to what we set in
`_config`. Yoou could just hardcode `320` instead for each of them, but if you
decide to change the width or height of your game, you'll be left searching for
and updating all of those old values. When possible, it's best to not use
**magic numbers** for values in our game.

The `_update` function contains our player movement, similar to Dodge 'Em Up.
Except rather than changing the player's `x` and `y` value in the `if` checks,
we update a variable called `input_delta`. `input_delta` is a Lua table that
lets us set whether or not there was movement on a given axis. By using `1`,
we're creating what's known as a unit vector, which makes normalizing it on the
diagonals easier. Then we call `util.vec_normalize(input_delta)` after our input
checks. `util` is a collection of functions that Usagi provides to make common
operations easier. That function returns a new table with the values normalized.

When you press right and down, rather than `x` and `y` both being `1`, the value
of both are: `0.7071...`. This makes it so that the distance traveled is the
same in all directions. We then take that normalized value and multiply it by
the `player_speed` and `dt` (`dt` is delta time, the amount of time since our
last `_update` call). This gives us the new position for the `State.player`.
After that, we prevent the player from moving off the screen by calling
`util.clamp` on the `x` and `y` position of the player. `util.clamp` takes three
values: the value you want to limit, the lower limit, and the upper limit. If
the value is below the lower limit, then the lower limit is returned. If the
value is higher than the upper limit, the upper limit is return. Otherwise, the
value is returned.

Finally, in `_draw`, we clear the screen so we have a white background. And then
draw a black rectangle at the `State.player`'s position.

This was a whole lot for the first section of our chapter, but we've got a good
starting point to build upon. Tweak the `player_speed` and `player_size` to see
what happens.

[View the source code for this section.](https://codeberg.org/brettchalupa/usagi/src/branch/main/book/src/code/02-shoot-em-up/01-moveable-player/main.lua)

## Firing Bullets

Let's make our player's ship fire bullets upward. We'll keep track of them in a
Lua table. Each frame we'll move them upward and if they scroll off the screen,
we'll remove them from the table.

Start by setting up some local variables at the top of `main.lua`:

```lua
{{#include code/02-shoot-em-up/02-firing-bullets/main.lua:3:7}}
```

We'll use all of these variables for firing and drawing bullets.

In our `State` table, add a new empty table for `bullets`:

```lua
{{#include code/02-shoot-em-up/02-firing-bullets/main.lua:20:26}}
```

We'll add new bullets into that table when they're fired and loop through it for
updating the bullets and draw them on the screen.

In our `_update` function, below where we handle player movement, add the
following code:

```lua
{{#include code/02-shoot-em-up/02-firing-bullets/main.lua:49:72}}
```

In each frame, we subtract the `dt` from `fire_timer` to count it down. Then, if
the `fire_timer` is less than or equal to `0` and the player is pressing BTN1
(keyboard: Z or gamepad: A by default), then fire three bullets. The firing of a
bullet uses the Lua function `table.insert`, which appends a new bullet at the
`x` and `y` position to `State.player.bullets` table. Then, finally, we reset
the `fire_timer` to `fire_delay`, which restarts the countdown, adding a slight
gap between each time a set of bullets get fired.

The `for i = #State.player.bullets, 1, -1 do` line of code is a loop that goes
through the player's bullets in reverse, moving them up the screen by
subtracting the `bullet_speed * dt` from each bullet's `y` position. If the
bullet is so far up the screen that's it's no longer visible (the negative
height of the bullet), then we need to remove it from the player's bullets
table. We have to loop through the bullets in reverse order so that if we do
remove a bullet, those in the array from that position onward will properly
shift into position. If you didn't reverse the order of looping through the
bullets, if you removed the first bullet, they remaining would shift forward,
causing the next iteration of the loop to skip one and potentially access an
index that no longer exists.

Now we need to draw our bullets by looping through them at the bottom of
`_draw()` and drawing a light gray rectangle:

```lua
{{#include code/02-shoot-em-up/02-firing-bullets/main.lua:82:85}}
```

In less than 100 lines of code, we've got a pretty good feeling player ship that
moves around the screen and fires bullets. Not bad!

![player moving around the screen and firing bullets](./img/shmup-bullet-firing.gif)

[View the source code for this section.](https://codeberg.org/brettchalupa/usagi/src/branch/main/book/src/code/02-shoot-em-up/02-firing-bullets/main.lua)

## Enemies Fly Into Position

TODO

## Defeating Enemies

TODO

## Player Death

TODO

---

## Outline:

- Player movement w/ normalized diagonals
- Firing bullets
- Enemies fly into position
- Focus shot vs spread
- Hitboxes & enemy death
- Dev mode display
- Enemies fire bullets
- Player death
- Bullet patterns pt 1 - aimed shots
- Bullet patterns pt 2 - static shots
- Waves of enemies
- Medals
- Enemies move
- Starfield Background
- Player bombs
- Time over & scoring
- Bonus Credits
  - Sound effects
  - Sprites
