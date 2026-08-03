# Snake

Snake is great for learning how to code games. It's simple yet fun. It deals
with grid-based movement, which is essential for lots of types of games like
RPGs, roguelikes, and puzzle games.

The concept of Snake is simple: eat apples to grow longer, avoid the hitting
edges of the stage, and avoid biting yourself.

While in many ways Snake is simpler than what we built in
[the Shoot 'Em Up chapter](/02-shoot-em-up.html), the chapter will cover learn
concepts: sprites, high score tracking, playing music, and grid-based movement.
You can then take what you learn and apply it to the games from the previous
chapters.

## Auto-Moving Player

Run `usagi init snake` to create your new game. Open up that folder in your
editor and your terminal and start `usagi dev`. Clear out the placeholder text
drawing and set the values in `_config` appropriately.

TODO:

- Drawing the player
- Player has direction and auto-moves
- Clamping/checking when going off screen
- Game over when hitting the edge

## Eating Apples & Growing

TODO

## Sprites

TODO

## High Score Tracking

TODO

## Playing Music

TODO

## Bonus Credits

- Instead of making the snake die when it hits the edges of the stage, make the
  snake wrap around the opposite side, adding a different challenge to the game.
- Add sound effects to the game, following the process from
  [the Shoot 'Em Up chapter](/02-shoot-em-up.html#sound-effects); good events:
  eating an apple, game over, turning
