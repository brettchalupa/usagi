-- Classic snake. Grid is COLS x ROWS cells at CELL pixels each.
-- Direction input is edge-detected; the snake advances on a fixed TICK,
-- so frame rate doesn't affect gameplay speed.

local CELL = 10
local COLS = 32 -- usagi.GAME_W / CELL
local ROWS = 18 -- usagi.GAME_H / CELL
local TICK = 0.12

function _config()
  return { title = "Snake" }
end

local function die()
  sfx.play("explosion")
  state.alive = false
end

function _init()
  state = {
    snake = { { x = 16, y = 9 }, { x = 15, y = 9 }, { x = 14, y = 9 } },
    dir = { x = 1, y = 0 },
    next_dir = { x = 1, y = 0 },
    food = { x = 24, y = 9 },
    timer = 0,
    alive = true,
    score = 0,
  }
end

local function read_input()
  -- Only accept perpendicular turns, so the snake can't reverse onto itself.
  if input.pressed(input.LEFT) and state.dir.x == 0 then
    state.next_dir = { x = -1, y = 0 }
  elseif input.pressed(input.RIGHT) and state.dir.x == 0 then
    state.next_dir = { x = 1, y = 0 }
  elseif input.pressed(input.UP) and state.dir.y == 0 then
    state.next_dir = { x = 0, y = -1 }
  elseif input.pressed(input.DOWN) and state.dir.y == 0 then
    state.next_dir = { x = 0, y = 1 }
  end
end

local function place_food()
  while true do
    local x = math.random(0, COLS - 1)
    local y = math.random(0, ROWS - 1)
    local on_snake = false
    for _, seg in ipairs(state.snake) do
      if seg.x == x and seg.y == y then
        on_snake = true
        break
      end
    end
    if not on_snake then
      state.food = { x = x, y = y }
      return
    end
  end
end

local function step()
  state.dir = state.next_dir
  local head = state.snake[1]
  local new_head = { x = head.x + state.dir.x, y = head.y + state.dir.y }

  if new_head.x < 0 or new_head.x >= COLS or new_head.y < 0 or new_head.y >= ROWS then
    die()
    return
  end
  for _, seg in ipairs(state.snake) do
    if seg.x == new_head.x and seg.y == new_head.y then
      return
    end
  end

  table.insert(state.snake, 1, new_head)
  if new_head.x == state.food.x and new_head.y == state.food.y then
    state.score = state.score + 1
    sfx.play("eat")
    place_food()
  else
    table.remove(state.snake)
  end
end

function _update(dt)
  if not state.alive then
    if input.pressed(input.CONFIRM) then
      _init()
    end
    return
  end

  read_input()
  state.timer = state.timer + dt
  while state.timer >= TICK do
    state.timer = state.timer - TICK
    step()
    if not state.alive then
      break
    end
  end
end

function _draw(dt)
  gfx.clear(gfx.COLOR_ORANGE)

  gfx.rect_fill(state.food.x * CELL, state.food.y * CELL, CELL, CELL, gfx.COLOR_RED)

  for i, seg in ipairs(state.snake) do
    local color = (i == 1) and gfx.COLOR_GREEN or gfx.COLOR_DARK_GREEN
    gfx.rect_fill(seg.x * CELL, seg.y * CELL, CELL, CELL, color)
  end

  gfx.text("score " .. state.score, 4, 4, gfx.COLOR_WHITE)

  if not state.alive then
    gfx.text("game over", 128, 80, gfx.COLOR_RED)
    gfx.text("press CONFIRM", 116, 96, gfx.COLOR_WHITE)
  end
end
