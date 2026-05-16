local M = {}

function M.init()
  print("main_menu init")
end

function M.close()
  print("main_menu close")
end

function M.update(_dt)
  if input.pressed(input.BTN1) then
    SwitchScene("gameplay")
  end
end

function M.draw()
  gfx.text("Hello from Main Menu!", 10, 10, gfx.COLOR_WHITE)
  gfx.text("Press " .. input.mapping_for(input.BTN1) .. " to switch to Gameplay!", 10, 30, gfx.COLOR_PEACH)
end

return M
