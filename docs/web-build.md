# Web build (wasm32-unknown-emscripten) notes

This file captures the non-obvious bits of getting Usagi to compile and link for
the web. If the web build breaks, start here.

## Toolchain

- Stable Rust (currently 1.95.0 verified). No `rust-toolchain.toml` pin.
- Emscripten via emsdk. `setup-emscripten.sh` installs to `$XDG_DATA_HOME/emsdk`
  (or `~/.local/share/emsdk`); source `~/.local/share/emsdk/emsdk_env.sh` to put
  `emcc` on `PATH`. On macOS, you can do `brew install emscripten`.
- emcc 5.0.6 verified.

Build with `just build-web` (or `just build-web-release`).

## The wasm exception ABI: what you need to know

Rust 1.93+ unconditionally passes `-fwasm-exceptions` to emcc when targeting
`wasm32-unknown-emscripten`. This was [rust-lang/rust#147224][r147224]. The
older `panic = "abort"`-disables-it advice from blog posts and Stack Overflow no
longer applies: rustc emits `-fwasm-exceptions` regardless of panic strategy
because the prebuilt stable sysroot is itself built with wasm-eh (see
[rust-lang/rust#135450][r135450]).

That means _every_ C/C++ object file in the link must also use the wasm-eh ABI.
If anything is built with the legacy JS-EH ABI, the link fails with undefined
symbols like `__cxa_find_matching_catch_3`.

[r147224]: https://github.com/rust-lang/rust/pull/147224
[r135450]: https://github.com/rust-lang/rust/pull/135450

### Three places we need to keep in sync

1. **rustc link args** in `.cargo/config.toml` rustflags:
   `-C link-arg=-sSUPPORT_LONGJMP=wasm`. emcc rejects
   `SUPPORT_LONGJMP=emscripten` (the legacy setjmp ABI) when wasm-eh is on, so
   we use the wasm-native pairing. mlua's vendored Lua uses setjmp for error
   handling, which is why this matters.
2. **cc-rs CFLAGS** in `.cargo/config.toml` `[env]`:
   `CFLAGS_wasm32_unknown_emscripten` and `CXXFLAGS_wasm32_unknown_emscripten`
   set to `-fwasm-exceptions -sSUPPORT_LONGJMP=wasm`. cc-rs reads these when
   compiling C deps for the target, including mlua's vendored Lua.
3. **emcc CFLAGS** via `EMCC_CFLAGS` in the justfile recipes:
   `-fwasm-exceptions -sSUPPORT_LONGJMP=wasm` (alongside the raylib port flags).
   raylib's CMake build is invoked through emcc directly, bypassing cc-rs, so it
   needs the same flags via `EMCC_CFLAGS`.

If you change one of these, change them together. A mismatch shows up as either
a `__cxa_find_matching_catch_*` undefined symbol error (some object file used
JS-EH ABI) or as a
`SUPPORT_LONGJMP=emscripten is not compatible
with -fwasm-exceptions` rejection
(legacy longjmp ABI).

### What `panic = "abort"` is and isn't doing

The `[profile.dev]` and `[profile.release]` `panic = "abort"` settings in
`Cargo.toml` are there for binary size, on all targets. They are NOT what makes
the web build link. (The 2024-era articles claiming panic=abort disables
`-fwasm-exceptions` are stale; that path was removed.)

Cargo automatically uses `panic = "unwind"` for the test profile, so
`cargo test` is unaffected by these settings.

## Frame loop

Usagi's wasm build uses `emscripten_set_main_loop_arg` (not ASYNCIFY). The
browser drives the per-frame body at requestAnimationFrame rate. ASYNCIFY was
tried earlier and rejected because it conflicts with `-fwasm-exceptions` and
adds runtime overhead. The session struct owns all per-frame state so we can
hand a single `&mut Session` to emscripten's main loop.

## Game data: runtime is decoupled from the game

The wasm runtime is game-agnostic. It does NOT have a game baked in via
`--preload-file`. Instead:

1. The JS shell (`web/shell.html`) fetches `game.usagi` (overridable via
   `window.USAGI_BUNDLE_URL`) over HTTP after the runtime initializes.
2. JS writes the bytes into the wasm virtual FS at `/game.usagi` using
   `Module.FS.writeFile` (which is why `FS` is in `EXPORTED_RUNTIME_METHODS`).
3. JS calls `Module.callMain([])`. Rust's `main()` on the emscripten target
   loads `/game.usagi`, builds a `BundleBacked` vfs, and runs.

This is the same `.usagi` bundle format `usagi export --target bundle`
produces on native. So the build artifacts in `target/web/` are:

| File         | Role                                     | Game-specific? |
| ------------ | ---------------------------------------- | -------------- |
| `index.html` | Shell with click-to-play overlay         | No             |
| `usagi.js`   | Emscripten loader / glue                 | No             |
| `usagi.wasm` | Usagi runtime                            | No             |
| `game.usagi` | Bundled `main.lua` + `sprites.png` + sfx | **Yes**        |

To swap games you only need to replace `game.usagi`; the other three files are
reusable across games. That's also what makes `usagi export --target web`
viable without an emcc rebuild on the user's machine.

## Shipping a game (to itch.io etc.)

`usagi export <path>` produces all targets at once by default:

```sh
usagi export path/to/your/game
# -> ./export/{your_game-linux.zip, your_game-macos.zip, your_game-windows.zip, your_game-web.zip, your_game.usagi}
```

For just the web slice:

```sh
usagi export path/to/your/game --target web
# -> ./your_game-web.zip  (unzip and upload)
```

The web output contains `index.html`, `usagi.js`, `usagi.wasm`, and
`game.usagi`. Zip it and upload to an itch.io HTML5 project page (set "This file
will be played in the browser" and point to the zip). itch serves `index.html`
from the zip root.

### Where the runtime comes from

Release-built `usagi` (e.g. `cargo build --release`, `just install`) has the
wasm runtime _embedded_ in the native binary at compile time. So an installed
`usagi` produces a working `web/` from anywhere on the user's machine, no source
checkout, no emcc, no `target/web/`.

Debug builds (`cargo run -- export ...`) skip the embed to keep dev fast, and
fall back to reading `target/web/` from disk. So the dev loop in this checkout
is:

```sh
just build-web-release   # builds wasm, populates target/web/
cargo build --release    # rebuilds native, embeds the wasm
                         # -> 3-4 MB binary including the runtime
```

Re-run `just build-web-release` whenever the runtime source changes; the next
`cargo build --release` picks it up via build.rs's `rerun-if-changed`. See
[build.rs](../build.rs) for the embed mechanics.

## Developing and testing examples in the browser

Quickstart:

1. One-time setup (per machine):
   - `bash setup-emscripten.sh` (installs emsdk to `~/.local/share/emsdk`).
   - `just setup-web` (adds the wasm target and a tiny static server).
2. Each new shell session, source emsdk so `emcc` is on PATH:
   ```sh
   source ~/.local/share/emsdk/emsdk_env.sh
   ```
3. Build + serve at `http://localhost:3535`:
   ```sh
   just serve-web
   ```
   Or just build (no server):
   ```sh
   just build-web        # debug
   just build-web-release # release, much smaller, no source maps
   ```

### Running a different example

The runtime in `target/web/` is game-agnostic. To swap the game without
rebuilding the runtime:

```sh
just example-web spr           # rebundles examples/spr -> target/web/game.usagi
# refresh the browser tab
```

This works for any path that `usagi export --target bundle` accepts: a directory with
a `main.lua`, a single `.lua` file, etc. If you change something in the bundled
game's source, rerun `just example-web <name>` and refresh.

Skip `just build-web` between examples; the runtime is identical across games.

### Smoke-testing checklist

Web bugs love to hide in code paths native doesn't exercise. Before declaring a
web build done, in the browser:

1. Confirm the click-to-play overlay appears, then dismisses.
2. Confirm the game renders a frame (snake or whatever).
3. **Play a sound.** miniaudio's WebAudio init busy-waits via
   `emscripten_sleep`, which only fires on the first sfx playback. A silent
   smoke test passes even when ASYNCIFY is missing, then the game aborts the
   moment it tries to beep.
4. Read input (arrows, gamepad). Some keys behave differently in browser vs
   native.

### Debugging

- **Console first.** Open DevTools and watch the Console; raylib trace logs and
  `[usagi]` messages land there. Lua runtime errors show up too.
- **Source maps work for debug builds.** Set breakpoints in `usagi.wasm` /
  `usagi.js` from the Sources panel.
- **Check `Module.audioContext.state`** in the console after clicking the start
  overlay to confirm audio is `running`, not `suspended`.
- **Tab focus and input.** raylib's GLFW backend reads from the canvas. Click
  the canvas (after dismissing the start overlay) to give it focus before
  testing keyboard.
- **No live-reload on web.** Edit Lua, rebuild, refresh the browser tab. (Native
  `usagi dev` is the right loop for fast iteration; use web for smoke tests and
  shipping.)
- **No `usagi tools` on web.** Tools are native-only.
- **Stuck on "Loading…"?** The runtime didn't reach `onRuntimeInitialized`. Look
  in the DevTools Console / Network panels for a wasm instantiation failure or a
  404 on `usagi.wasm` / `usagi.js`.
- **"Failed to load" overlay?** `game.usagi` couldn't be fetched. The message
  under the button is the fetch error (typically a 404 if you haven't run
  `just build-web` or `just example-web`).

### What the recipes do

- `just build-web` builds the wasm runtime, copies `usagi.{wasm,js}` and
  `web/shell.html` (as `index.html`) into `target/web/`, and bundles
  `examples/snake` as `target/web/game.usagi`.
- `just example-web <name>` rebundles a different example into
  `target/web/game.usagi` without rebuilding the runtime.
- `just serve-web` does `build-web` and starts `simple-http-server` on
  `${PORT:=3535}`.

Files in `target/web/` are overwritten on every build, so don't edit them in
place.

## Click-to-play overlay

`web/shell.html` shows a "Click to play" overlay that holds `Module.main()`
until the user clicks. It does three things on click:

1. Fetches `game.usagi` over HTTP and writes it to the wasm virtual FS.
2. Resumes any suspended AudioContext (browsers gate audio behind a user
   gesture).
3. Calls `Module.callMain([])` to start the game.

We use `Module.noInitialRun = true` so emscripten doesn't auto-run main() the
moment the runtime initializes; the click handler drives it instead. The bundle
URL defaults to `game.usagi` (relative to the page); set
`window.USAGI_BUNDLE_URL` before loading `usagi.js` to override.
