# Porting to Love2D

Usagi makes it easy to port your game to Love2D, another Lua game programming
engine/toolkit/framework. This is desirable if you build a fun prototype with
Usagi but then you want to expand the game's capabilities and platforms. For
example, if you made a game you want to run on iOS and Android. Or integrate
with Steam. Or have multiplayer support. The sky's the limit with Love2D!

Usagi comes with a command to amke this as easy as possible: `usagi loveify`

`usagi loveify` sets up your game to run in Love2D by outputting a shim to
translate Usagi function calls to use Love2D's API. It also expands some of the
compound assignment operators that Love2D does not support. After you run that
command, you'll end up with a Love game you can run.

If you have an Usagi game in the `mygame` folder, you'd run:
`usagi loveify mygame mygame_love`. The `./mygame_love` folder will contain your
game converted to Love2D. Then run`love mygame_love` to boot the Love2D version.

`usagi loveify` is intended to be done once when you want to move you project
from Usagi to Love2D. It comes with some constraints and limitations: there's no
Pause menu, no input mapping, and some APIs don't work. Live reload and
cross-platform export aren't nearly as easy either with Love2D. You lose some of
the nicities of Usagi but gain lots of power. This might be a good tradeoff for
you and your game!

Read the full details on the `usagi loveify` command and shim:
[https://codeberg.org/brettchalupa/usagi/src/branch/main/examples/loveify#usagi-loveify-shim](https://codeberg.org/brettchalupa/usagi/src/branch/main/examples/loveify#usagi-loveify-shim)

In summary, `usagi loveify` is a good fit if you want:

- To integrate with Steam
- Release your game on iOS and Android
- Integrate networking
- More advanced rendering and input functionality
