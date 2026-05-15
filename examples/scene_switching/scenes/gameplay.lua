local M = {}

function M.init()
  print("gameplay init")
end

function M.close()
  print("gameplay close")
end

function M.update(_dt)
  if input.pressed(input.BTN2) then
    SwitchScene("main_menu")
  end
end

function M.draw()
  gfx.text("Hello from Gameplay!", 10, 10, gfx.COLOR_WHITE)
  gfx.text("Press " .. input.mapping_for(input.BTN2) .. " to switch to Main Menu!", 10, 30, gfx.COLOR_PEACH)
end

return M
