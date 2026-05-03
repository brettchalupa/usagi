-- Bare-bones mouse UI: a button that toggles a message, plus a
-- draggable box. Shows the click-vs-drag pattern (mouse_pressed for
-- one-shot button clicks, mouse_down + offset tracking for dragging).

function _config()
  return { title = "Mouse UI" }
end

local BUTTON = { x = 12, y = 24, w = 72, h = 18 }
local MESSAGE = "Hello, mouse!"

function _init()
  State = {
    message_visible = false,
    box = { x = 160, y = 80, w = 60, h = 30 },
    -- When non-nil, holds the offset from the cursor to the box's
    -- top-left at the moment the drag started. Tracking the offset
    -- (instead of snapping the box's center to the cursor) keeps the
    -- box from jumping under the pointer when you grab it from a corner.
    drag_offset = nil,
  }
end

local function point_in_rect(px, py, rect)
  return px >= rect.x and px < rect.x + rect.w and py >= rect.y and py < rect.y + rect.h
end

function _update(_dt)
  local mx, my = input.mouse()
  local in_bounds = mx >= 0 and mx < usagi.GAME_W and my >= 0 and my < usagi.GAME_H

  -- Button: fires only on the press edge so holding the mouse down
  -- doesn't flip the message every frame.
  if in_bounds and input.mouse_pressed(input.MOUSE_LEFT) and point_in_rect(mx, my, BUTTON) then
    State.message_visible = not State.message_visible
  end

  -- Drag start: only when the press edge lands inside the box.
  if in_bounds and input.mouse_pressed(input.MOUSE_LEFT) and point_in_rect(mx, my, State.box) then
    State.drag_offset = { x = State.box.x - mx, y = State.box.y - my }
  end

  -- While held, follow the cursor with the original grab offset.
  if State.drag_offset and input.mouse_down(input.MOUSE_LEFT) then
    State.box.x = mx + State.drag_offset.x
    State.box.y = my + State.drag_offset.y
  else
    State.drag_offset = nil
  end
end

local function draw_button(rect, label, hovered, pressed)
  local fill
  if pressed then
    fill = gfx.COLOR_DARK_BLUE
  elseif hovered then
    fill = gfx.COLOR_INDIGO
  else
    fill = gfx.COLOR_DARK_GRAY
  end
  gfx.rect_fill(rect.x, rect.y, rect.w, rect.h, fill)
  gfx.rect(rect.x, rect.y, rect.w, rect.h, gfx.COLOR_WHITE)
  gfx.text(label, rect.x + 6, rect.y + 4, gfx.COLOR_WHITE)
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_DARK_PURPLE)

  local mx, my = input.mouse()
  local hover_btn = point_in_rect(mx, my, BUTTON)
  local pressed_btn = hover_btn and input.mouse_down(input.MOUSE_LEFT)
  draw_button(BUTTON, "Toggle msg", hover_btn, pressed_btn)

  if State.message_visible then
    gfx.text(MESSAGE, BUTTON.x + BUTTON.w + 8, BUTTON.y + 6, gfx.COLOR_YELLOW)
  end

  -- Box highlights when hovered or while being dragged, so the
  -- interaction State is readable without a tutorial.
  local hover_box = point_in_rect(mx, my, State.box)
  local box_color
  if State.drag_offset then
    box_color = gfx.COLOR_PINK
  elseif hover_box then
    box_color = gfx.COLOR_PEACH
  else
    box_color = gfx.COLOR_LIGHT_GRAY
  end
  gfx.rect_fill(State.box.x, State.box.y, State.box.w, State.box.h, box_color)
  gfx.rect(State.box.x, State.box.y, State.box.w, State.box.h, gfx.COLOR_WHITE)
  gfx.text("drag me", State.box.x + 8, State.box.y + 10, gfx.COLOR_BLACK)

  gfx.text("Click the button. Drag the box.", 4, 4, gfx.COLOR_WHITE)
end
