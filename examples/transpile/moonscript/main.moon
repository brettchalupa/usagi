export _config, _draw

_config = -> { name: "MoonScript Ex" }

_draw = (dt) ->
  gfx.clear(gfx.COLOR_BLACK)
  gfx.text("Hello MoonScript!", 10, 10, gfx.COLOR_WHITE)
  gfx.text("dt: " .. dt, 10, 32, gfx.COLOR_PEACH)
