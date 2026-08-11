(fn _draw [dt]
  (gfx.clear gfx.COLOR_BLACK)
  (gfx.text "Hello Fennel!" 10 10 gfx.COLOR_WHITE)
  (gfx.text (.. "dt: " dt) 10 32 gfx.COLOR_PEACH))

(set _G._draw _draw)
