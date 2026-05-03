-- Mouse drag with positional AABB collision resolution. Drag any box;
-- it pushes the others along the minimum-translation axis. Pushes
-- cascade through stacks, and walls clamp the play area.
--
-- Pure positional resolution: no velocity, no momentum, no friction.
-- Each frame the dragged box snaps to the cursor, then a few passes
-- of pairwise overlap fixes settle the rest.

function _config()
  return { name = "Mouse Physics" }
end

function _init()
  State = {
    boxes = {
      { x = 50, y = 50, w = 28, h = 28, color = gfx.COLOR_PEACH },
      { x = 110, y = 70, w = 32, h = 22, color = gfx.COLOR_GREEN },
      { x = 170, y = 90, w = 24, h = 36, color = gfx.COLOR_PINK },
      { x = 230, y = 60, w = 30, h = 30, color = gfx.COLOR_YELLOW },
      { x = 80, y = 120, w = 26, h = 26, color = gfx.COLOR_ORANGE },
      { x = 200, y = 130, w = 28, h = 24, color = gfx.COLOR_INDIGO },
    },
    dragged = nil,
    drag_offset = nil,
  }
end

local function point_in_rect(px, py, r)
  return px >= r.x and px < r.x + r.w and py >= r.y and py < r.y + r.h
end

-- Returns the (x, y) overlap of two AABBs, or nil if they don't
-- overlap. Both components are positive when there's an overlap;
-- comparing them picks the minimum-translation axis.
local function overlap(a, b)
  local ox = math.min(a.x + a.w, b.x + b.w) - math.max(a.x, b.x)
  local oy = math.min(a.y + a.h, b.y + b.h) - math.max(a.y, b.y)
  if ox <= 0 or oy <= 0 then
    return nil
  end
  return ox, oy
end

-- Pushes `movable` out of `fixed` along whichever axis has less
-- overlap. Returns true if any movement happened (so the outer pass
-- loop knows to keep iterating).
local function push_out(movable, fixed)
  local ox, oy = overlap(movable, fixed)
  if not ox then
    return false
  end
  local m_cx = movable.x + movable.w / 2
  local m_cy = movable.y + movable.h / 2
  local f_cx = fixed.x + fixed.w / 2
  local f_cy = fixed.y + fixed.h / 2
  if ox < oy then
    if m_cx < f_cx then
      movable.x = movable.x - ox
    else
      movable.x = movable.x + ox
    end
  else
    if m_cy < f_cy then
      movable.y = movable.y - oy
    else
      movable.y = movable.y + oy
    end
  end
  return true
end

-- When two non-dragged boxes overlap each gets pushed half the
-- overlap, so neither feels privileged. Same axis-pick as push_out.
local function push_apart(a, b)
  local ox, oy = overlap(a, b)
  if not ox then
    return false
  end
  local a_cx = a.x + a.w / 2
  local a_cy = a.y + a.h / 2
  local b_cx = b.x + b.w / 2
  local b_cy = b.y + b.h / 2
  if ox < oy then
    local h = ox / 2
    if a_cx < b_cx then
      a.x = a.x - h
      b.x = b.x + h
    else
      a.x = a.x + h
      b.x = b.x - h
    end
  else
    local h = oy / 2
    if a_cy < b_cy then
      a.y = a.y - h
      b.y = b.y + h
    else
      a.y = a.y + h
      b.y = b.y - h
    end
  end
  return true
end

local function clamp_to_play_area(b)
  if b.x < 0 then
    b.x = 0
  end
  if b.y < 0 then
    b.y = 0
  end
  if b.x + b.w > usagi.GAME_W then
    b.x = usagi.GAME_W - b.w
  end
  if b.y + b.h > usagi.GAME_H then
    b.y = usagi.GAME_H - b.h
  end
end

local MAX_PASSES = 8

function _update(_dt)
  local mx, my = input.mouse()
  local in_bounds = mx >= 0 and mx < usagi.GAME_W and my >= 0 and my < usagi.GAME_H

  -- Pick a box to drag on the press edge. Iterate in reverse so the
  -- visually-on-top box (drawn last) gets the click when boxes stack.
  if in_bounds and input.mouse_pressed(input.MOUSE_LEFT) and not State.dragged then
    for i = #State.boxes, 1, -1 do
      if point_in_rect(mx, my, State.boxes[i]) then
        State.dragged = i
        State.drag_offset = {
          x = State.boxes[i].x - mx,
          y = State.boxes[i].y - my,
        }
        break
      end
    end
  end

  if State.dragged and input.mouse_down(input.MOUSE_LEFT) then
    local b = State.boxes[State.dragged]
    b.x = mx + State.drag_offset.x
    b.y = my + State.drag_offset.y
    clamp_to_play_area(b)
  else
    State.dragged = nil
    State.drag_offset = nil
  end

  -- Resolve overlaps in passes so cascades settle: dragged pushes A,
  -- A might now overlap B, B might overlap C. Capped at MAX_PASSES so
  -- a pathological wedge can't lock the frame.
  for _ = 1, MAX_PASSES do
    local any_moved = false
    for i = 1, #State.boxes - 1 do
      for j = i + 1, #State.boxes do
        local a = State.boxes[i]
        local b = State.boxes[j]
        if i == State.dragged then
          if push_out(b, a) then
            any_moved = true
          end
        elseif j == State.dragged then
          if push_out(a, b) then
            any_moved = true
          end
        else
          if push_apart(a, b) then
            any_moved = true
          end
        end
      end
    end
    if not any_moved then
      break
    end
  end

  -- Final wall clamp on non-dragged boxes. Pushed-into-wall boxes may
  -- visibly overlap the dragged one for a frame; that's the wedge
  -- case and the cursor is the explicit input source so we accept it.
  for i, b in ipairs(State.boxes) do
    if i ~= State.dragged then
      clamp_to_play_area(b)
    end
  end
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_DARK_BLUE)

  for i, b in ipairs(State.boxes) do
    gfx.rect_fill(b.x, b.y, b.w, b.h, b.color)
    -- Brighter outline on the box you're holding so the interaction
    -- State is readable while the cursor moves around.
    local outline
    if i == State.dragged then
      outline = gfx.COLOR_WHITE
    else
      outline = gfx.COLOR_DARK_GRAY
    end
    gfx.rect(b.x, b.y, b.w, b.h, outline)
  end

  gfx.text("Drag a box. Push the others around.", 4, 4, gfx.COLOR_WHITE)
end
