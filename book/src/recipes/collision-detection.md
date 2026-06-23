# Collision Detection

Usagi provides a few utility functions to help make collision detection easier:

```
util.point_in_rect(p, r)
util.point_in_circ(p, c)
util.rect_overlap(a, b)
util.circ_overlap(a, b)
util.circ_rect_overlap(c, r)
```

They expect tables to represent each of the shapes. Here's an example of how you
could check if a bullet overlaps with a player:

```lua
local player = { x = 84, y = 160, w = 32, h = 32 }
local bullet = { x = 80, y = 120, w = 16, h = 16 }

if util.rect_overlap(player, bullet) then
  player.alive = false
  bullet.alive = false
end
```

It can be helpful to separate the hitbox from the entities you're checking
overlap of if it should be smaller or larger. You could do something like this:

```lua
local player = { x = 84, y = 160, w = 32, h = 32 }
local bullet = { x = 80, y = 120, w = 16, h = 16 }

-- player's hitbox is a small square in the center
local function player_hitbox(p)
  local s = 4
  return {
    x = p.x + p.w / 2 + s / 2,
    y = p.y + p.h / 2 + s / 2,
    w = s,
    h = s,
  }
end

if util.rect_overlap(player_hitbox(player), bullet) then
  player.alive = false
  bullet.alive = false
end
```

[View the full documentation of `util`.](https://usagiengine.com/#util)
