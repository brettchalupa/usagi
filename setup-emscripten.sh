#!/usr/bin/env bash
# Install Emscripten (emsdk) so `just build-web` can produce wasm builds.
#
# Idempotent: if $EMSDK_DIR already exists this just pulls + reactivates.
# Defaults to $XDG_DATA_HOME/emsdk (or ~/.local/share/emsdk if XDG_DATA_HOME
# is unset), per the XDG Base Directory spec. Override by exporting EMSDK_DIR
# before running.
#
# Usage:
#   ./setup-emscripten.sh
#   EMSDK_DIR=/opt/emsdk ./setup-emscripten.sh

set -euo pipefail

EMSDK_DIR="${EMSDK_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/emsdk}"

if ! command -v git >/dev/null 2>&1; then
  echo "[setup] git is required but not on PATH" >&2
  exit 1
fi
if ! command -v python3 >/dev/null 2>&1 && ! command -v python >/dev/null 2>&1; then
  echo "[setup] python (3.x) is required by emsdk but not on PATH" >&2
  exit 1
fi

if [[ ! -d "$EMSDK_DIR" ]]; then
  echo "[setup] cloning emsdk to $EMSDK_DIR"
  git clone https://github.com/emscripten-core/emsdk.git "$EMSDK_DIR"
else
  echo "[setup] emsdk already at $EMSDK_DIR; updating"
  git -C "$EMSDK_DIR" pull --ff-only
fi

cd "$EMSDK_DIR"
./emsdk install latest
./emsdk activate latest

cat <<EOF

[setup] Done. To use emcc in your current shell:

    source $EMSDK_DIR/emsdk_env.sh

Add that line to your ~/.bashrc / ~/.zshrc to make it persistent.

Then in the usagi project:

    just setup-web    # adds the wasm rustup target + simple-http-server
    just build-web    # produces target/web/{usagi.wasm, usagi.js, usagi.data, index.html}
    just serve-web    # serves it at http://localhost:3535

EOF
