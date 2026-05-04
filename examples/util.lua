-- Util demo: a tiny shooter scene that exercises every util function.
-- Move with arrows/WASD, BTN1 to shoot a 3-bullet spread. The red
-- enemy chases you, the wall blocks the player and bullets, the green
-- bar tracks your X position, and a tiny dot orbits the corner.
-- Hover the enemy or wall with the mouse to see them outline.
--
-- Functions covered:
--   clamp, sign, round, approach, lerp, wrap, flash
--   vec_normalize, vec_dist, vec_dist_sq, vec_from_angle
--   point_in_rect, point_in_circ
--   rect_overlap, circ_overlap, circ_rect_overlap

function _config()
  return { name = "Util Demo" }
end

local PSPEED = 80
local PACCEL = 600
local BSPEED = 140
local BLIFE = 1.5
local ESPEED = 28
local DANGER_R = 40

function _init()
  State = {
    p = { x = 160, y = 130, vx = 0, vy = 0, r = 4, fx = 1, fy = 0, hit_t = 0 },
    e = { x = 40, y = 40, r = 5 },
    wall = { x = 130, y = 60, w = 60, h = 8 },
    bullets = {},
    spinner = 0,
  }
end

function _update(dt)
  local p = State.p
  local e = State.e
  local w = State.wall

  -- Movement: read input as a direction, approach a target velocity,
  -- step axis-by-axis so the player slides along the wall instead of
  -- sticking on it.
  local tx, ty = 0, 0
  if input.held(input.LEFT)  then tx = tx - 1 end
  if input.held(input.RIGHT) then tx = tx + 1 end
  if input.held(input.UP)    then ty = ty - 1 end
  if input.held(input.DOWN)  then ty = ty + 1 end

  p.vx = util.approach(p.vx, tx * PSPEED, PACCEL * dt)
  p.vy = util.approach(p.vy, ty * PSPEED, PACCEL * dt)

  local nx = util.clamp(p.x + p.vx * dt, p.r, usagi.GAME_W - p.r)
  if util.rect_overlap({ x = nx - p.r, y = p.y - p.r, w = p.r * 2, h = p.r * 2 }, w) then
    p.vx = 0
  else
    p.x = nx
  end
  local ny = util.clamp(p.y + p.vy * dt, p.r, usagi.GAME_H - p.r)
  if util.rect_overlap({ x = p.x - p.r, y = ny - p.r, w = p.r * 2, h = p.r * 2 }, w) then
    p.vy = 0
  else
    p.y = ny
  end

  -- Facing follows input direction. vec_normalize handles the
  -- diagonal case so facing length stays at 1.
  if tx ~= 0 or ty ~= 0 then
    local n = util.vec_normalize({ x = tx, y = ty })
    p.fx, p.fy = n.x, n.y
  end

  -- Shoot: 3-bullet spread built with vec_from_angle.
  if input.pressed(input.BTN1) then
    local angle = math.atan(p.fy, p.fx)
    for _, spread in ipairs({ -0.2, 0, 0.2 }) do
      local v = util.vec_from_angle(angle + spread, BSPEED)
      table.insert(State.bullets, {
        x = p.x, y = p.y, vx = v.x, vy = v.y, r = 1, life = BLIFE,
      })
    end
  end

  -- Enemy chases via normalized direction toward the player.
  local n = util.vec_normalize({ x = p.x - e.x, y = p.y - e.y })
  e.x = e.x + n.x * ESPEED * dt
  e.y = e.y + n.y * ESPEED * dt

  -- Bullets: integrate, then test against wall (circ_rect_overlap)
  -- and enemy (circ_overlap). Knock the enemy back via vec_from_angle.
  for i = #State.bullets, 1, -1 do
    local b = State.bullets[i]
    b.x = b.x + b.vx * dt
    b.y = b.y + b.vy * dt
    b.life = b.life - dt

    local kill = b.life <= 0
    if not kill and util.circ_rect_overlap({ x = b.x, y = b.y, r = b.r }, w) then
      kill = true
    end
    if not kill and util.circ_overlap({ x = b.x, y = b.y, r = b.r }, e) then
      kill = true
      local kn = util.vec_from_angle(math.atan(e.y - b.y, e.x - b.x), 18)
      e.x = e.x + kn.x
      e.y = e.y + kn.y
    end
    if kill then table.remove(State.bullets, i) end
  end

  -- Player invincibility timer + collision (circ_overlap).
  if p.hit_t > 0 then p.hit_t = p.hit_t - dt end
  if p.hit_t <= 0 and util.circ_overlap(p, e) then
    p.hit_t = 1.5
  end

  -- Spinner uses wrap to keep the angle in [0, 2π).
  State.spinner = util.wrap(State.spinner + 2 * dt, 0, math.pi * 2)
end

function _draw(_dt)
  local p = State.p
  local e = State.e
  local w = State.wall

  gfx.clear(gfx.COLOR_DARK_BLUE)

  -- Mouse-hover hit tests: point_in_rect for the wall, point_in_circ
  -- for the enemy. Hovered shapes get a yellow outline.
  local mx, my = input.mouse()
  local mouse = { x = mx, y = my }
  local hover_wall = util.point_in_rect(mouse, w)
  local hover_enemy = util.point_in_circ(mouse, e)

  -- Wall (with hover ring).
  gfx.rect_fill(w.x, w.y, w.w, w.h, gfx.COLOR_DARK_GRAY)
  if hover_wall then
    gfx.rect(w.x - 1, w.y - 1, w.w + 2, w.h + 2, gfx.COLOR_YELLOW)
  end

  -- Enemy (with hover ring).
  gfx.circ_fill(util.round(e.x), util.round(e.y), e.r, gfx.COLOR_RED)
  if hover_enemy then
    gfx.circ(util.round(e.x), util.round(e.y), e.r + 2, gfx.COLOR_YELLOW)
  end

  -- Player (round on draw for pixel snap, flash during invincibility).
  local visible = p.hit_t <= 0 or util.flash(p.hit_t, 8)
  if visible then
    gfx.circ_fill(util.round(p.x), util.round(p.y), p.r, gfx.COLOR_WHITE)
    gfx.line(
      util.round(p.x), util.round(p.y),
      util.round(p.x + p.fx * 10), util.round(p.y + p.fy * 10),
      gfx.COLOR_PEACH
    )
    -- Enemy-direction indicator: sign(dx) gives -1 / 0 / 1 so the
    -- arrow always points the right way regardless of distance.
    local side = util.sign(e.x - p.x)
    if side ~= 0 then
      gfx.text(side > 0 and ">" or "<", util.round(p.x) - 2, util.round(p.y) - 14, gfx.COLOR_RED)
    end
  end

  -- Bullets.
  for _, b in ipairs(State.bullets) do
    gfx.circ_fill(util.round(b.x), util.round(b.y), b.r, gfx.COLOR_YELLOW)
  end

  -- Spinner: dot orbiting a fixed center via vec_from_angle + wrap.
  local sp = util.vec_from_angle(State.spinner, 8)
  gfx.pixel(util.round(300 + sp.x), util.round(20 + sp.y), gfx.COLOR_GREEN)
  gfx.text("WRAP", 286, 4, gfx.COLOR_LIGHT_GRAY)

  -- HUD: distance text uses vec_dist; danger flag uses vec_dist_sq.
  local shoot_label = input.mapping_for(input.BTN1) or "?"
  gfx.text("MOVE: WASD/ARROWS  SHOOT: " .. shoot_label, 4, 4, gfx.COLOR_LIGHT_GRAY)

  local d = util.vec_dist(p, e)
  local danger = util.vec_dist_sq(p, e) < DANGER_R * DANGER_R
  local danger_text = danger and "  DANGER" or ""
  gfx.text(string.format("DIST %d%s", util.round(d), danger_text), 4, 14, gfx.COLOR_LIGHT_GRAY)

  -- Lerp demo: a green bar whose width tracks the player's X position.
  local t = util.clamp(p.x / usagi.GAME_W, 0, 1)
  local bar_w = util.round(util.lerp(0, 80, t))
  gfx.rect(4, 168, 80, 6, gfx.COLOR_LIGHT_GRAY)
  gfx.rect_fill(4, 168, bar_w, 6, gfx.COLOR_GREEN)
  gfx.text("LERP", 88, 168, gfx.COLOR_LIGHT_GRAY)
end
