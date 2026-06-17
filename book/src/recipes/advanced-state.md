# Advanced State

The `State` global variable that is created in `usagi init` and used in
[the examples](https://codeberg.org/brettchalupa/usagi/src/branch/main/examples)
is nothing special to Usagi. It's just a global variable that's a table that
gets set in `_init` and persists across live reloads. You could call this
variable whatever you want. You don't have to use it either. But the big benefit
of having a global variable in `_init` is that you can make changes to your game
and keep the current level, the player's location, etc. all in memory while
changing the behavior of the game and rapidly testing your changes.

You will read in various places that global variables are bad news, and while
that's generally true, for smaller games, it's actually a really convenient way
to keep track of your game's data. Especially since Usagi is single-threaded and
intended for smaller games. By having a data structure that keeps track of your
game's data, you can make a clear separation in your game's code that actually
makes it less risky and easier to maintain. This recipe covers a few of the best
practices I've used to make working with global `State` and Usagi's live reload
as enjoyable as possible.

## Separate Data from Behavior

`State` is best used as a simple bucket for data: numbers, strings, booleans,
and tables that consist of those. When you start assigning functions or
instances of objects, the live reload doesn't work because what's in memory
doesn't refresh automatically with the changes.

If you have:

```lua
function _init()
  State = {
    player = {
      x = 10,
      y = 10
    }
  }
end

function _update()
  update_player()
end


function update_player()
  State.player.x += 1
end
```

You have your data (the player's position) separate from the behavior
(`update_player`). If you change `1` to `2`, Usagi automatically picks that up.
You might be tempted to even put `1` into `speed` so you reference it as
`State.player.speed`. This is fine in practice, but you'd need to hard reload
(<kbd>Ctrl + R</kbd>) to refresh that value if you change it in `_init`. When
values stabilize, that's a fine thing to do.

You might be tempted to do something like this:

```lua
function _init()
  State = {
    player = Player.new(10, 10)
  }
end

function _update()
  State.player:update()
end
```

But I am pretty sure Usagi's live reload won't pick up on the changes to Player
if you revise the `update()` instance function. You're mixing your data with
your behavior and putting instances in `State` isn't going to play nicely. I
also think this is difficult to debug and reason about.

How I write my Usagi code is actually to pass around specific data structures
and not rely on the global `State` whenever it's possible. So something like:

```lua
function _init()
  State = {
    player = {
      x = 10,
      y = 10
    }
  }
end

function _update()
  update_player(State.player)
end


function update_player(player)
  player.x += 1
end
```

That way my code doesn't have to reach into `State` to get what it knows. It
makes the code more resilient for functions to just receive the data it needs
versus all of it.
