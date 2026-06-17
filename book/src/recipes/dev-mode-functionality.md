# Dev Mode Functionality

Usagi provides a simple boolean that represents whether or not your game is
running in dev mode (`usagi dev`) or release mode (`usagi run` or just the
player running the executable): `usagi.IS_DEV`

When making games, it is so incredibly helpful to add dev mode functionality
that makes developing your game easier. Dev mode-specific functionality could be
things like:

- Jump between levels when you press a keyboard key
- Make the player invincible
- Show hitboxes
- Skip tedious stuff

All you have to do is:

```lua
if usagi.IS_DEV then
  gfx.text("HP: ", State.player.hp, 10, 10, gfx.COLOR_RED)
end
```

Here's an example from one of my games where I use it to draw a hitbox,
toggleable with <kbd>0</kbd>:

```lua
-- _init
State.draw_debug = false

-- _update
if usagi.IS_DEV then
  if input.key_pressed(input.KEY_0) then
    State.draw_debug = not State.draw_debug
  end
end

-- _draw
if usagi.IS_DEV then
  if State.draw_debug then
    gfx.circ(e.x, e.y, e.r, Color.RED)
  end
end
```

📺
[Watch a video tutorial I made on this topic!](https://www.youtube.com/watch?v=s4xrrMpynXw)
