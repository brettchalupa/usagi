run:
    cargo run -- dev examples/hello_usagi.lua

ok:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test

test:
    cargo test

fmt:
    cargo fmt

# Open the Usagi tools window. Optional path is forwarded to `usagi tools`.

# Example: `just tools examples/spr`.
tools *args:
    cargo run -- tools {{ args }}

# Compile a game to a standalone executable. Example: `just compile examples/snake`.
compile *args:
    cargo run -- compile {{ args }}

# One-time toolchain setup for the web build: emscripten + wasm target +
# a tiny static server.
#
# emscripten itself is not installed by this recipe — install it from
# <https://emscripten.org/docs/getting_started/downloads.html>. After

# install, run `source emsdk_env.sh` in your shell so `emcc` is on PATH.
setup-web:
    rustup target add wasm32-unknown-emscripten
    cargo install simple-http-server

# Build the web (wasm) runtime. The runtime is game-agnostic: the bundle
# (game.usagi) is fetched by the JS shell at startup, so swapping games
# does not require an emcc rebuild. `just example-web <name>` rebundles
# without touching the runtime.
#
# Requires `emcc` on PATH (see `just setup-web` for the setup notes).
# `EMCC_CFLAGS` extends emcc's default flags for *every* invocation
# (emscripten ports, raylib's CMake, etc.). See `docs/web-build.md` for
# the wasm-eh ABI rationale.
build-web:
    bash -c 'set -e; \
      command -v emcc >/dev/null 2>&1 || source ~/.local/share/emsdk/emsdk_env.sh >/dev/null 2>&1 || { echo "[usagi] emcc not on PATH and no emsdk at ~/.local/share/emsdk/. Run setup-emscripten.sh first." >&2; exit 1; }; \
      EMCC_CFLAGS="-fwasm-exceptions -sSUPPORT_LONGJMP=wasm -s USE_LIBPNG=1 -s USE_OGG=1 -s USE_VORBIS=1" \
        cargo build --target wasm32-unknown-emscripten'
    mkdir -p target/web
    rm -rf target/web/*
    cp web/shell.html target/web/index.html
    cp target/wasm32-unknown-emscripten/debug/usagi.wasm target/web/
    cp target/wasm32-unknown-emscripten/debug/usagi.js target/web/
    cargo run --quiet -- compile examples/snake --target bundle -o target/web/game.usagi

build-web-release:
    bash -c 'set -e; \
      command -v emcc >/dev/null 2>&1 || source ~/.local/share/emsdk/emsdk_env.sh >/dev/null 2>&1 || { echo "[usagi] emcc not on PATH and no emsdk at ~/.local/share/emsdk/. Run setup-emscripten.sh first." >&2; exit 1; }; \
      EMCC_CFLAGS="-fwasm-exceptions -sSUPPORT_LONGJMP=wasm -s USE_LIBPNG=1 -s USE_OGG=1 -s USE_VORBIS=1" \
        cargo build --release --target wasm32-unknown-emscripten'
    mkdir -p target/web
    rm -rf target/web/*
    cp web/shell.html target/web/index.html
    cp target/wasm32-unknown-emscripten/release/usagi.wasm target/web/
    cp target/wasm32-unknown-emscripten/release/usagi.js target/web/
    cargo run --release --quiet -- compile examples/snake --target bundle -o target/web/game.usagi

# Rebundle target/web/game.usagi from a different example without
# rebuilding the runtime. Refresh the browser tab to load the new game.
# Example: `just example-web spr`.
example-web name:
    cargo run --quiet -- compile examples/{{ name }} --target bundle -o target/web/game.usagi
    @echo "[usagi] target/web/game.usagi swapped to examples/{{ name }}"

# Serve target/web/ locally on port 3535. Does NOT rebuild; pair with
# `just build-web` (one-time runtime build) and `just example-web <name>`
# (swap games without clobbering the runtime).
serve-web:
    simple-http-server --index --nocache -p ${PORT:=3535} target/web

# Smoke-test a `.usagi` bundle: compile it, then run via `usagi run`.
# Drops the bundle file in the cwd. Example: `just bundle snake`.
bundle name:
    cargo run --quiet -- compile examples/{{ name }} --target bundle
    cargo run --quiet -- run {{ name }}.usagi

# Release-build the usagi binary and copy it to ~/.local/bin/ for testing.
install:
    cargo build --release
    mkdir -p ~/.local/bin
    cp target/release/usagi ~/.local/bin/
    @echo "[usagi] installed to ~/.local/bin/usagi"

# Run a specific example in dev mode (live-reload on). Works for flat files
# (`just example hello_usagi`) and directory examples `just example spr` ->

# examples/spr/main.lua.
example name:
    cargo run -- dev examples/{{ name }}

examples:
    just example hello_usagi
    just example input
    just example input_test
    just example spr
    just example sound
    just example snake
