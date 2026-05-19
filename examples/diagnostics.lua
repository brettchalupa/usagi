-- diagnostics: stress the Lua GC and watch the verbose-mode
-- diagnostics line. Designed to be run with the env var set, e.g.
--
--   USAGI_VERBOSE=1 just example diagnostics
--
-- Under verbose mode the engine emits a one-shot startup snapshot
-- (build profile, GC params, resolution, palette / font source, lua
-- heap after init) and a rolling per-second frame summary
-- (avg / p50 / p99 / max ms, over-budget count, lua heap KB).
--
-- This example creates short-lived tables in `_update` so the GC has
-- real work to do. `up` / `down` scale the alloc rate so you can find
-- the point where your machine starts dropping frames. A regression
-- to the GC tuning (e.g. accidentally zeroing pause / stepmul /
-- stepsize) shows up immediately as the per-second frame avg jumping
-- 2-3x while the heap stays pinned near the post-init baseline.
--
-- Controls:
--   up    increase allocs/frame (+500)
--   down  decrease allocs/frame (-500, floor 0)
--   btn1  one-shot burst of 50k allocs (provokes a GC cycle)

local ALLOC_STEP = 500
local BURST_SIZE = 50000

function _config()
  return {
    name = "Diagnostics",
    game_id = "com.usagi.diagnostics",
  }
end

function _init()
  State = {
    allocs_per_frame = 2000,
    last_burst_t = -1,
    cooldown = 0,
    -- Hold onto a few survivors so the heap has a steady-state
    -- working set, not just per-frame churn. Otherwise the GC can
    -- always collect everything between frames and the diagnostics
    -- heap reading stays flat.
    survivors = {},
    survivor_cap = 200,
  }
end

local function alloc_some(n)
  -- Build N short-lived tables. The function returns nothing, so
  -- everything inside is unreachable on return and eligible for
  -- collection. A handful escape into State.survivors below.
  for i = 1, n do
    local t = { x = i, y = -i, label = "row " .. i, tags = { "a", "b", "c" } }
    -- Keep ~1 in 50 alive to grow the heap working set.
    if i % 50 == 0 then
      State.survivors[#State.survivors + 1] = t
      if #State.survivors > State.survivor_cap then
        table.remove(State.survivors, 1)
      end
    end
  end
end

function _update(dt)
  if State.cooldown > 0 then
    State.cooldown = State.cooldown - dt
  else
    if input.pressed(input.UP) then
      State.allocs_per_frame = State.allocs_per_frame + ALLOC_STEP
    elseif input.pressed(input.DOWN) then
      State.allocs_per_frame = math.max(0, State.allocs_per_frame - ALLOC_STEP)
    elseif input.pressed(input.BTN1) then
      alloc_some(BURST_SIZE)
      State.last_burst_t = usagi.elapsed
      State.cooldown = 0.15
    end
  end
  alloc_some(State.allocs_per_frame)
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_BLACK)
  gfx.text("usagi diagnostics", 6, 6, gfx.COLOR_WHITE)
  gfx.text("set USAGI_VERBOSE=1 to see snapshot + frame summary", 6, 22, gfx.COLOR_LIGHT_GRAY)

  local y = 50
  gfx.text("allocs/frame: " .. State.allocs_per_frame, 6, y, gfx.COLOR_WHITE)
  y = y + 14
  gfx.text("survivors held: " .. #State.survivors, 6, y, gfx.COLOR_WHITE)
  y = y + 14
  if State.last_burst_t > 0 then
    local since = usagi.elapsed - State.last_burst_t
    gfx.text(string.format("last burst: %.1fs ago", since), 6, y, gfx.COLOR_WHITE)
  end

  y = usagi.GAME_H - 26
  gfx.text("up/down: alloc rate", 6, y, gfx.COLOR_LIGHT_GRAY)
  gfx.text("btn1: burst " .. BURST_SIZE, 6, y + 12, gfx.COLOR_LIGHT_GRAY)
end
