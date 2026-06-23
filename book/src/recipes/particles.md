# Particles

Particles in games are circles or square shapes that move and expire, fading
away. They're a great way to add polish to your game. You could use particles to
show an explosion or as thrusters from a plane's engines. Here's some Lua code
you can drop into your game at `particle_manager.lua` to add easy particle
generation and drawing:

```lua
local ParticleManager = {}

-- Trigger an explosion at the specified location. `num` is how many circles to spawn, defaults to 12.
function ParticleManager.explosion(x, y, num)
  num = num or 12
  ParticleManager.spawn(x, y, {
    num = num,
    colors = { gfx.COLOR_WHITE, gfx.COLOR_YELLOW, gfx.COLOR_RED, gfx.COLOR_ORANGE, gfx.COLOR_PEACH },
    speed_range = { 60, 90 },
    angle_range = { 0, 360 },
    lifetime_range = { 0.2, 0.8 },
    radius_range = { 6, 12 },
  })
end

function ParticleManager.spawn(x, y, opts)
  for i = 1, opts.num do
    local angle_start = opts.angle_range[1]
    local angle_end = opts.angle_range[2] or angle_start
    local angle_rand = 0
    if angle_start ~= angle_end then
      angle_rand = math.random() / 2
      if math.random() < 0.5 then
        angle_rand *= -1
      end
    end
    local lifetime = math.random() * (opts.lifetime_range[2] - opts.lifetime_range[1]) + opts.lifetime_range[1]

    table.insert(State.particles, {
      angle = angle_rand + math.rad(angle_start + (i * ((angle_end - angle_start) / opts.num))),
      color = opts.colors[math.random(1, #opts.colors)],
      speed = math.random(opts.speed_range[1], opts.speed_range[2]),
      r = math.random(opts.radius_range[1], opts.radius_range[2]),
      lifetime = lifetime,
      life = lifetime,
      x = x,
      y = y,
    })
  end
end

function ParticleManager.update(dt)
  for i = #State.particles, 1, -1 do
    local particle = State.particles[i]
    particle.life -= dt

    if particle.life > 0 then
      particle.x += math.cos(particle.angle) * particle.speed * dt
      particle.y += math.sin(particle.angle) * particle.speed * dt
    else
      table.remove(State.particles, i)
    end
  end
end

function ParticleManager.draw()
  for _, particle in ipairs(State.particles) do
    local r = particle.r * (particle.life / particle.lifetime)
    if r >= 0.5 then
      gfx.circ_fill(particle.x, particle.y, r, particle.color)
    end
  end
end

return ParticleManager
```

This code contains four functions:

- `ParticleManager.explosion` - shows a bunch of circles at a point, with option
  number of circles to show; convenient helper
- `ParticleManager.spawn` - function to call to spawn particles given a bunch of
  parameters in the `opts` argument
- `ParticleManager.update` - call this every frame in `_update`
- `ParticleManager.draw` - call this every frame in `_draw`

The `ParticleManager` module expects `State.particles` to exist as a table,
which is used to keep track of the particles.

Here's an example of how to use it:

```lua
ParticleManager = require("particle_manager")

function _init()
  State = {
    particles = {}
  }
end

function _update(dt)
  ParticleManager.update(dt)

  if input.pressed(input.BTN1) then
    ParticleManager.explosion(40, 40)
  end
end

function _draw(dt)
  gfx.clear(gfx.COLOR_BLACK)
  ParticleManager.draw()
end
```

You can see that in `particle_manager.lua`, the `explosion` function calls out
to `spawn`, which can give you an example of how to write custom spawners and
all the arguements supported.
