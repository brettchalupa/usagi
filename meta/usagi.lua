---@meta
-- Usagi API stubs for lua-language-server.
-- Declarations only; this file is never executed by the runtime.

---Pico-8 palette, indices 0-15.
---@class Usagi.Gfx
---@field COLOR_BLACK        integer  0
---@field COLOR_DARK_BLUE    integer  1
---@field COLOR_DARK_PURPLE  integer  2
---@field COLOR_DARK_GREEN   integer  3
---@field COLOR_BROWN        integer  4
---@field COLOR_DARK_GRAY    integer  5
---@field COLOR_LIGHT_GRAY   integer  6
---@field COLOR_WHITE        integer  7
---@field COLOR_RED          integer  8
---@field COLOR_ORANGE       integer  9
---@field COLOR_YELLOW       integer  10
---@field COLOR_GREEN        integer  11
---@field COLOR_BLUE         integer  12
---@field COLOR_INDIGO       integer  13
---@field COLOR_PINK         integer  14
---@field COLOR_PEACH        integer  15
gfx = {}

---Clears the screen to the given color.
---@param color integer  a gfx.COLOR_* constant
function gfx.clear(color) end

---Draws text at (x, y) in the given color. Uses the bundled monogram
---font at its 16px design size (a 5×7 pixel font with 16px line height).
---@param text  string  string to render
---@param x     number  left edge in game-space pixels
---@param y     number  top edge in game-space pixels
---@param color integer  a gfx.COLOR_* constant
function gfx.text(text, x, y, color) end


---Draws a rectangle outline.
---@param x     number  left edge in game-space pixels
---@param y     number  top edge in game-space pixels
---@param w     number  width in pixels
---@param h     number  height in pixels
---@param color integer  a gfx.COLOR_* constant
function gfx.rect(x, y, w, h, color) end

---Draws a filled rectangle.
---@param x     number  left edge in game-space pixels
---@param y     number  top edge in game-space pixels
---@param w     number  width in pixels
---@param h     number  height in pixels
---@param color integer  a gfx.COLOR_* constant
function gfx.rect_fill(x, y, w, h, color) end

---Draws a circle outline centered at (x, y).
---@param x     number  center x in game-space pixels
---@param y     number  center y in game-space pixels
---@param r     number  radius in pixels
---@param color integer  a gfx.COLOR_* constant
function gfx.circ(x, y, r, color) end

---Draws a filled circle centered at (x, y).
---@param x     number  center x in game-space pixels
---@param y     number  center y in game-space pixels
---@param r     number  radius in pixels
---@param color integer  a gfx.COLOR_* constant
function gfx.circ_fill(x, y, r, color) end

---Draws a line from (x1, y1) to (x2, y2).
---@param x1    number  start x in game-space pixels
---@param y1    number  start y in game-space pixels
---@param x2    number  end x in game-space pixels
---@param y2    number  end y in game-space pixels
---@param color integer  a gfx.COLOR_* constant
function gfx.line(x1, y1, x2, y2, color) end

---Sets a single pixel.
---@param x     number  x in game-space pixels
---@param y     number  y in game-space pixels
---@param color integer  a gfx.COLOR_* constant
function gfx.pixel(x, y, color) end

---Draws a 16×16 sprite from the loaded sheet at (x, y). The sheet is
---`sprites.png` next to the game's main .lua; indices run left-to-right,
---top-to-bottom. Alpha-channel pixels render as transparent.
---@param index integer  one-based sprite index (1 = top-left cell)
---@param x     number   destination left edge in game-space pixels
---@param y     number   destination top edge in game-space pixels
function gfx.spr(index, x, y) end

---Extended `spr`: draws a 16×16 sprite with required flip flags. Same
---indexing as `gfx.spr`.
---@param index  integer  one-based sprite index (1 = top-left cell)
---@param x      number   destination left edge in game-space pixels
---@param y      number   destination top edge in game-space pixels
---@param flip_x boolean  flip horizontally (mirror left/right) when true
---@param flip_y boolean  flip vertically (mirror top/bottom) when true
function gfx.spr_ex(index, x, y, flip_x, flip_y) end

---Draws an arbitrary (sx, sy, sw, sh) rectangle from `sprites.png` at
---(dx, dy) at its original size. `s*` args index into the source sheet
---in pixels; `d*` args are the destination on screen.
---@param sx number  source rect left edge on `sprites.png` (pixels)
---@param sy number  source rect top edge on `sprites.png` (pixels)
---@param sw number  source rect width in pixels
---@param sh number  source rect height in pixels
---@param dx number  destination left edge in game-space pixels
---@param dy number  destination top edge in game-space pixels
function gfx.sspr(sx, sy, sw, sh, dx, dy) end

---Extended `sspr`: source rect stretched to (dw, dh) at the destination
---with required flip flags. All ten args required; write a thin
---wrapper if a particular flag combination shows up often in your
---code.
---@param sx     number   source rect left edge on `sprites.png` (pixels)
---@param sy     number   source rect top edge on `sprites.png` (pixels)
---@param sw     number   source rect width in pixels
---@param sh     number   source rect height in pixels
---@param dx     number   destination left edge in game-space pixels
---@param dy     number   destination top edge in game-space pixels
---@param dw     number   destination width in pixels (stretches the source)
---@param dh     number   destination height in pixels (stretches the source)
---@param flip_x boolean  flip horizontally (mirror left/right) when true
---@param flip_y boolean  flip vertically (mirror top/bottom) when true
function gfx.sspr_ex(sx, sy, sw, sh, dx, dy, dw, dh, flip_x, flip_y) end

---Activates a post-process fragment shader. Loads `shaders/<name>.fs`
---(and optional `<name>.vs`) and runs it as the final pass when the
---game render target is blitted to the window. Pass nil to clear.
---On web the loader prefers `<name>_es.fs` (GLSL ES 100); on desktop
---it prefers `<name>.fs` (GLSL 330). Shader source live-reloads on
---save in `usagi dev`.
---@param name string|nil  shader name (file stem under `shaders/`), or nil to clear
function gfx.shader_set(name) end

---Sets a uniform on the active shader. The value type drives the
---uniform type: a number maps to float, a 2/3/4-length numeric table
---maps to vec2 / vec3 / vec4. Queues the write; the engine flushes
---queued uniforms once per frame before the post-process pass.
---@param name  string                                    uniform name as declared in the shader source
---@param value number|number[]                           float, or {x, y} / {x, y, z} / {x, y, z, w}
function gfx.shader_uniform(name, value) end

---@class Usagi.Sfx
sfx = {}

---Plays a sound effect by name. Names are file stems from the `sfx/`
---directory next to the game's main .lua (e.g. `sfx/jump.wav` → "jump").
---Unknown names silently no-op. Calling while already playing restarts.
---@param name string  file stem of a `.wav` under `sfx/`
function sfx.play(name) end

---@class Usagi.Music
music = {}

---Plays a music track once and stops at the end. Names are file stems
---from the `music/` directory next to the game's main .lua (e.g.
---`music/intro.ogg` → "intro"). Recognized extensions: ogg, mp3, wav,
---flac. Stops the currently-playing track first if there is one.
---Unknown names silently no-op. Callable from `_init` so a title
---track can start the moment the window opens.
---@param name string  file stem under `music/`
function music.play(name) end

---Plays a music track and loops it forever. Stops the currently-
---playing track first. Callable from `_init`.
---@param name string  file stem under `music/`
function music.loop(name) end

---Stops whatever music is currently playing. No-op when nothing is.
function music.stop() end

---Abstract input actions. Each is a union over keyboard keys, gamepad
---buttons, and analog-stick directions:
---
---- LEFT:  arrow left, A, dpad left, left stick left
---- RIGHT: arrow right, D, dpad right, left stick right
---- UP:    arrow up, W, dpad up, left stick up
---- DOWN:  arrow down, S, dpad down, left stick down
---- BTN1:  Z, J; gamepad south face (Xbox A, PS Cross)
---- BTN2:  X, K; gamepad east face  (Xbox B, PS Circle)
---- BTN3:  C, L; gamepad north + west face (Xbox Y/X, PS Triangle/Square)
---
---Mouse buttons (separate from the action constants above):
---
---- MOUSE_LEFT:   left mouse button
---- MOUSE_RIGHT:  right mouse button
---- MOUSE_MIDDLE: middle mouse button (wheel click)
---
---Source identifiers for `input.last_source()` and the source-aware
---`input.mapping_for`:
---
---- SOURCE_KEYBOARD: "keyboard"
---- SOURCE_GAMEPAD:  "gamepad"
---@class Usagi.Input
---@field LEFT             integer
---@field RIGHT            integer
---@field UP               integer
---@field DOWN             integer
---@field BTN1             integer
---@field BTN2             integer
---@field BTN3             integer
---@field MOUSE_LEFT       integer
---@field MOUSE_RIGHT      integer
---@field MOUSE_MIDDLE     integer
---@field SOURCE_KEYBOARD  string
---@field SOURCE_GAMEPAD   string
---@field KEY_A            integer
---@field KEY_B            integer
---@field KEY_C            integer
---@field KEY_D            integer
---@field KEY_E            integer
---@field KEY_F            integer
---@field KEY_G            integer
---@field KEY_H            integer
---@field KEY_I            integer
---@field KEY_J            integer
---@field KEY_K            integer
---@field KEY_L            integer
---@field KEY_M            integer
---@field KEY_N            integer
---@field KEY_O            integer
---@field KEY_P            integer
---@field KEY_Q            integer
---@field KEY_R            integer
---@field KEY_S            integer
---@field KEY_T            integer
---@field KEY_U            integer
---@field KEY_V            integer
---@field KEY_W            integer
---@field KEY_X            integer
---@field KEY_Y            integer
---@field KEY_Z            integer
---@field KEY_0            integer
---@field KEY_1            integer
---@field KEY_2            integer
---@field KEY_3            integer
---@field KEY_4            integer
---@field KEY_5            integer
---@field KEY_6            integer
---@field KEY_7            integer
---@field KEY_8            integer
---@field KEY_9            integer
---@field KEY_F1           integer
---@field KEY_F2           integer
---@field KEY_F3           integer
---@field KEY_F4           integer
---@field KEY_F5           integer
---@field KEY_F6           integer
---@field KEY_F7           integer
---@field KEY_F8           integer
---@field KEY_F9           integer
---@field KEY_F10          integer
---@field KEY_F11          integer
---@field KEY_F12          integer
---@field KEY_SPACE        integer
---@field KEY_ENTER        integer
---@field KEY_ESCAPE       integer
---@field KEY_TAB          integer
---@field KEY_BACKSPACE    integer
---@field KEY_DELETE       integer
---@field KEY_LEFT         integer
---@field KEY_RIGHT        integer
---@field KEY_UP           integer
---@field KEY_DOWN         integer
---@field KEY_LSHIFT       integer
---@field KEY_RSHIFT       integer
---@field KEY_LCTRL        integer
---@field KEY_RCTRL        integer
---@field KEY_LALT         integer
---@field KEY_RALT         integer
---@field KEY_BACKTICK     integer
---@field KEY_MINUS        integer
---@field KEY_EQUAL        integer
---@field KEY_LBRACKET     integer
---@field KEY_RBRACKET     integer
---@field KEY_BACKSLASH    integer
---@field KEY_SEMICOLON    integer
---@field KEY_APOSTROPHE   integer
---@field KEY_COMMA        integer
---@field KEY_PERIOD       integer
---@field KEY_SLASH        integer
input = {}

---Returns true the frame any source bound to `action` first went down.
---@param action integer  one of input.LEFT / RIGHT / UP / DOWN / BTN1 / BTN2 / BTN3
---@return boolean
function input.pressed(action) end

---Returns true while any source bound to `action` is held.
---@param action integer  one of input.LEFT / RIGHT / UP / DOWN / BTN1 / BTN2 / BTN3
---@return boolean
function input.held(action) end

---Returns true the frame any source bound to `action` first went up
---(transitioned from held to released). Mirrors `input.pressed` for the
---release edge.
---@param action integer  one of input.LEFT / RIGHT / UP / DOWN / BTN1 / BTN2 / BTN3
---@return boolean
function input.released(action) end

---Label of the active input source's primary binding for `action` (e.g.
---"Z" on keyboard, "Pad-A" on gamepad). Honors any keymap remap the
---player set via the pause menu's Configure Keys flow. Useful for
---rendering contextual control prompts. Returns `nil` for unknown
---actions or when the active source has no binding for `action`.
---@param action integer  one of input.LEFT / RIGHT / UP / DOWN / BTN1 / BTN2 / BTN3
---@return string?
function input.mapping_for(action) end

---The input source that most recently fired any bound action. Returns
---`input.SOURCE_KEYBOARD` ("keyboard") or `input.SOURCE_GAMEPAD`
---("gamepad"). Switches only when a *bound* input fires, so menu keys
---and idle activity don't flip it.
---@return string  matches one of input.SOURCE_KEYBOARD / input.SOURCE_GAMEPAD
function input.last_source() end

---Cursor position in game-space pixels (so it lines up with `gfx.*`
---coords regardless of window size or pixel-perfect scaling). Returns
---two values: `x, y`. When the cursor sits over the letterbox bars,
---the values fall outside `0..usagi.GAME_W` / `0..usagi.GAME_H` —
---bounds-check before treating them as in-game coords.
---@return integer x  game-space x in pixels
---@return integer y  game-space y in pixels
function input.mouse() end

---Returns true while the given mouse button is held.
---@param button integer  one of input.MOUSE_LEFT / input.MOUSE_RIGHT / input.MOUSE_MIDDLE
---@return boolean
function input.mouse_held(button) end

---Returns true the frame the given mouse button first went down.
---@param button integer  one of input.MOUSE_LEFT / input.MOUSE_RIGHT / input.MOUSE_MIDDLE
---@return boolean
function input.mouse_pressed(button) end

---Returns true the frame the given mouse button first went up
---(transitioned from held to released).
---@param button integer  one of input.MOUSE_LEFT / input.MOUSE_RIGHT / input.MOUSE_MIDDLE
---@return boolean
function input.mouse_released(button) end

---Returns true while the given keyboard key is held.
---
---Direct keyboard reads bypass the keymap override and gamepad
---bindings — prefer `input.held(action)` for game actions players
---should be able to remap or play with a controller. Use this for dev
---hotkeys (toggling debug overlays, F-key shortcuts) and for
---keyboard-and-mouse-only games.
---@param key integer  one of the input.KEY_* constants
---@return boolean
function input.key_held(key) end

---Returns true the frame the given keyboard key first went down. See
---`input.key_held` for the bypass-the-keymap caveat.
---@param key integer  one of the input.KEY_* constants
---@return boolean
function input.key_pressed(key) end

---Returns true the frame the given keyboard key first went up
---(transitioned from held to released). See `input.key_held` for the
---bypass-the-keymap caveat.
---@param key integer  one of the input.KEY_* constants
---@return boolean
function input.key_released(key) end

---Show or hide the OS cursor over the game window. Persists until
---changed. Callable from `_init` so games can hide the cursor before
---the first frame draws (e.g. when rendering a custom in-game cursor).
---@param visible boolean  true to show, false to hide
function input.set_mouse_visible(visible) end

---Returns true when the OS cursor is currently shown over the window.
---Reflects the latest `input.set_mouse_visible` call synchronously, so
---it's safe to use as part of a toggle:
---`input.set_mouse_visible(not input.mouse_visible())`.
---@return boolean
function input.mouse_visible() end

---Engine-level info. The per-domain APIs (`gfx`, `input`) are top-level
---globals, not fields on this table.
---@class Usagi
---@field GAME_W      number   game render width in pixels
---@field GAME_H      number   game render height in pixels
---@field SPRITE_SIZE integer  side length, in pixels, of one cell in `sprites.png` (drives `gfx.spr` indexing)
---@field IS_DEV      boolean  true under `usagi dev`; false for `usagi run` and compiled binaries
---@field elapsed     number   wall-clock seconds since session start; updated once per frame before _update
usagi = {}

---Measures `text` in the bundled font and returns its rendered size
---in pixels. Returns two values: `width, height`. Available from any
---callback (`_init`, `_update`, `_draw`) — useful for pre-computing
---layout once in `_init` and reusing the result every frame.
---@param text string  string to measure
---@return integer width   pixel width
---@return integer height  pixel height (equals the font's line height)
function usagi.measure_text(text) end

---Pretty-prints any Lua value to a string. Tables are recursed with
---sorted keys; arrays render in order; cycles render as `<cycle>`;
---non-serializable values (functions, userdata, threads) render as
---placeholders. Pair with `print(usagi.dump(state))` for terminal
---debugging or feed the result into `gfx.text` to draw it on screen.
---@param v any  the value to inspect
---@return string pretty  human-readable Lua-ish source for `v`
function usagi.dump(v) end

---Persist a Lua table as JSON. Saves are per-game, namespaced by
---`game_id` from `_config()`. One file per game; nest your own
---structure inside (settings, run state, unlocks).
---@param t table   table to serialize. functions, userdata, NaN, and cycles error
function usagi.save(t) end

---Read the persisted save table back. Returns `nil` on first run
---(no save file). Idiomatic call: `state = usagi.load() or { ... defaults ... }`.
---@return table?
function usagi.load() end

---Config table returned by `_config()`. All fields optional except
---`game_id`, which is only required if you call `usagi.save` /
---`usagi.load`. Missing fields fall back to engine defaults.
---@class Usagi.Config
---@field name? string  display name. Window title, macOS .app bundle directory, and (slugged) archive/binary names on `usagi export` (default: project directory name)
---@field pixel_perfect? boolean false (default) = any scale that fits the window while preserving aspect ratio; true = integer scale only with letterbox bars
---@field game_id? string  reverse-DNS identifier (e.g. "com.you.mygame"), required for save/load
---@field icon? integer  1-based tile index into sprites.png to use as the window icon (same indexing as gfx.spr); omit for the default Usagi bunny
---@field game_width? number  game render width in pixels (default 320). Tested range 160..640
---@field game_height? number  game render height in pixels (default 180). Tested range 90..360
---@field sprite_size? integer  side length, in pixels, of one cell in sprites.png (default 16). Drives gfx.spr indexing, the tilepicker tool's grid, and the window-icon slicer. sprites.png must be a multiple of this value on both axes.

---Optional. Returns engine config read once before the window opens.
---Omit if the defaults are fine.
---@return Usagi.Config?
function _config() end

---Called once when the game starts. Use for loading assets and initializing state.
function _init() end

---Called every frame to update game state. Runs before _draw.
---@param dt number  delta-time: seconds since last frame
function _update(dt) end

---Called every frame to render. Runs after _update.
---@param dt number  delta-time: seconds since last frame
function _draw(dt) end

---@class Usagi.Vec2
---@field x number
---@field y number

---@class Usagi.Rect
---@field x number
---@field y number
---@field w number
---@field h number

---@class Usagi.Circ
---@field x number
---@field y number
---@field r number

---Drop-in math/geometry helpers. Pure Lua, no engine state. Source
---lives in `runtime/util.lua` — read it for full implementations or
---fork it if you want different semantics.
---@class Usagi.Util
util = {}

---Clamps `v` into `[lo, hi]`.
---@param v number
---@param lo number
---@param hi number
---@return number
function util.clamp(v, lo, hi) end

---Returns -1, 0, or 1 according to the sign of `v`.
---@param v number
---@return integer
function util.sign(v) end

---Half-up rounding to the nearest integer. Pixel snapping is the
---driving use case in 2D pixel-art games.
---@param v number
---@return integer
function util.round(v) end

---Moves `current` toward `target` by at most `max_delta`, never
---overshooting. Per-frame smoothing primitive — pass a delta
---scaled by `dt` for frame-rate independence.
---@param current number
---@param target number
---@param max_delta number
---@return number
function util.approach(current, target, max_delta) end

---Linear interpolation. `t = 0` returns `a`, `t = 1` returns `b`.
---Values of `t` outside `[0, 1]` extrapolate (no clamping).
---@param a number
---@param b number
---@param t number
---@return number
function util.lerp(a, b, t) end

---Wraps `v` into `[lo, hi)`. Useful for cyclic values like angles or
---looped indexing. Works for negative `v`: `util.wrap(-1, 0, 4) == 3`.
---@param v number
---@param lo number
---@param hi number
---@return number
function util.wrap(v, lo, hi) end

---Boolean from time. Toggles `hz` times per second — the on/off
---interval is `1/hz` seconds. For invincibility flicker, UI blinks,
---low-health warnings.
---@param t number  seconds
---@param hz number  toggles per second
---@return boolean
function util.flash(t, hz) end

---Normalizes a `{x, y}` vector to unit length. Returns a new table;
---the input is unchanged. A zero vector returns `{x = 0, y = 0}`.
---@param v Usagi.Vec2
---@return Usagi.Vec2
function util.vec_normalize(v) end

---Distance between two `{x, y}` points.
---@param a Usagi.Vec2
---@param b Usagi.Vec2
---@return number
function util.vec_dist(a, b) end

---Squared distance between two `{x, y}` points. Cheaper than
---`vec_dist` (skips the sqrt); use for "is X closer than Y?" by
---comparing against `r * r`.
---@param a Usagi.Vec2
---@param b Usagi.Vec2
---@return number
function util.vec_dist_sq(a, b) end

---Builds a vector at `angle` (radians) with magnitude `len`. `len`
---defaults to 1 for a unit vector. Pair with `math.atan(dy, dx)` to
---convert any direction into a velocity.
---@param angle number  radians
---@param len? number   magnitude (default 1)
---@return Usagi.Vec2
function util.vec_from_angle(angle, len) end

---True when the `{x, y}` point is inside the rect `{x, y, w, h}`.
---Half-open: left/top edges are inside, right/bottom edges are
---outside. Matches typical sprite-rect hit testing.
---@param p Usagi.Vec2
---@param r Usagi.Rect
---@return boolean
function util.point_in_rect(p, r) end

---True when the `{x, y}` point is strictly inside the circle
---`{x, y, r}`. Points on the boundary are considered outside.
---@param p Usagi.Vec2
---@param c Usagi.Circ
---@return boolean
function util.point_in_circ(p, c) end

---True when the two AABBs share interior area. Edge-adjacent rects
---are considered non-overlapping.
---@param a Usagi.Rect
---@param b Usagi.Rect
---@return boolean
function util.rect_overlap(a, b) end

---True when the two circles overlap. Tangent circles are
---considered non-overlapping.
---@param a Usagi.Circ
---@param b Usagi.Circ
---@return boolean
function util.circ_overlap(a, b) end

---True when a circle and a rect overlap. Uses the closest-point
---method: clamp the circle center to the rect, test distance.
---@param c Usagi.Circ
---@param r Usagi.Rect
---@return boolean
function util.circ_rect_overlap(c, r) end

---Engine-level juice primitives: hitstop, screen shake, flash, and
---slow-motion. Each call sets per-session state that decays once per
---frame. Stacking rule across all four: longer duration wins; for
---the magnitude param, the latest call wins. Spam-calling is safe.
effect = {}

---Freezes the game's `_update` loop for `time` seconds. `_draw` keeps
---running so the world stays on-screen. The classic juice trick for
---weighty hits: pair with `effect.screen_shake` and `effect.flash` on
---impact. If a longer hitstop is already in flight, this call is a
---no-op (longer wins).
---@param time number  seconds to freeze update
function effect.hitstop(time) end

---Shakes the rendered view for `time` seconds with up to `intensity`
---game-pixel offset. Magnitude decays linearly to zero across the
---duration. The shake is applied to the RT-to-screen blit, so
---overlays drawn outside the world (error, REC indicator) stay
---stable.
---@param time      number  seconds to shake
---@param intensity number  maximum offset in game pixels (try 2-6)
function effect.screen_shake(time, intensity) end

---Flashes a full-screen overlay of palette color `color` over the
---rendered view for `time` seconds. Alpha decays linearly from
---opaque to transparent. White on hits, red on damage, etc.
---@param time  number   seconds the flash is visible
---@param color integer  a gfx.COLOR_* constant
function effect.flash(time, color) end

---Scales the `dt` passed to `_update` for `time` seconds. `scale=0.5`
---is half-speed; `scale=0` freezes update (use `effect.hitstop` for
---that explicitly); `scale>1` plays faster. Wall-clock decay is
---unaffected; the slow_mo timer itself counts down at real time.
---@param time  number  seconds the scale is applied
---@param scale number  dt multiplier; 0..1 for slow, >1 for fast
function effect.slow_mo(time, scale) end

---Cancels every active effect immediately (hitstop, screen_shake,
---flash, slow_mo). Useful on game-over, scene transitions, or
---anywhere lingering juice would clash with the new state. Reset and
---F5 / Ctrl+R already call this internally; this is the manual
---escape hatch.
function effect.stop() end
