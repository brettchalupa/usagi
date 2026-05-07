# Usagi Shader System

Post-process GLSL fragment shaders run as the final pass when the game's render
target is blitted to the window. Use them for CRT effects, palette swaps,
vignettes, color grading, and other full-screen presentation effects.

Status: experimental. The API surface and shader file format may change before
Usagi 1.0. Native captures bake the active shader into saved screenshots and GIF
frames.

## Lua API

- `gfx.shader_set("name")`: activate `shaders/<name>.usagi.fs`, or a native
  `shaders/<name>.fs` / `shaders/<name>_es.fs` fallback.
- `gfx.shader_set(nil)`: clear the active shader.
- `gfx.shader_uniform("u_name", v)`: queue a uniform write. `v` may be a number
  (float) or a 2/3/4-length numeric table (vec2/vec3/vec4). Call this every
  frame inside `_update` or `_draw` for animated values.

```lua
function _init() gfx.shader_set("crt") end

function _draw(_dt)
  gfx.shader_uniform("u_time", usagi.elapsed)
  gfx.shader_uniform("u_resolution", { usagi.GAME_W, usagi.GAME_H })
  -- ... your normal gfx.* calls ...
end
```

For generic `.usagi.fs` shaders, Usagi reflects declared `float`, `vec2`,
`vec3`, and `vec4` uniforms and reports a clear error if a queued Lua value has
the wrong shape. Uniform names missing from the active shader remain a no-op, so
shared game code can set optional shader uniforms safely. Native GLSL fallback
files are loaded directly through raylib and do not have generic reflection
metadata.

## Cross-Platform Shader Files

The recommended path is one generic Usagi shader at
`shaders/<name>.usagi.fs`. Usagi parses that source, validates the engine-owned
bindings, lowers Usagi intrinsics, and emits target GLSL. Desktop currently uses
GLSL `#version 330`; web uses GLSL ES `#version 100` (WebGL 1 / GLES 2). The
compiler has a GLSL 440 emitter profile staged for future desktop backend
selection.

Generic shaders must not include `#version` or declare `texture0`,
`fragTexCoord`, `fragColor`, `finalColor`, `gl_FragColor`, or `main`. Usagi
provides those. Define exactly one entrypoint:

```glsl
#usagi shader 1

vec4 usagi_main(vec2 uv, vec4 color) {
    vec3 src = usagi_texture(texture0, uv).rgb;
    return vec4(src, 1.0) * color;
}
```

Use `usagi_texture(texture0, uv)` for texture reads. The compiler rejects direct
`texture(...)` / `texture2D(...)` calls in generic shader sources so one file
stays target-neutral.

## Shader Language Contract

A `.usagi.fs` file is a fragment-only, GLSL-like source file. The stable grammar
is intentionally small:

```text
module       = marker? item*
marker       = "#usagi shader 1" on the first non-blank line
item         = uniform_decl | function_decl | raw_top_level_decl
uniform_decl = "uniform" type name ("," name)* ";"
function_decl = type name "(" params? ")" block
params       = type name ("," type name)*
entrypoint   = "vec4 usagi_main(vec2 uv, vec4 color)"
```

Function bodies use GLSL expressions and statements that are valid for all
active generic targets. The compiler preserves comments and whitespace, rejects
`#version`, and treats other preprocessor lines as target-neutral advanced use:
they pass through today, but they are not interpreted by Usagi and must compile
on every target you intend to ship.

The engine owns these names:

- `texture0`: sampler for the game render target.
- `fragTexCoord`: generated input UV, exposed as the `uv` parameter.
- `fragColor`: generated input color, exposed as the `color` parameter.
- `finalColor`: generated desktop fragment output.
- `gl_FragColor`: generated GLSL ES fragment output.
- `main`: generated wrapper that calls `usagi_main(...)`.

The `usagi_` prefix is reserved for shader intrinsics. The first supported
intrinsic is `vec4 usagi_texture(sampler2D sampler, vec2 uv)`. It lowers to
`texture(sampler, uv)` on GLSL 330 / staged GLSL 440, and to
`texture2D(sampler, uv)` on GLSL ES 100.

## Generic Target Guarantees

- GLSL ES 100: emits `#version 100`, `precision mediump float;`, `varying`
  inputs, `uniform sampler2D texture0`, `texture2D(...)`, and `gl_FragColor`.
- GLSL 330: emits `#version 330`, `in` inputs, `uniform sampler2D texture0`,
  `texture(...)`, and `out vec4 finalColor`.
- GLSL 440: emitter is staged for future runtime selection and emits
  `#version 440 core` plus `layout(location = 0) out vec4 finalColor`.

Compatibility gate: the bundled generic shaders in `examples/shader`,
`examples/notetris`, and `examples/playdate` are the backwards-compatibility
baseline. Changes to the generic compiler must keep them compiling for every
supported generic profile unless the shader language version is intentionally
changed.

## Native GLSL Fallbacks

Native GLSL files remain supported as an escape hatch:

- `shaders/<name>.fs`: desktop, `#version 330`, `in`/`out`, `texture(...)`,
  custom `out vec4 finalColor`.
- `shaders/<name>_es.fs`: web, `#version 100`, `precision mediump float;`,
  `varying`, `texture2D(...)`, `gl_FragColor` output.

Usagi first looks for `<name>.usagi.fs`. If it is missing, web prefers `_es.fs`
and falls back to `.fs`; desktop is the reverse. If only one native file is
shipped, every platform that loads it runs that one. Native fallback files are
loaded directly through raylib, so they own their target-specific GLSL syntax.

## Live Reload

Saving the active shader's `.usagi.fs`, `.fs`, or `.vs` file rebuilds it
in-place. Cached uniforms are replayed onto the new shader. Compile errors
print to the terminal and keep the previous shader live.

## Bundling

`usagi export` walks `shaders/` and ships every `.usagi.fs`, `.fs`, and `.vs` in
the bundle, so shaders work the same in `usagi dev`, `usagi run`, `.usagi`
files, and fused exes on every platform.

## Captures

F8 / Cmd+F screenshots and F9 / Cmd+G GIF recording include the active shader.
The on-screen pass still runs at window resolution, while native captures render
the same post-process into a game-sized capture target before the usual 2x
export. PNG captures preserve full shader RGB. GIF captures use the fixed Pico
palette for unshaded frames and an adaptive per-frame palette when a shader is
baked in. Window-only overlays such as the Lua error banner and REC indicator
stay out of saved files.

## Examples

See `examples/shader/`, `examples/notetris/`, and `examples/playdate/` for
generic shader examples.

## Resources

- [Raylib shaders demo](https://www.raylib.com/examples/shaders/loader.html?name=shaders_postprocessing)
- [Raylib shaders source](https://github.com/raysan5/raylib/blob/master/examples/shaders/shaders_postprocessing.c)
