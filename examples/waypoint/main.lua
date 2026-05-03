-- Click anywhere to drop a waypoint. The unit walks the waypoints in the order
-- they were placed, consuming each one as it arrives. Right-click clears the
-- queue.

function _config()
  return { title = "Waypoint" }
end

function _init()
  State = {
    waypoints = {},
    unit = {
      x = usagi.GAME_W / 2,
      y = usagi.GAME_H / 2,
      spd = 60,
    },
  }
end

local WAYPOINT_RADIUS = 4
local UNIT_RADIUS = 3
local ARRIVE_DISTANCE = WAYPOINT_RADIUS + UNIT_RADIUS

function _update(dt)
  local mx, my = input.mouse()
  local in_bounds = mx >= 0 and mx < usagi.GAME_W and my >= 0 and my < usagi.GAME_H

  if in_bounds and input.mouse_pressed(input.MOUSE_LEFT) then
    State.waypoints[#State.waypoints + 1] = { x = mx, y = my }
  end

  if input.mouse_pressed(input.MOUSE_RIGHT) then
    State.waypoints = {}
  end

  -- Walk toward the next waypoint at a constant speed. When close
  -- enough, pop it and the next one becomes the target. dt-based step
  -- so movement stays consistent across frame rates.
  local next_wp = State.waypoints[1]
  if next_wp then
    local dx = next_wp.x - State.unit.x
    local dy = next_wp.y - State.unit.y
    local dist = math.sqrt(dx * dx + dy * dy)
    if dist <= ARRIVE_DISTANCE then
      table.remove(State.waypoints, 1)
    else
      local step = State.unit.spd * dt
      State.unit.x = State.unit.x + (dx / dist) * step
      State.unit.y = State.unit.y + (dy / dist) * step
    end
  end
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_DARK_BLUE)

  -- Path between queued waypoints, so the player can read the route
  -- the unit will take.
  for i = 1, #State.waypoints - 1 do
    local a = State.waypoints[i]
    local b = State.waypoints[i + 1]
    gfx.line(a.x, a.y, b.x, b.y, gfx.COLOR_DARK_GRAY)
  end

  -- Live tether from the unit to its current target, in a brighter
  -- color so the immediate goal stands out from the rest of the path.
  local target = State.waypoints[1]
  if target then
    gfx.line(State.unit.x, State.unit.y, target.x, target.y, gfx.COLOR_YELLOW)
  end

  for i, w in ipairs(State.waypoints) do
    -- Highlight the active waypoint in pink; the rest are indigo. A
    -- gentle pulse on the active one, for vibes.
    local color
    if i == 1 then
      color = gfx.COLOR_PINK
    else
      color = gfx.COLOR_INDIGO
    end
    local r = WAYPOINT_RADIUS
    if i == 1 then
      r = WAYPOINT_RADIUS + math.sin(usagi.elapsed * 6) * 1.5
    end
    gfx.circ_fill(w.x, w.y, r, color)
  end

  gfx.circ_fill(State.unit.x, State.unit.y, UNIT_RADIUS, gfx.COLOR_WHITE)

  gfx.text("Left click: drop waypoint   Right click: clear", 4, 4, gfx.COLOR_WHITE)
  gfx.text("Queued: " .. #State.waypoints, 4, 14, gfx.COLOR_LIGHT_GRAY)
end
