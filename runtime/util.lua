-- Small drop-in math/geometry helpers. Embedded in the engine and
-- available globally as `util` (no `require` needed). Pure Lua, no
-- engine state, easy to fork or override: assign `util.clamp = ...`
-- in your own code if you want different semantics.
--
-- Functions that take tables with required fields (vectors, rects,
-- circles) shape-check their args and raise an error pointing at the
-- caller's line, so a typo like `util.rect_overlap({x=0, y=0, w=10})`
-- (missing `h`) fails fast with a useful message instead of a silent
-- nil-arithmetic explosion deep inside the helper.
--
-- Does not implement functions that Lua provides, like in `math`.

local util = {}

-- Helpers that take tables call this with the expected field list.
-- Errors at level 3 so the reported source line is the user's call
-- site (level 1 = inside this helper, level 2 = inside the util
-- function that called it, level 3 = the user).
local function assert_shape(value, fields, fn_name, arg_idx)
  if type(value) ~= "table" then
    error(
      string.format(
        "util.%s: arg %d must be a table, got %s",
        fn_name, arg_idx, type(value)
      ),
      3
    )
  end
  for _, f in ipairs(fields) do
    if type(value[f]) ~= "number" then
      error(
        string.format(
          "util.%s: arg %d table missing or non-numeric field '%s'",
          fn_name, arg_idx, f
        ),
        3
      )
    end
  end
end

-- Clamps `v` into [lo, hi]. No assertion when `lo > hi` -- that's a
-- caller bug.
function util.clamp(v, lo, hi)
  if v < lo then return lo end
  if v > hi then return hi end
  return v
end

-- Returns -1, 0, or 1 according to the sign of `v`. Lua doesn't have
-- a built-in for this; reach for it for facing direction, AI "which
-- side of me," collision response, etc.
function util.sign(v)
  if v > 0 then return 1 end
  if v < 0 then return -1 end
  return 0
end

-- Half-up rounding to the nearest integer. Pixel snapping is the
-- driving use case in 2D pixel-art games -- pass world-space floats
-- through this on draw to keep sprites crisp instead of drifting to
-- subpixel positions.
function util.round(v)
  return math.floor(v + 0.5)
end

-- Moves `current` toward `target` by at most `max_delta`, never
-- overshooting. Per-frame smoothing primitive used in nearly every
-- 2D game (deceleration, AI chase, easing speed up to a max). Pass
-- a delta scaled by dt for frame-rate independence:
--   p.vx = util.approach(p.vx, target_vx, accel * dt)
function util.approach(current, target, max_delta)
  if current < target then
    return math.min(current + max_delta, target)
  elseif current > target then
    return math.max(current - max_delta, target)
  end
  return current
end

-- Boolean from time. Toggles `hz` times per second -- the on/off
-- interval is `1/hz` seconds. For invincibility flicker, UI blinks,
-- low-health warnings.
function util.flash(t, hz)
  return math.floor(t * hz) % 2 == 0
end

-- Linear interpolation. `t = 0` returns `a`, `t = 1` returns `b`. `t`
-- outside [0, 1] extrapolates (no clamping).
function util.lerp(a, b, t)
  return a + (b - a) * t
end

-- Wraps `v` into [lo, hi). Useful for cyclic values like angles or
-- looped indexing. Lua's modulo follows the divisor sign so this
-- works for negatives too: util.wrap(-1, 0, 4) == 3.
function util.wrap(v, lo, hi)
  local span = hi - lo
  return ((v - lo) % span) + lo
end

-- Normalizes the {x, y} vector to unit length. Returns a *new* table;
-- input is unchanged. A zero vector returns {x = 0, y = 0} rather
-- than dividing by zero.
function util.vec_normalize(v)
  assert_shape(v, { "x", "y" }, "vec_normalize", 1)
  local len = math.sqrt(v.x * v.x + v.y * v.y)
  if len == 0 then return { x = 0, y = 0 } end
  return { x = v.x / len, y = v.y / len }
end

-- Distance between two `{x, y}` points. Used everywhere: AI awareness
-- ranges, projectile homing, pickup pull radius, tooltips.
function util.vec_dist(a, b)
  assert_shape(a, { "x", "y" }, "vec_dist", 1)
  assert_shape(b, { "x", "y" }, "vec_dist", 2)
  local dx = a.x - b.x
  local dy = a.y - b.y
  return math.sqrt(dx * dx + dy * dy)
end

-- Squared distance between two `{x, y}` points. Cheaper than
-- `vec_dist` because it skips the sqrt; use it when you only need
-- to compare distances ("is X closer than Y?") -- compare against
-- the squared threshold (`r * r`).
function util.vec_dist_sq(a, b)
  assert_shape(a, { "x", "y" }, "vec_dist_sq", 1)
  assert_shape(b, { "x", "y" }, "vec_dist_sq", 2)
  local dx = a.x - b.x
  local dy = a.y - b.y
  return dx * dx + dy * dy
end

-- Builds a vector at `angle` (radians) with magnitude `len`. `len`
-- defaults to 1 for a unit vector. Pair with `math.atan(dy, dx)` to
-- spawn projectiles in a direction, emit particles in a cone, or
-- convert any angle into a velocity.
function util.vec_from_angle(angle, len)
  len = len or 1
  return { x = math.cos(angle) * len, y = math.sin(angle) * len }
end

-- AABB overlap. True when the two rects share interior area;
-- rects sharing only an edge or corner are considered non-overlapping.
function util.rect_overlap(a, b)
  assert_shape(a, { "x", "y", "w", "h" }, "rect_overlap", 1)
  assert_shape(b, { "x", "y", "w", "h" }, "rect_overlap", 2)
  return a.x < b.x + b.w
      and b.x < a.x + a.w
      and a.y < b.y + b.h
      and b.y < a.y + a.h
end

-- Circle-vs-circle overlap. Tangent circles are non-overlapping.
function util.circ_overlap(a, b)
  assert_shape(a, { "x", "y", "r" }, "circ_overlap", 1)
  assert_shape(b, { "x", "y", "r" }, "circ_overlap", 2)
  local dx = a.x - b.x
  local dy = a.y - b.y
  local rsum = a.r + b.r
  return dx * dx + dy * dy < rsum * rsum
end

-- Circle-vs-rect overlap via closest-point method: clamp the circle
-- center to the rect, then test the distance against the radius.
function util.circ_rect_overlap(c, r)
  assert_shape(c, { "x", "y", "r" }, "circ_rect_overlap", 1)
  assert_shape(r, { "x", "y", "w", "h" }, "circ_rect_overlap", 2)
  local cx = util.clamp(c.x, r.x, r.x + r.w)
  local cy = util.clamp(c.y, r.y, r.y + r.h)
  local dx = c.x - cx
  local dy = c.y - cy
  return dx * dx + dy * dy < c.r * c.r
end

return util
