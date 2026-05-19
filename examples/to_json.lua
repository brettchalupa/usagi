-- usagi.to_json demo: serialize a Lua table to a pretty-printed JSON
-- string. Same shape rules as `usagi.save` (keys are all strings or a
-- dense 1..n integer array; no cycles, NaN, functions, or userdata).
--
-- Useful for devtools overlays, structured stdout logs, and any other
-- place you want JSON without writing to the save file. Reach for
-- `usagi.dump` instead when you want a forgiving pretty-print that
-- tolerates cycles and mixed-key tables.
--
-- Controls:
--   BTN1 (Z / pad-A): print the current state as JSON to stdout
--   BTN2 (X / pad-B): bump the score

function _config()
  return { name = "to_json demo", game_id = "com.usagi.tojsondemo" }
end

function _init()
  State = {
    score = 0,
    started_at = math.floor(usagi.elapsed),
    run = {
      seed = 42,
      deaths = 0,
      tags = { "rabbit", "carrot", "moon" },
    },
  }
end

function _update(dt)
  if input.pressed(input.BTN1) then
    local file = io.open("foo.json", "w")
    file:write(usagi.to_json(State))
    file:close()

    print(usagi.to_json(State))
  end

  if input.pressed(input.BTN2) then
    State.score = State.score + 10
  end
end

function _draw(dt)
  gfx.clear(gfx.COLOR_BLACK)
  gfx.text("usagi.to_json", 8, 8, gfx.COLOR_WHITE)
  gfx.text("score: " .. State.score, 8, 28, gfx.COLOR_PEACH)
  gfx.text("seed:  " .. State.run.seed, 8, 40, gfx.COLOR_PEACH)
  gfx.text("tags:  " .. State.run.tags[1] .. ", "
    .. State.run.tags[2] .. ", "
    .. State.run.tags[3], 8, 52, gfx.COLOR_PEACH)
  local b1 = input.mapping_for(input.BTN1) or "BTN1"
  local b2 = input.mapping_for(input.BTN2) or "BTN2"
  gfx.text(b1 .. " print json", 8, usagi.GAME_H - 30, gfx.COLOR_LIGHT_GRAY)
  gfx.text(b2 .. " score +10", 8, usagi.GAME_H - 18, gfx.COLOR_LIGHT_GRAY)
end
