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
  return { title = "notetris", game_id = "com.brettmakesgames.notetris", icon = 1 }
end

local function spawn(key)
  state.piece = pieces.new(key)
  state.fall_timer = 0
  state.hold_used = false
  if board.collides(state.board, state.piece, 0, 0) then
    state.alive = false
    sfx.play("gameover")
    effects.trigger_shake(state.fx, 1, 0.25)
    usagi.save({ high_score = state.high_score })
  end
end

local function spawn_next()
  local key = state.next or pieces.pull(state.bag)
  state.next = pieces.pull(state.bag)
  spawn(key)
end

local function commit_piece()
  board.lock(state.board, state.piece)
  local rows = board.detect_full_rows(state.board)
  local n = #rows
  if n > 0 then
    state.clearing_rows = rows
    state.clear_timer = cfg.CLEAR_FLASH_DURATION
    state.piece = nil

    local gain = scoring.score_for_lines(n, state.level)
    state.score = state.score + gain
    state.lines = state.lines + n
    local prev_level = state.level
    state.level = scoring.level_for_lines(state.lines)
    if state.level > prev_level then
      sfx.play("levelup")
    end

    local cy = cfg.BOARD_Y + (rows[1] - 1) * cfg.CELL
    local cx = cfg.BOARD_X + cfg.BOARD_W / 2
    effects.add_popup(state.fx, "+" .. gain, cx, cy - 4, gfx.COLOR_YELLOW)
    effects.add_popup(state.fx, POPUP_LABELS[n], cx, cy + 6, gfx.COLOR_WHITE)

    sfx.play(n == 4 and "tetris" or "clear")
    if n == 4 then
      effects.trigger_shake(state.fx, 1, 0.1)
      local sum_y = 0
      for _, r in ipairs(rows) do
        sum_y = sum_y + (cfg.BOARD_Y + (r - 1) * cfg.CELL + cfg.CELL / 2)
      end
      effects.trigger_pulse(state.fx, (sum_y / n) / usagi.GAME_H, 0.5, 1.0)
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
  state = {
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
  state.next = pieces.pull(state.bag)
  spawn_next()
end

local function do_hold()
  if state.hold_used then
    return
  end
  local current = state.piece.key
  if state.hold then
    local swap = state.hold
    state.hold = current
    spawn(swap)
  else
    state.hold = current
    spawn_next()
  end
  state.hold_used = true
  sfx.play("hold")
end

local function try_move(dx, dy)
  if not board.collides(state.board, state.piece, dx, dy) then
    state.piece.x = state.piece.x + dx
    state.piece.y = state.piece.y + dy
    return true
  end
  return false
end

local function try_rotate(dir)
  local rot = (dir > 0) and pieces.rotate_cw(state.piece.grid) or pieces.rotate_ccw(state.piece.grid)
  -- Wall kicks: try the rotated shape at center, then nudged ±1, ±2 cols.
  local kicks = { 0, 1, -1, 2, -2 }
  for _, kx in ipairs(kicks) do
    if not board.collides(state.board, state.piece, kx, 0, rot) then
      state.piece.grid = rot
      state.piece.x = state.piece.x + kx
      sfx.play("rotate")
      return true
    end
  end
  return false
end

local function hard_drop()
  local d = board.ghost_drop_distance(state.board, state.piece)
  state.piece.y = state.piece.y + d
  state.score = state.score + d * 2
  commit_piece()
  sfx.play("drop")
end

local function step_gravity()
  if not try_move(0, 1) then
    commit_piece()
  end
end

function _update(dt)
  effects.update(state.fx, dt)
  state.t = state.t + dt
  if state.score > state.high_score then
    state.high_score = state.score
  end

  if state.clearing_rows then
    state.clear_timer = state.clear_timer - dt
    if state.clear_timer <= 0 then
      board.remove_rows(state.board, state.clearing_rows)
      state.clearing_rows = nil
      state.clear_timer = 0
      spawn_next()
    end
    return
  end

  if not state.alive then
    if input.pressed(input.BTN1) then
      _init()
    end
    return
  end

  -- Horizontal: pressed = immediate move, then DAS delay before ARR autorepeat.
  if input.pressed(input.LEFT) then
    try_move(-1, 0)
    state.move_dir = -1
    state.move_timer = -cfg.DAS
  end
  if input.pressed(input.RIGHT) then
    try_move(1, 0)
    state.move_dir = 1
    state.move_timer = -cfg.DAS
  end

  local dir = 0
  if input.down(input.LEFT) then
    dir = dir - 1
  end
  if input.down(input.RIGHT) then
    dir = dir + 1
  end
  if dir ~= 0 and dir == state.move_dir then
    state.move_timer = state.move_timer + dt
    while state.move_timer >= cfg.ARR do
      state.move_timer = state.move_timer - cfg.ARR
      if not try_move(dir, 0) then
        break
      end
    end
  else
    state.move_timer = 0
    state.move_dir = 0
  end

  if input.pressed(input.BTN1) then
    try_rotate(1)
  end
  if input.pressed(input.BTN2) then
    try_rotate(-1)
  end

  if input.pressed(input.BTN3) then
    do_hold()
    if not state.alive then
      return
    end
  end

  if input.pressed(input.UP) then
    hard_drop()
    return
  end

  if input.pressed(input.DOWN) then
    state.fall_timer = 0
  end

  local interval = scoring.gravity_interval(state.level)
  if input.down(input.DOWN) then
    interval = math.min(interval, cfg.SOFT_DROP_INTERVAL)
  end

  state.fall_timer = state.fall_timer + dt
  while state.fall_timer >= interval do
    state.fall_timer = state.fall_timer - interval
    if input.down(input.DOWN) then
      state.score = state.score + 1
    end
    step_gravity()
    if not state.alive or state.clearing_rows then
      break
    end
  end
end

local function row_is_clearing(r)
  if not state.clearing_rows then
    return false
  end
  for _, fr in ipairs(state.clearing_rows) do
    if fr == r then
      return true
    end
  end
  return false
end

function _draw(_dt)
  gfx.shader_uniform("u_time", state.t)
  gfx.shader_uniform("u_pulse", effects.pulse_value(state.fx))
  gfx.shader_uniform("u_pulse_y", state.fx.pulse_y)

  gfx.clear(gfx.COLOR_DARK_BLUE)

  -- Playfield-only shake offset: HUD stays anchored, board rattles.
  local sx, sy = effects.shake_offset(state.fx)
  local bx = cfg.BOARD_X + sx
  local by = cfg.BOARD_Y + sy

  -- Playfield: dark border, then black interior.
  gfx.rect_fill(bx - 2, by - 2, cfg.BOARD_W + 4, cfg.BOARD_H + 4, gfx.COLOR_LIGHT_GRAY)
  gfx.rect_fill(bx, by, cfg.BOARD_W, cfg.BOARD_H, gfx.COLOR_BLACK)

  -- Flash cleared rows on/off across the brief pre-removal window.
  local flash_on = state.clearing_rows and (math.floor(state.clear_timer * 30) % 2 == 0) or false

  for r = 1, cfg.ROWS do
    for c = 1, cfg.COLS do
      if state.board[r][c] ~= 0 then
        local color = state.board[r][c]
        if flash_on and row_is_clearing(r) then
          color = gfx.COLOR_WHITE
        end
        draw.cell(bx + (c - 1) * cfg.CELL, by + (r - 1) * cfg.CELL, color)
      end
    end
  end

  if state.alive and state.piece then
    local gd = board.ghost_drop_distance(state.board, state.piece)
    draw.ghost(
      state.piece.grid,
      state.piece.color,
      bx + (state.piece.x - 1) * cfg.CELL,
      by + (state.piece.y - 1 + gd) * cfg.CELL
    )
    draw.piece(
      state.piece.grid,
      state.piece.color,
      bx + (state.piece.x - 1) * cfg.CELL,
      by + (state.piece.y - 1) * cfg.CELL
    )
  end

  effects.draw_popups(state.fx, sx, sy)

  gfx.text("notetris", usagi.GAME_W - usagi.measure_text("notetris") - 10, 10, gfx.COLOR_WHITE)

  local hold_x = 56
  gfx.text("hold", hold_x, 10, gfx.COLOR_LIGHT_GRAY)
  if state.hold then
    local p = pieces.DEFS[state.hold]
    local color = state.hold_used and gfx.COLOR_DARK_GRAY or p.color
    draw.piece(p.grid, color, hold_x, 24)
  end

  -- Right-side stats.
  local new_high = state.score > state.prev_high and state.score > 0
  gfx.text("score", cfg.UI_X, 10, gfx.COLOR_LIGHT_GRAY)
  gfx.text(tostring(state.score), cfg.UI_X, 22, new_high and gfx.COLOR_GREEN or gfx.COLOR_WHITE)
  gfx.text("high", cfg.UI_X, 38, gfx.COLOR_LIGHT_GRAY)
  gfx.text(tostring(state.high_score), cfg.UI_X, 50, gfx.COLOR_PEACH)
  gfx.text("level", cfg.UI_X, 66, gfx.COLOR_LIGHT_GRAY)
  gfx.text(tostring(state.level), cfg.UI_X, 78, gfx.COLOR_WHITE)
  gfx.text("lines", cfg.UI_X, 94, gfx.COLOR_LIGHT_GRAY)
  gfx.text(tostring(state.lines), cfg.UI_X, 106, gfx.COLOR_WHITE)

  gfx.text("next", cfg.UI_X, 128, gfx.COLOR_LIGHT_GRAY)
  if state.next then
    local p = pieces.DEFS[state.next]
    draw.piece(p.grid, p.color, cfg.UI_X, 142)
  end

  if not state.alive then
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
