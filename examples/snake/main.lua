-- Classic snake. Grid is COLS x ROWS cells at CELL pixels each.
-- Direction input is edge-detected; the snake advances on a fixed TICK,
-- so frame rate doesn't affect gameplay speed.

local CELL = 10
local COLS = 32 -- usagi.GAME_W / CELL
local ROWS = 18 -- usagi.GAME_H / CELL
local TICK = 0.12

function _config()
  return { title = "Snake", game_id = "com.brettmakesgames.usagisnake" }
end

local function die()
  sfx.play("explosion")
  State.alive = false
  if State.score > State.high_score then
    State.high_score = State.score
    usagi.save({ high_score = State.high_score })
  end
end

function _init()
  local save = usagi.load()
  State = {
    snake = { { x = 16, y = 9 }, { x = 15, y = 9 }, { x = 14, y = 9 } },
    dir = { x = 1, y = 0 },
    next_dir = { x = 1, y = 0 },
    food = { x = 24, y = 9 },
    timer = 0,
    alive = true,
    score = 0,
    high_score = save and (save.high_score or 0) or 0,
  }
end

local function read_input()
  -- Only accept perpendicular turns, so the snake can't reverse onto itself.
  if input.pressed(input.LEFT) and State.dir.x == 0 then
    State.next_dir = { x = -1, y = 0 }
  elseif input.pressed(input.RIGHT) and State.dir.x == 0 then
    State.next_dir = { x = 1, y = 0 }
  elseif input.pressed(input.UP) and State.dir.y == 0 then
    State.next_dir = { x = 0, y = -1 }
  elseif input.pressed(input.DOWN) and State.dir.y == 0 then
    State.next_dir = { x = 0, y = 1 }
  end
end

local function place_food()
  while true do
    local x = math.random(0, COLS - 1)
    local y = math.random(0, ROWS - 1)
    local on_snake = false
    for _, seg in ipairs(State.snake) do
      if seg.x == x and seg.y == y then
        on_snake = true
        break
      end
    end
    if not on_snake then
      State.food = { x = x, y = y }
      return
    end
  end
end

local function step()
  State.dir = State.next_dir
  local head = State.snake[1]
  local new_head = { x = head.x + State.dir.x, y = head.y + State.dir.y }

  if new_head.x < 0 or new_head.x >= COLS or new_head.y < 0 or new_head.y >= ROWS then
    die()
    return
  end
  -- Skip the tail when not growing: it'll vacate that cell on this same
  -- step, so sliding into it is fair game (classic snake rule).
  local will_grow = (new_head.x == State.food.x and new_head.y == State.food.y)
  local check_to = will_grow and #State.snake or #State.snake - 1
  for i = 1, check_to do
    local seg = State.snake[i]
    if seg.x == new_head.x and seg.y == new_head.y then
      die()
      return
    end
  end

  table.insert(State.snake, 1, new_head)
  if new_head.x == State.food.x and new_head.y == State.food.y then
    State.score = State.score + 1
    sfx.play("eat")
    place_food()
  else
    table.remove(State.snake)
  end
end

function _update(dt)
  if not State.alive then
    if input.pressed(input.BTN1) then
      _init()
    end
    return
  end

  read_input()
  State.timer = State.timer + dt
  while State.timer >= TICK do
    State.timer = State.timer - TICK
    step()
    if not State.alive then
      break
    end
  end
end

function _draw(dt)
  gfx.clear(gfx.COLOR_ORANGE)

  gfx.rect_fill(State.food.x * CELL, State.food.y * CELL, CELL, CELL, gfx.COLOR_RED)

  for i, seg in ipairs(State.snake) do
    local color = (i == 1) and gfx.COLOR_GREEN or gfx.COLOR_DARK_GREEN
    gfx.rect_fill(seg.x * CELL, seg.y * CELL, CELL, CELL, color)
  end

  local score_color = gfx.COLOR_WHITE
  local score_text = "score: " .. State.score
  if State.score > State.high_score then
    score_color = gfx.COLOR_DARK_BLUE
    score_text = score_text .. "!"
  end
  gfx.text(score_text, 4, 4, score_color)

  if not State.alive then
    local game_over_txt = "game over"
    gfx.text(game_over_txt, usagi.GAME_W / 2 - usagi.measure_text(game_over_txt) / 2, 80, gfx.COLOR_RED)
    local restart_txt = "press BTN1"
    gfx.text(restart_txt, usagi.GAME_W / 2 - usagi.measure_text(restart_txt) / 2, 96, gfx.COLOR_WHITE)
  end
end
