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

The portability contract is strict: a valid generic Usagi shader should compile
through the selected platform target without the author maintaining separate
desktop and web shader files. Native GLSL files are an advanced escape hatch for
engine or target-specific experiments, not the normal way to ship a
cross-platform shader.

Generic shaders must not include `#version` or declare `texture0`,
`fragTexCoord`, `fragColor`, `finalColor`, `gl_FragColor`, or `main`. Usagi
provides those. They also should not declare target-specific stage-interface
or precision qualifiers such as `in`, `out`, `varying`, `layout`, or
`precision`; the compiler validates those against the selected target before
the GL driver sees the generated source. Define exactly one entrypoint:

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

A `.usagi.fs` file is a fragment-only, GLSL-like source file. The official
contract is intentionally smaller than full GLSL: every construct listed here
must either lower for the selected target profile or fail with a deterministic
Usagi compiler diagnostic before the GL driver runs. Constructs not listed here
are outside the generic portability contract and belong in native fallback files
until they are added to this contract and compiler validation.

```text
module          = blank* marker? item*
marker          = "#usagi shader 1" on the first non-blank line
item            = uniform_decl | function_decl | raw_top_level_decl
uniform_decl    = "uniform" ident name ("," name)* ";"
function_decl   = type name "(" params? ")" block
type            = "void" | "bool" | "int" | "float"
                | "vec2" | "vec3" | "vec4"
                | "bvec2" | "bvec3" | "bvec4"
                | "ivec2" | "ivec3" | "ivec4"
                | "mat2" | "mat3" | "mat4"
params          = ident name ("," ident name)*
block           = "{" statement* "}"
statement       = return_stmt | if_stmt | block | raw_stmt
return_stmt     = "return" expression? ";"
if_stmt         = "if" "(" expression ")" branch ("else" branch)?
branch          = block | statement
raw_stmt        = expression ";"
raw_top_level_decl = balanced tokens ending with a top-level ";"
expression      = balanced GLSL token sequence with parsed function calls
entrypoint      = "vec4 usagi_main(vec2 uv, vec4 color)"
```

The entrypoint must appear exactly once. User helper functions may use any
listed return type, but the public entrypoint signature is fixed because Usagi
generates the target-specific `main()` wrapper.

Function bodies support `return`, `if` / `else`, nested blocks, and raw
semicolon-terminated GLSL statements such as local declarations, assignments,
constructor calls, arithmetic expressions, `discard;`, and helper-function
calls. Expressions may use scalar, vector, matrix, swizzle, constructor, and
operator syntax that exists in the common GLSL ES 100 / GLSL 330 / GLSL 440
fragment-shader subset. Calls are parsed so Usagi intrinsics and target-specific
texture calls can be validated before emission. Structured statements not
listed here, such as `for`, `while`, `do`, and `switch`, are not part of the
generic contract yet.

The compiler preserves comments, whitespace, and raw GLSL tokens when emitting
target GLSL. `#version` is always rejected because Usagi owns the emitted target
version. The optional `#usagi shader 1` marker may appear only on the first
non-blank line and is stripped before GLSL emission. Other preprocessor lines
are preserved but not interpreted by Usagi; they must be target-neutral and must
compile correctly on every selected profile.

Top-level raw declarations are limited to balanced token sequences that end in a
top-level semicolon, for example constants or structs that are valid on every
target profile. Generic shaders must not declare target-specific stage IO,
outputs, precision directives, or target-specific texture functions. Use Usagi
entrypoint parameters and intrinsics instead.

The engine owns these names:

- `texture0`: sampler for the game render target.
- `fragTexCoord`: generated input UV, exposed as the `uv` parameter.
- `fragColor`: generated input color, exposed as the `color` parameter.
- `finalColor`: generated desktop fragment output.
- `gl_FragColor`: generated GLSL ES fragment output.
- `main`: generated wrapper that calls `usagi_main(...)`.

The `usagi_` prefix is reserved for shader intrinsics. The supported intrinsic
set is currently:

```text
vec4 usagi_texture(sampler2D sampler, vec2 uv)
```

`usagi_texture(...)` lowers to `texture(sampler, uv)` on GLSL 330 / staged GLSL
440, and to `texture2D(sampler, uv)` on GLSL ES 100. Direct `texture(...)` and
`texture2D(...)` calls are rejected in generic sources because they are
target-specific.

## Generic Target Guarantees

Generic `.usagi.fs` portability means the same source compiles for each selected
target profile. The compiler generates the stage interface, texture sampler,
output variable, and wrapper function for the target:

| Profile | Runtime selection | Version | Inputs | Output | Texture intrinsic | Precision |
| --- | --- | --- | --- | --- | --- | --- |
| GLSL ES 100 | Web / WebGL 1 | `#version 100` | `varying vec2 fragTexCoord`, `varying vec4 fragColor` | `gl_FragColor` | `texture2D(...)` | emits `precision mediump float;` |
| GLSL 330 | Current desktop default | `#version 330` | `in vec2 fragTexCoord`, `in vec4 fragColor` | `out vec4 finalColor` | `texture(...)` | no precision directive |
| GLSL 440 | Staged desktop profile | `#version 440 core` | `in vec2 fragTexCoord`, `in vec4 fragColor` | `layout(location = 0) out vec4 finalColor` | `texture(...)` | no precision directive |

The portable feature baseline is the intersection of GLSL ES 100, GLSL 330, and
GLSL 440 fragment-shader behavior:

- `float`, `int`, `bool`, `vec*`, `ivec*`, `bvec*`, and `mat2` / `mat3` /
  `mat4` helper-function return types.
- Uniform declarations, including reflected `float`, `vec2`, `vec3`, and
  `vec4` uniforms for Lua-side value-shape validation.
- Common scalar/vector/matrix constructors, operators, swizzles, math
  functions, comparisons, and branches that are valid in all selected profiles.
- `usagi_texture(texture0, uv)` for sampling the game render target.
- Common fragment built-ins available in every selected target profile.

The following source constructs are target-specific and are rejected by at least
one profile before GL compile:

- `#version`; Usagi always emits the selected target version.
- `texture(...)` and `texture2D(...)`; use `usagi_texture(...)`.
- `in`, `out`, `varying`, `layout`, and `precision` declarations in generic
  shader source; Usagi emits the correct form for the target.
- Declarations of `texture0`, `fragTexCoord`, `fragColor`, `finalColor`,
  `gl_FragColor`, or `main`; Usagi owns those bindings.

GLSL 440 is available through the offline compiler profile today, but the
runtime still selects GLSL 330 for desktop until the active backend/context can
prove GLSL 440 support. Native fallback files may still target a specific GLSL
version directly, but they are outside the generic compiler and reflection
contract.

Compatibility gate: the bundled generic shaders in `examples/shader`,
`examples/notetris`, and `examples/playdate` are the backwards-compatibility
baseline. Changes to the generic compiler must keep them compiling for every
supported generic profile unless the shader language version is intentionally
changed.

## Compiler Module Layout

Shader-specific runtime, CLI, and compiler code should stay under `src/shader/`
so generic shader behavior has one ownership boundary:

- `mod.rs`: owns runtime shader loading, native fallback selection, live reload,
  uniform replay, and integration with the render path.
- `check_cli.rs`: owns `usagi shaders check`, project shader discovery, and
  offline conformance reporting.
- `compiler.rs`: owns the compile entrypoint, result metadata, and the
  high-level parse, validate, and emit pipeline.
- `compiler/syntax.rs`: owns source spans, tokens, lexing, parsing, and the
  parsed source tree for declarations, functions, statements, expressions, and
  calls.
- `compiler/emit_glsl.rs`: owns GLSL target capability records and emission for
  GLSL ES 100, GLSL 330, and GLSL 440.
- `compiler/check.rs`: owns compiler validation. It starts with target capability checks
  and should grow into semantic validation and type checking.
- `compiler/ir.rs`: owns the checked backend-neutral compiler boundary used by GLSL
  emitters and later HLSL, MSL, or SPIR-V emitters.

Avoid naming the first parser split `ast.rs` while it still preserves source
tokens and rewrite spans. `syntax.rs` is more accurate until a fully checked
AST/ABT boundary exists.

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
in-place. Cached uniforms are replayed onto the new shader. Errors print to the
terminal with a category: `[compiler]` for generic `.usagi.fs` validation and
generation failures, `[source]` for missing or unreadable shader files, and
`[gl-driver]` for native OpenGL/WebGL compile or link failures. Reload failures
keep the previous shader live.

## Offline Checks

Run `usagi shaders check path/to/project` to compile every direct
`shaders/*.usagi.fs` file without opening a window. By default it checks the
desktop runtime target (currently GLSL 330), reports every compiler diagnostic
it finds, and exits non-zero if the selected target fails. Use `--target web`
for the web runtime target (GLSL ES 100), `--target desktop` for desktop, or
`--target all` for a conformance sweep across ES 100, GLSL 330, and the staged
GLSL 440 profile.

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
