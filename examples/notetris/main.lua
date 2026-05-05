-- Tetris-alike. 10x20 board, 7-bag piece randomizer, ghost piece,
-- DAS/ARR horizontal autorepeat, soft + hard drop. Logic split across
-- siblings: config, scoring, pieces, board, effects, draw.
--
-- LEFT/RIGHT  move          (auto-repeats after DAS)
-- DOWN        soft drop     (held)
-- UP          hard drop
-- BTN1        rotate CW
-- BTN2        rotate CCW
-- BTN3        hold (swap active piece with held; once per piece)

local cfg = require("config")
local pieces = require("pieces")
local board = require("board")
local scoring = require("scoring")
local effects = require("effects")
local draw = require("draw")

local POPUP_LABELS = { "single", "double", "triple", "tetris" }

function _config()
  return { name = "notetris", game_id = "com.brettmakesgames.notetris", icon = 1 }
end

local function spawn(key)
  State.piece = pieces.new(key)
  State.fall_timer = 0
  State.hold_used = false
  if board.collides(State.board, State.piece, 0, 0) then
    State.alive = false
    sfx.play("gameover")
    effect.screen_shake(0.25, 1)
    usagi.save({ high_score = State.high_score })
  end
end

local function spawn_next()
  local key = State.next or pieces.pull(State.bag)
  State.next = pieces.pull(State.bag)
  spawn(key)
end

local function commit_piece()
  board.lock(State.board, State.piece)
  local rows = board.detect_full_rows(State.board)
  local n = #rows
  if n > 0 then
    State.clearing_rows = rows
    State.clear_timer = cfg.CLEAR_FLASH_DURATION
    State.piece = nil

    local gain = scoring.score_for_lines(n, State.level)
    State.score = State.score + gain
    State.lines = State.lines + n
    local prev_level = State.level
    State.level = scoring.level_for_lines(State.lines)
    if State.level > prev_level then
      sfx.play("levelup")
    end

    local cy = cfg.BOARD_Y + (rows[1] - 1) * cfg.CELL
    local cx = cfg.BOARD_X + cfg.BOARD_W / 2
    effects.add_popup(State.fx, "+" .. gain, cx, cy - 4, gfx.COLOR_YELLOW)
    effects.add_popup(State.fx, POPUP_LABELS[n], cx, cy + 6, gfx.COLOR_WHITE)

    sfx.play(n == 4 and "tetris" or "clear")
    if n == 4 then
      effect.screen_shake(0.1, 1)
      local sum_y = 0
      for _, r in ipairs(rows) do
        sum_y = sum_y + (cfg.BOARD_Y + (r - 1) * cfg.CELL + cfg.CELL / 2)
      end
      effects.trigger_pulse(State.fx, (sum_y / n) / usagi.GAME_H, 0.5, 1.0)
    end
  else
    sfx.play("lock")
    spawn_next()
  end
end

function _init()
  music.loop("korobeiniki")
  gfx.shader_set("notetris")

  local saved = usagi.load()
  local prev_high = (saved and saved.high_score) or 0
  State = {
    board = board.new(),
    bag = pieces.new_bag(),
    next = nil,
    piece = nil,
    hold = nil,
    hold_used = false,
    fall_timer = 0,
    move_timer = 0,
    move_dir = 0,
    score = 0,
    high_score = prev_high,
    prev_high = prev_high,
    t = 0,
    lines = 0,
    level = 1,
    alive = true,
    clearing_rows = nil,
    clear_timer = 0,
    fx = effects.new(),
  }
  State.next = pieces.pull(State.bag)
  spawn_next()
end

local function do_hold()
  if State.hold_used then
    return
  end
  local current = State.piece.key
  if State.hold then
    local swap = State.hold
    State.hold = current
    spawn(swap)
  else
    State.hold = current
    spawn_next()
  end
  State.hold_used = true
  sfx.play("hold")
end

local function try_move(dx, dy)
  if not board.collides(State.board, State.piece, dx, dy) then
    State.piece.x = State.piece.x + dx
    State.piece.y = State.piece.y + dy
    return true
  end
  return false
end

local function try_rotate(dir)
  local rot = (dir > 0) and pieces.rotate_cw(State.piece.grid) or pieces.rotate_ccw(State.piece.grid)
  -- Wall kicks: try the rotated shape at center, then nudged ±1, ±2 cols.
  local kicks = { 0, 1, -1, 2, -2 }
  for _, kx in ipairs(kicks) do
    if not board.collides(State.board, State.piece, kx, 0, rot) then
      State.piece.grid = rot
      State.piece.x = State.piece.x + kx
      sfx.play("rotate")
      return true
    end
  end
  return false
end

local function hard_drop()
  local d = board.ghost_drop_distance(State.board, State.piece)
  State.piece.y = State.piece.y + d
  State.score = State.score + d * 2
  commit_piece()
  sfx.play("drop")
end

local function step_gravity()
  if not try_move(0, 1) then
    commit_piece()
  end
end

function _update(dt)
  effects.update(State.fx, dt)
  State.t = State.t + dt
  if State.score > State.high_score then
    State.high_score = State.score
  end

  if State.clearing_rows then
    State.clear_timer = State.clear_timer - dt
    if State.clear_timer <= 0 then
      board.remove_rows(State.board, State.clearing_rows)
      State.clearing_rows = nil
      State.clear_timer = 0
      spawn_next()
    end
    return
  end

  if not State.alive then
    if input.pressed(input.BTN1) then
      _init()
    end
    return
  end

  -- Horizontal: pressed = immediate move, then DAS delay before ARR autorepeat.
  if input.pressed(input.LEFT) then
    try_move(-1, 0)
    State.move_dir = -1
    State.move_timer = -cfg.DAS
  end
  if input.pressed(input.RIGHT) then
    try_move(1, 0)
    State.move_dir = 1
    State.move_timer = -cfg.DAS
  end

  local dir = 0
  if input.held(input.LEFT) then
    dir = dir - 1
  end
  if input.held(input.RIGHT) then
    dir = dir + 1
  end
  if dir ~= 0 and dir == State.move_dir then
    State.move_timer = State.move_timer + dt
    while State.move_timer >= cfg.ARR do
      State.move_timer = State.move_timer - cfg.ARR
      if not try_move(dir, 0) then
        break
      end
    end
  else
    State.move_timer = 0
    State.move_dir = 0
  end

  if input.pressed(input.BTN1) then
    try_rotate(1)
  end
  if input.pressed(input.BTN2) then
    try_rotate(-1)
  end

  if input.pressed(input.BTN3) then
    do_hold()
    if not State.alive then
      return
    end
  end

  if input.pressed(input.UP) then
    hard_drop()
    return
  end

  if input.pressed(input.DOWN) then
    State.fall_timer = 0
  end

  local interval = scoring.gravity_interval(State.level)
  if input.held(input.DOWN) then
    interval = math.min(interval, cfg.SOFT_DROP_INTERVAL)
  end

  State.fall_timer = State.fall_timer + dt
  while State.fall_timer >= interval do
    State.fall_timer = State.fall_timer - interval
    if input.held(input.DOWN) then
      State.score = State.score + 1
    end
    step_gravity()
    if not State.alive or State.clearing_rows then
      break
    end
  end
end

local function row_is_clearing(r)
  if not State.clearing_rows then
    return false
  end
  for _, fr in ipairs(State.clearing_rows) do
    if fr == r then
      return true
    end
  end
  return false
end

function _draw(_dt)
  gfx.shader_uniform("u_time", State.t)
  gfx.shader_uniform("u_pulse", effects.pulse_value(State.fx))
  gfx.shader_uniform("u_pulse_y", State.fx.pulse_y)

  gfx.clear(gfx.COLOR_DARK_BLUE)

  local bx = cfg.BOARD_X
  local by = cfg.BOARD_Y

  -- Playfield: dark border, then black interior.
  gfx.rect_fill(bx - 2, by - 2, cfg.BOARD_W + 4, cfg.BOARD_H + 4, gfx.COLOR_LIGHT_GRAY)
  gfx.rect_fill(bx, by, cfg.BOARD_W, cfg.BOARD_H, gfx.COLOR_BLACK)

  -- Flash cleared rows on/off across the brief pre-removal window.
  local flash_on = State.clearing_rows and (math.floor(State.clear_timer * 30) % 2 == 0) or false

  for r = 1, cfg.ROWS do
    for c = 1, cfg.COLS do
      if State.board[r][c] ~= 0 then
        local color = State.board[r][c]
        if flash_on and row_is_clearing(r) then
          color = gfx.COLOR_WHITE
        end
        draw.cell(bx + (c - 1) * cfg.CELL, by + (r - 1) * cfg.CELL, color)
      end
    end
  end

  if State.alive and State.piece then
    local gd = board.ghost_drop_distance(State.board, State.piece)
    draw.ghost(
      State.piece.grid,
      State.piece.color,
      bx + (State.piece.x - 1) * cfg.CELL,
      by + (State.piece.y - 1 + gd) * cfg.CELL
    )
    draw.piece(
      State.piece.grid,
      State.piece.color,
      bx + (State.piece.x - 1) * cfg.CELL,
      by + (State.piece.y - 1) * cfg.CELL
    )
  end

  effects.draw_popups(State.fx)

  gfx.text("notetris", usagi.GAME_W - usagi.measure_text("notetris") - 10, 10, gfx.COLOR_WHITE)

  local hold_x = 56
  gfx.text("hold", hold_x, 10, gfx.COLOR_LIGHT_GRAY)
  if State.hold then
    local p = pieces.DEFS[State.hold]
    local color = State.hold_used and gfx.COLOR_DARK_GRAY or p.color
    draw.piece(p.grid, color, hold_x, 24)
  end

  -- Right-side stats.
  local new_high = State.score > State.prev_high and State.score > 0
  gfx.text("score", cfg.UI_X, 10, gfx.COLOR_LIGHT_GRAY)
  gfx.text(tostring(State.score), cfg.UI_X, 22, new_high and gfx.COLOR_GREEN or gfx.COLOR_WHITE)
  gfx.text("high", cfg.UI_X, 38, gfx.COLOR_LIGHT_GRAY)
  gfx.text(tostring(State.high_score), cfg.UI_X, 50, gfx.COLOR_PEACH)
  gfx.text("level", cfg.UI_X, 66, gfx.COLOR_LIGHT_GRAY)
  gfx.text(tostring(State.level), cfg.UI_X, 78, gfx.COLOR_WHITE)
  gfx.text("lines", cfg.UI_X, 94, gfx.COLOR_LIGHT_GRAY)
  gfx.text(tostring(State.lines), cfg.UI_X, 106, gfx.COLOR_WHITE)

  gfx.text("next", cfg.UI_X, 128, gfx.COLOR_LIGHT_GRAY)
  if State.next then
    local p = pieces.DEFS[State.next]
    draw.piece(p.grid, p.color, cfg.UI_X, 142)
  end

  if not State.alive then
    local msg = "game over"
    local w = usagi.measure_text(msg)
    local box_y = by + cfg.BOARD_H / 2 - 22
    gfx.rect_fill(bx, box_y, cfg.BOARD_W, 44, gfx.COLOR_BLACK)
    gfx.rect(bx, box_y, cfg.BOARD_W, 44, gfx.COLOR_RED)
    gfx.text(msg, bx + (cfg.BOARD_W - w) / 2, box_y + 6, gfx.COLOR_RED)
    local hint = "btn1: retry"
    local w2 = usagi.measure_text(hint)
    gfx.text(hint, bx + (cfg.BOARD_W - w2) / 2, box_y + 24, gfx.COLOR_WHITE)
  end
end
