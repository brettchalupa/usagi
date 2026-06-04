# Coming from Pico-8

Usagi's API is heavily inspired by Pico-8's. The core idea behind the creation
of Usagi Engine was: _what if Pico-8 had live reload and a little bit more
functionality?_

While the APIs are similar, they're not exactly the same. Usagi plays audio
files rather than having an editor and music tracker. Usagi doesn't have a
camera or map helpers.

The main reasons to use Usagi instead of Pico-8 (or even Picotron) is that Usagi
has live reload of your code and assets. As you change them, they automatically
update in your game. This leads to ultra fast iteration speeds, which is
essential for game dev.

## Pico-8 Shim Example

If you want to use Usagi with functions that are as close to Pico-8's as
possible, look at
[the Pico-8 example](https://github.com/brettchalupa/usagi/tree/main/examples/pico8).

You'd drop in
[`pico8.lua`](https://github.com/brettchalupa/usagi/blob/main/examples/pico8/pico8.lua)
into your project and `require "pico8"`. This lets you write Usagi games with an
API like this:

```lua
function _update(dt)
  if btn(0) then
    State.p.x = State.p.x - State.p.spd * dt
    State.p.face_left = true
  end
  if btn(1) then
    State.p.x = State.p.x + State.p.spd * dt
    State.p.face_left = false
  end
  if btn(2) then
    State.p.y = State.p.y - State.p.spd * dt
  end
  if btn(3) then
    State.p.y = State.p.y + State.p.spd * dt
  end
  if btnp(4) then
    State.count += 1
  end
end

function _draw(_dt)
  cls(1)

  rectfill(0, 0, usagi.GAME_W - 1, 13, 0)
  line(0, 14, usagi.GAME_W - 1, 14, 5)
  print("pico-8 flavor", 2, 1, 15)
  print("count: " .. State.count, 200, 1, 10)
end
```

The Pico-8 shim is not 100% of Pico-8's functions, as Usagi has differing
functionality at some points. The way music is played differs. Usagi has no
camera. It's all documented at the top of `pico8.lua`.

The shim is meant more for comfort, not for having it be extremely easy to port
your Pico-8 game to Usagi. But if you did want to do that, it wouldn't be too
difficult of a port dependent on what your game utilizes.

## Exporting Pico-8 Assets

Pico-8 makes it easy for you to get your spritesheets, music, and sound effects
exported into a common format that other programs can use. What's nice is that
these files work really well with Usagi.

### Export sprites.png

In Pico-8, run this command:

```
export sprites.png
```

Then you can use:

```
folder
```

to find your new `sprites.png` file. You can copy or move it into your Usagi
game project.

Pico-8's spritesheet is 8px by 8px by default. So in your Usagi `main.lua`,
update your `_config()` function to set the `sprite_size` accordingly:

```lua
function _config()
  return { sprite_size = 8 } -- other options omitted
end
```

Alternatively, if you want your sprites larger or want to stick to Usagi's
defaulty 16px by 16px size, you could double the size of your `sprites.png` in
an image editor.

### Export Music

In Pico-8, open the music editor and press <kbd>Esc</kbd>. Then run this
command:

```
export music_%d.wav
```

Use `folder` to open them in your operating system's file explorer. And then
copy them into your Usagi game's `music` directory. It exports all tracks. Now
you can play those sound effects with `music.loop("music_1")`, etc.

### Export Sound Effects

In Pico-8, open the sound effect editor and press <kbd>Esc</kbd>. Then run this
command:

```
export sfx_%d.wav
```

Use `folder` to open them in your operating system's file explorer. And then
copy them into your Usagi game's `sfx` directory. It exports all 64 sound effect
slots. You can delete the ones that don't play any sound. Now you can play those
sound effects with `sfx.play("sfx_1")`, etc.
