# Developing usagi

Guide to how to work on the engine.

## Style

The Rust code uses the default styles from `cargo fmt`.

Pull requests will be renamed to follow
[the Conventional Commits spec](https://www.conventionalcommits.org/en/v1.0.0/).

### Lua style

- 2 spaces for indentation. `snake_case` for locals, function names, table
  fields, and helper module names (e.g. `enemy.lua`, `local fresh_state`).
- `SCREAMING_SNAKE_CASE` for compile-time-ish constants (file-scope
  `local TICK = 0.12`, `local MAX_BULLETS = 12`, the engine's `gfx.COLOR_*`
  table). Distinguishes "tunable knob" from "runtime variable."
- Engine API is lowercase (`gfx`, `input`, `sfx`, `music`, `usagi`). It's
  declared in `meta/usagi.lua` so the LSP treats it as predefined; reads in user
  code don't trip `lowercase-global`.
- **Cross-frame globals are `Capitalized`.** This includes the canonical
  game-state container (`State = { ... }` set inside `_init`) and module imports
  kept as globals (`Player = require("player")`). The capitalization signals
  "intentional global, lives across reloads"; anything lowercase at file scope
  is treated by `lowercase-global` as an accident (forgot a `local`).
- Why `State` is a global: live reload re-execs the chunk on every saved edit. A
  `local State` at module scope would get re-bound to a fresh table every save
  and obliterate the running game. Setting `State` in `_init` (and only in
  `_init`, which only runs at startup and on F5) lets the table outlive reloads.
- If you have a real cross-frame need that isn't `State`, the convention scales:
  capitalize and document it. Module-bound require results (`Enemy`, `Bullet`)
  are the common second case.

This pattern (engine-API lowercase / game-state capitalized / locals snake_case)
is the same one shipped in `.luarc.json` and the `usagi init` template, and is
what every example under `examples/` follows.

## Dependencies

Install Rust: https://rustup.rs

## Docs

Documentation is written in Markdown. It's formatted with `deno fmt`, but it's
not a huge deal if there's drift there, so CI doesn't enforce this.

Changes to the README should be made in `README_DEV.md`, which is used to stage
docs for the next release. The `README.md` is for the most recently released
version of Usagi, so that when people come to GitHub they don't see
documentation for features that aren't in a shipped version yet.

Only update `README.md` too when fixing a typo or adding something that's not
version-specific.

`CHANGELOG.md` contains a list of changes for each version. Pre-release notes
get squashed into the full release when it happens.

## Local

After cloning, run `just setup` once. It points git at `.githooks/`, so
`just ok` runs automatically before every push (skip with `--no-verify` in a
pinch).

`just ok` runs fmt, clippy, and tests. Run before any commit.

`just example <name>` boots an example in dev mode with live reload.

While developing Usagi itself, replace `usagi` with `cargo run --` (for example
`cargo run -- dev examples/hello_usagi.lua`).

`just build-web` then `just serve-web` builds the wasm runtime and serves it
locally on port 3535. Needs emscripten on PATH; run `./setup-emscripten.sh` once
to install it on Fedora. `brew install emscripten` works on macOS.

See `justfile` for the full list of recipes.

## Developing

- `just run` - run hello_usagi example
- `just ok` - run all checks
- `just fmt` - format Rust code
- `just serve-web` - build and serve the web build at <http://localhost:3535>
  (requires `emcc` on PATH; see [docs/web-build.md](docs/web-build.md))

## Testing the Web Build Locally

`usagi export --target web` normally fetches a runtime template from the
matching release tag. On a `-dev` build there's no published release, so point
the CLI at the locally-built runtime instead. `--template-path` accepts an
extracted directory in addition to an archive:

```sh
just build-web                          # one-time: build the wasm runtime
cargo run -- export examples/snake --target web \
    --template-path target/wasm32-unknown-emscripten/release
```

That writes `snake-web.zip` in the cwd. To run it in a browser:

```sh
unzip -d /tmp/snake-web snake-web.zip
simple-http-server --index --nocache -p 3535 /tmp/snake-web
# open http://localhost:3535
```

`simple-http-server` is the same server `just serve-web` uses (installed by
`just setup-web`).

`shell.html` is auto-picked up from `web/shell.html` in the source tree when
running from the repo root, so you don't need to stage it. Pass
`--web-shell PATH` (or drop a `shell.html` next to your script) to use a custom
one.

## CI (`.github/workflows/ci.yml`)

Runs on every push to `main` and every pull request. Three jobs:

- `check`: matrix of Linux (`ubuntu-22.04`), macOS (`macos-latest`, Apple
  Silicon), and Windows (`windows-latest`). Runs fmt, clippy, tests, and a
  release build. Uploads the binary as an artifact.
- `web`: builds the emscripten wasm runtime. Uploads `usagi.js` and
  `usagi.wasm`.

CI artifacts expire after 90 days and require a GitHub login to download. Use
them for spot-checking a PR. For distribution, cut a release.

## Releases (`.github/workflows/release.yml`)

### Release Prep

1. Run `just ok` to ensure all checks pass
2. Run `just examples` to verify everything is working as expected
3. Test syntax error recovery in a `require` to ensure it works
4. Test `usagi tools`
5. Verify web builds work
6. Verify gamepads work
7. Bump `version` in `Cargo.toml` and run `cargo update -p usagi` to refresh
   `Cargo.lock` before tagging. The tag should match the manifest version
8. Update the current version in README_DEV.md
9. Update CHANGELOG.md
10. `cp README_DEV.md README.md`

### Tagging

Push a tag matching `v*` to trigger a release build:

```sh
git tag v0.1.0
git push origin v0.1.0
```

### Publishing the Release

The workflow builds release binaries on every supported target, packages them,
and creates a GitHub Release **as a draft** with the archives attached. Once the
workflow finishes, there will be a draft release.

Copy the Changelog notes and assets on the Releases page, then click Publish to
make it public. Tags containing a hyphen (`v0.1.0-dev.1`) are flagged as
prereleases following semver convention.

### Release Artifacts

| File                               | Target                                               |
| ---------------------------------- | ---------------------------------------------------- |
| `usagi-<ver>-linux-x86_64.tar.gz`  | Linux x86_64, glibc 2.35+                            |
| `usagi-<ver>-macos-aarch64.tar.gz` | macOS, Apple Silicon                                 |
| `usagi-<ver>-windows-x86_64.zip`   | Windows 10+                                          |
| `usagi-<ver>-wasm.tar.gz`          | Web runtime (`usagi.js` + `usagi.wasm` + shell.html) |

Each artifact also publishes a `<file>.sha256` sidecar (sha256sum format).
`usagi export` fetches the sidecar alongside the archive and verifies before
extraction; mismatches fail loudly.

Filenames carry the architecture so future arm/x86 splits drop in without
renaming. `usagi export` resolves `--target linux` to the matching artifact via
the URL convention `${USAGI_TEMPLATE_BASE}/v<ver>/<file>`.

### Post Release

After the release is made, bump the version in `Cargo.toml` to the next version
that will be worked on and add the `-dev` suffix. So if `v1.1.0` was just
released, update it to `1.2.0-dev` and run `cargo update -p usagi`. Commit and
push this to GitHub. This helps make it clear that what's on `main` is not the
published version nor the upcoming version (yet).

Push a new version of notetris to itch.io: `./examples/notetris/push.rb`

## Build Environment Notes

- The Linux runner is `ubuntu-22.04` (glibc 2.35) for portability. Binaries
  should run on Debian 12+, RHEL 9+, Fedora, Arch, openSUSE Leap 15.4+.
- `macos-latest` is Apple Silicon. No Intel mac binary is produced.

## Web Build (wasm32-unknown-emscripten) Notes

The non-obvious bits of getting Usagi to compile and link for the web. If the
web build breaks, start here.

### Toolchain

- Stable Rust (currently 1.95.0 verified). No `rust-toolchain.toml` pin.
- Emscripten via emsdk. `setup-emscripten.sh` installs to `$XDG_DATA_HOME/emsdk`
  (or `~/.local/share/emsdk`); source `~/.local/share/emsdk/emsdk_env.sh` to put
  `emcc` on `PATH`. On macOS, you can do `brew install emscripten`.
- emcc 5.0.6 verified.

Build with `just build-web` (or `just build-web-release`).

### The wasm exception ABI: what you need to know

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

#### Three places to keep in sync

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

#### What `panic = "abort"` is and isn't doing

The `[profile.dev]` and `[profile.release]` `panic = "abort"` settings in
`Cargo.toml` are there for binary size, on all targets. They are NOT what makes
the web build link. (The 2024-era articles claiming panic=abort disables
`-fwasm-exceptions` are stale; that path was removed.)

Cargo automatically uses `panic = "unwind"` for the test profile, so
`cargo test` is unaffected by these settings.

### Frame loop

Usagi's wasm build uses `emscripten_set_main_loop_arg` (not ASYNCIFY). The
browser drives the per-frame body at requestAnimationFrame rate. ASYNCIFY was
tried earlier and rejected because it conflicts with `-fwasm-exceptions` and
adds runtime overhead. The session struct owns all per-frame state so it can
hand a single `&mut Session` to emscripten's main loop.

### Game data: runtime is decoupled from the game

The wasm runtime is game-agnostic. It does NOT have a game baked in via
`--preload-file`. Instead:

1. The JS shell (`web/shell.html`) fetches `game.usagi` (overridable via
   `window.USAGI_BUNDLE_URL`) over HTTP after the runtime initializes.
2. JS writes the bytes into the wasm virtual FS at `/game.usagi` using
   `Module.FS.writeFile` (which is why `FS` is in `EXPORTED_RUNTIME_METHODS`).
3. JS calls `Module.callMain([])`. Rust's `main()` on the emscripten target
   loads `/game.usagi`, builds a `BundleBacked` vfs, and runs.

This is the same `.usagi` bundle format `usagi export --target bundle` produces
on native. So the build artifacts in `target/web/` are:

| File         | Role                                     | Game-specific? |
| ------------ | ---------------------------------------- | -------------- |
| `index.html` | Shell with click-to-play overlay         | No             |
| `usagi.js`   | Emscripten loader / glue                 | No             |
| `usagi.wasm` | Usagi runtime                            | No             |
| `game.usagi` | Bundled `main.lua` + `sprites.png` + sfx | **Yes**        |

To swap games you only need to replace `game.usagi`; the other three files are
reusable across games. That's also what makes `usagi export --target web` viable
without an emcc rebuild on the user's machine.

### Developing and testing examples in the browser

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

#### Running a different example

The runtime in `target/web/` is game-agnostic. To swap the game without
rebuilding the runtime:

```sh
just example-web spr           # rebundles examples/spr -> target/web/game.usagi
# refresh the browser tab
```

This works for any path that `usagi export --target bundle` accepts: a directory
with a `main.lua`, a single `.lua` file, etc. If you change something in the
bundled game's source, rerun `just example-web <name>` and refresh.

Skip `just build-web` between examples; the runtime is identical across games.

### Click-to-play overlay

`web/shell.html` shows a "Click to play" overlay that holds `Module.main()`
until the user clicks. It does three things on click:

1. Fetches `game.usagi` over HTTP and writes it to the wasm virtual FS.
2. Resumes any suspended AudioContext (browsers gate audio behind a user
   gesture).
3. Calls `Module.callMain([])` to start the game.

`Module.noInitialRun = true` is used so emscripten doesn't auto-run main() the
moment the runtime initializes; the click handler drives it instead. The bundle
URL defaults to `game.usagi` (relative to the page); set
`window.USAGI_BUNDLE_URL` before loading `usagi.js` to override.
