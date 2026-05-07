#!/usr/bin/env bash
# Build the shader demo for web and validate CRT/Game Boy shaders in a
# headless Chromium browser.

set -euo pipefail

RELEASE=0
PORT="${PORT:-3535}"
DEBUG_PORT="${DEBUG_PORT:-9223}"
BROWSER_PATH="${BROWSER_PATH:-}"
EMSDK_ARGS=()

usage() {
  cat <<EOF
Usage: scripts/smoke_web_shader.sh [--release] [--port PORT] [--debug-port PORT] [--browser PATH] [--emsdk-dir PATH]

Builds examples/shader for web, serves target/web/, then runs a headless
browser smoke test that compiles CRT and Game Boy generic shaders under WebGL.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release)
      RELEASE=1
      shift
      ;;
    --port)
      PORT="${2:?missing port after --port}"
      shift 2
      ;;
    --debug-port)
      DEBUG_PORT="${2:?missing port after --debug-port}"
      shift 2
      ;;
    --browser)
      BROWSER_PATH="${2:?missing browser path after --browser}"
      shift 2
      ;;
    --emsdk-dir)
      EMSDK_ARGS+=(--emsdk-dir "${2:?missing path after --emsdk-dir}")
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "[usagi] unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

require_node_probe_support() {
  if ! command -v node >/dev/null 2>&1; then
    echo "[usagi] Node.js is required for the Chrome DevTools smoke probe." >&2
    exit 1
  fi
  if ! node -e 'process.exit(typeof WebSocket === "function" ? 0 : 1)'; then
    echo "[usagi] Node.js 22+ is required because the smoke probe uses the built-in WebSocket client." >&2
    exit 1
  fi
}

pick_port() {
  node -e '
const net = require("node:net");
const start = Number(process.argv[1]);
function tryPort(port) {
  const server = net.createServer();
  server.unref();
  server.on("error", () => tryPort(port + 1));
  server.listen(port, "127.0.0.1", () => {
    console.log(port);
    server.close();
  });
}
tryPort(start);
' "$1"
}

resolve_browser() {
  if [[ -n "$BROWSER_PATH" ]]; then
    printf '%s\n' "$BROWSER_PATH"
    return
  fi

  for candidate in \
    google-chrome \
    google-chrome-stable \
    chromium \
    chromium-browser \
    microsoft-edge \
    msedge; do
    if command -v "$candidate" >/dev/null 2>&1; then
      command -v "$candidate"
      return
    fi
  done

  echo "[usagi] Chrome, Chromium, or Edge was not found. Pass --browser PATH." >&2
  exit 1
}

wait_http_ok() {
  local url="$1"
  node -e '
const url = process.argv[1];
const deadline = Date.now() + 20000;
(async function wait() {
  while (Date.now() < deadline) {
    try {
      const res = await fetch(url);
      if (res.status === 200) return;
    } catch (_) {}
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  console.error(`Timed out waiting for ${url}`);
  process.exit(1);
})();
' "$url"
}

wait_chrome_debug() {
  local port="$1"
  node -e '
const port = process.argv[1];
const deadline = Date.now() + 20000;
(async function wait() {
  while (Date.now() < deadline) {
    try {
      const res = await fetch(`http://127.0.0.1:${port}/json/version`);
      if (res.status === 200) return;
    } catch (_) {}
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  console.error(`Timed out waiting for Chrome remote debugging on port ${port}`);
  process.exit(1);
})();
' "$port"
}

SERVER_PID=""
BROWSER_PID=""
cleanup() {
  if [[ -n "$BROWSER_PID" ]] && kill -0 "$BROWSER_PID" >/dev/null 2>&1; then
    kill "$BROWSER_PID" >/dev/null 2>&1 || true
  fi
  if [[ -n "$SERVER_PID" ]] && kill -0 "$SERVER_PID" >/dev/null 2>&1; then
    kill "$SERVER_PID" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

require_node_probe_support

BUILD_ARGS=(--example shader)
if [[ "$RELEASE" -eq 1 ]]; then
  BUILD_ARGS+=(--release)
fi
BUILD_ARGS+=("${EMSDK_ARGS[@]}")
bash scripts/build_web.sh "${BUILD_ARGS[@]}"

SERVE_PORT="$(pick_port "$PORT")"
REMOTE_DEBUG_PORT="$(pick_port "$DEBUG_PORT")"
SMOKE_ROOT="target/web-shader-smoke"
CHROME_PROFILE="$SMOKE_ROOT/chrome-profile"
rm -rf "$CHROME_PROFILE"
mkdir -p "$CHROME_PROFILE"

PORT="$SERVE_PORT" just serve-web >"$SMOKE_ROOT/server.out.log" 2>"$SMOKE_ROOT/server.err.log" &
SERVER_PID="$!"
wait_http_ok "http://127.0.0.1:$SERVE_PORT/"
wait_http_ok "http://127.0.0.1:$SERVE_PORT/usagi.js"
wait_http_ok "http://127.0.0.1:$SERVE_PORT/usagi.wasm"
wait_http_ok "http://127.0.0.1:$SERVE_PORT/game.usagi"

BROWSER="$(resolve_browser)"
"$BROWSER" \
  --headless=new \
  "--remote-debugging-port=$REMOTE_DEBUG_PORT" \
  "--user-data-dir=$CHROME_PROFILE" \
  --no-first-run \
  --no-default-browser-check \
  --enable-webgl \
  --ignore-gpu-blocklist \
  about:blank >"$SMOKE_ROOT/browser.out.log" 2>"$SMOKE_ROOT/browser.err.log" &
BROWSER_PID="$!"

wait_chrome_debug "$REMOTE_DEBUG_PORT"

node scripts/smoke_web_shader_probe.js \
  --url "http://127.0.0.1:$SERVE_PORT/" \
  --debug-port "$REMOTE_DEBUG_PORT" \
  --out-dir "$SMOKE_ROOT"
