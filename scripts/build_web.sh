#!/usr/bin/env bash
# Build the reusable web runtime plus a default game bundle into target/web/.

set -euo pipefail

RELEASE=0
EXAMPLE="snake"
EMSDK_DIR="${EMSDK_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/emsdk}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release)
      RELEASE=1
      shift
      ;;
    --example)
      EXAMPLE="${2:?missing example name after --example}"
      shift 2
      ;;
    --emsdk-dir)
      EMSDK_DIR="${2:?missing path after --emsdk-dir}"
      shift 2
      ;;
    -h|--help)
      cat <<EOF
Usage: scripts/build_web.sh [--release] [--example NAME] [--emsdk-dir PATH]

Builds target/web/{index.html,usagi.js,usagi.wasm,game.usagi}.
EOF
      exit 0
      ;;
    *)
      echo "[usagi] unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

if ! command -v emcc >/dev/null 2>&1; then
  if [[ -f "$EMSDK_DIR/emsdk_env.sh" ]]; then
    # shellcheck disable=SC1091
    source "$EMSDK_DIR/emsdk_env.sh" >/dev/null
  else
    echo "[usagi] emcc not on PATH and no emsdk at $EMSDK_DIR. Run scripts/setup_emscripten.sh first." >&2
    exit 1
  fi
fi

export EMCC_CFLAGS="-fwasm-exceptions -sSUPPORT_LONGJMP=wasm -s USE_LIBPNG=1 -s USE_OGG=1 -s USE_VORBIS=1"

PROFILE_DIR="debug"
CARGO_BUILD=(cargo build --target wasm32-unknown-emscripten)
CARGO_EXPORT=(cargo run --quiet --)
if [[ "$RELEASE" -eq 1 ]]; then
  PROFILE_DIR="release"
  CARGO_BUILD+=(--release)
  CARGO_EXPORT=(cargo run --release --quiet --)
fi

"${CARGO_BUILD[@]}"

mkdir -p target/web
rm -rf target/web/*
cp web/shell.html target/web/index.html
cp "target/wasm32-unknown-emscripten/$PROFILE_DIR/usagi.wasm" target/web/
cp "target/wasm32-unknown-emscripten/$PROFILE_DIR/usagi.js" target/web/
"${CARGO_EXPORT[@]}" export "examples/$EXAMPLE" --target bundle -o target/web/game.usagi

echo "[usagi] wrote target/web/index.html, usagi.js, usagi.wasm, and game.usagi"
