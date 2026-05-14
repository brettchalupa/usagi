# One-time per-clone setup: point git at .githooks/ so the pre-push hook (which runs `just ok`) fires for everyone working in this repo.
setup:
    git config core.hooksPath .githooks
    @echo "[usagi] git hooks installed (pre-push runs 'just ok')"

# Run all checks and tests
ok:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test

run:
    cargo run -- dev examples/hello_usagi.lua

test:
    cargo test

fmt:
    cargo fmt
    deno fmt *.md **/*.md **/*.html

# Open the Usagi tools window. Optional path is forwarded to `usagi tools`.

# Example: `just tools examples/spr`.
tools *args:
    cargo run -- tools {{ args }}

[doc("""
One-time toolchain setup for the web build: emscripten + wasm target +
a tiny static server.

emscripten itself is not installed by this recipe — install it from
<https://emscripten.org/docs/getting_started/downloads.html>. After
install, run `source emsdk_env.sh` in your shell so `emcc` is on PATH.
""")]
setup-web:
    rustup target add wasm32-unknown-emscripten
    cargo install simple-http-server

build:
    cargo build

build-release:
    cargo build --release

[doc("""
Build the web (wasm) runtime. The runtime is game-agnostic: the bundle
(game.usagi) is fetched by the JS shell at startup, so swapping games
does not require an emcc rebuild. `just example-web <name>` rebundles
without touching the runtime.

Requires `emcc` on PATH (see `just setup-web` for the setup notes).
`EMCC_CFLAGS` extends emcc's default flags for *every* invocation
(emscripten ports, raylib's CMake, etc.). See `docs/web-build.md` for
the wasm-eh ABI rationale.
""")]
build-web:
    bash -c 'set -e; \
      command -v emcc >/dev/null 2>&1 || source ~/.local/share/emsdk/emsdk_env.sh >/dev/null 2>&1 || { echo "[usagi] emcc not on PATH and no emsdk at ~/.local/share/emsdk/. Run scripts/setup_emscripten.sh first." >&2; exit 1; }; \
      EMCC_CFLAGS="-fwasm-exceptions -sSUPPORT_LONGJMP=wasm -s USE_LIBPNG=1 -s USE_OGG=1 -s USE_VORBIS=1" \
        cargo build --target wasm32-unknown-emscripten'
    mkdir -p target/web
    rm -rf target/web/*
    cp web/shell.html target/web/index.html
    cp web/mock-itch.html target/web/
    cp target/wasm32-unknown-emscripten/debug/usagi.wasm target/web/
    cp target/wasm32-unknown-emscripten/debug/usagi.js target/web/
    cargo run --quiet -- export examples/snake --target bundle -o target/web/game.usagi

build-web-release:
    bash -c 'set -e; \
      command -v emcc >/dev/null 2>&1 || source ~/.local/share/emsdk/emsdk_env.sh >/dev/null 2>&1 || { echo "[usagi] emcc not on PATH and no emsdk at ~/.local/share/emsdk/. Run scripts/setup_emscripten.sh first." >&2; exit 1; }; \
      EMCC_CFLAGS="-fwasm-exceptions -sSUPPORT_LONGJMP=wasm -s USE_LIBPNG=1 -s USE_OGG=1 -s USE_VORBIS=1" \
        cargo build --release --target wasm32-unknown-emscripten'
    mkdir -p target/web
    rm -rf target/web/*
    cp web/shell.html target/web/index.html
    cp web/mock-itch.html target/web/
    cp target/wasm32-unknown-emscripten/release/usagi.wasm target/web/
    cp target/wasm32-unknown-emscripten/release/usagi.js target/web/
    cargo run --release --quiet -- export examples/snake --target bundle -o target/web/game.usagi

[doc("""
Rebundle target/web/game.usagi from a different example without
rebuilding the runtime. Refresh the browser tab to load the new game.
Example: `just example-web spr`.
""")]
example-web name:
    cargo run --quiet -- export examples/{{ name }} --target bundle -o target/web/game.usagi
    @echo "[usagi] target/web/game.usagi swapped to examples/{{ name }}"

[doc("""
Serve target/web/ locally on port 3535. Does NOT rebuild; pair with
`just build-web` (one-time runtime build) and `just example-web <name>`
(swap games without clobbering the runtime).
""")]
serve-web:
    simple-http-server --index --nocache -p ${PORT:=3535} target/web

[doc("""
Package the current local web runtime + a named example as an
itch-ready zip at `target/<name>-web.zip`. Use this for pre-release
testing of the engine itself (verifies the in-development wasm
against itch's actual iframe wrapper). Example: `just package-web-zip notetris`.

Distinct from `usagi export --target web`: that ships with the
*released* engine binary; this ships with whatever's in the local
working tree, which is what you want when validating an RC. The
mock-itch.html dev harness is intentionally excluded.
""")]
package-web-zip name:
    just build-web-release
    cargo run --release --quiet -- export examples/{{ name }} --target bundle -o target/web/game.usagi
    cd target/web && rm -f ../{{ name }}-web.zip && zip -q ../{{ name }}-web.zip index.html usagi.js usagi.wasm game.usagi
    @echo "[usagi] target/{{ name }}-web.zip"

[doc("""
Package and push a named example to brettchalupa/usagi-web-test on
itch via butler. Pair with the release checklist in DEVELOPING.md
to validate web behavior in the actual itch wrapper before tagging
an engine release. Requires `butler login` once. Example:
`just push-web-test notetris`.
""")]
push-web-test name:
    just package-web-zip {{ name }}
    butler push target/{{ name }}-web.zip brettchalupa/usagi-web-test:html

# Smoke-test a `.usagi` bundle: export it, then run via `usagi run`. Drops the bundle file in the cwd. Example: `just bundle snake`.
bundle name:
    cargo run --quiet -- export examples/{{ name }} --target bundle
    cargo run --quiet -- run {{ name }}.usagi

[doc("""
Push usagi release archives to itch.io via butler. Defaults to the latest
published GitHub release; pass a tag to push a different version. Use
--dry-run to download archives without pushing.

Examples:
  just push-itch
  just push-itch v0.6.0
  just push-itch --dry-run
""")]
push-itch *args:
    ruby scripts/push_itch.rb {{ args }}

# Release-build the usagi binary and copy it to ~/.local/bin/ for testing.
install:
    cargo build --release
    mkdir -p ~/.local/bin
    cp target/release/usagi ~/.local/bin/
    @echo "[usagi] installed to ~/.local/bin/usagi"

# Run a specific example in dev mode (live-reload on). Works for flat files (`just example hello_usagi`) and directory examples `just example spr` -> examples/spr/main.lua.
example name:
    cargo run -- dev examples/{{ name }}

examples:
    just example hello_usagi
    just example shapes
    just example demoscene
    just example input
    just example input_test
    just example spr
    just example sound
    just example multifile
    just example operators
    just example snake
    just example notetris
    just example px
    just example pico8
    just example dialog
    just example music
    just example save
    just example mouse
    just example mouse_physics
    just example waypoint
    just example shader
    just example text
    just example keyboard
    just example util
    just example effect
    just example resolution/vertical
    just example resolution/high_res
    just example custom_sprite_size
    just example custom_menu
    just example custom_font
    just example palette_swap
