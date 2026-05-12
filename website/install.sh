#!/bin/sh
# Usagi Engine installer (Linux, macOS)
#
# Usage:
#   curl -fsSL https://usagiengine.com/install.sh | sh
#   curl -fsSL https://usagiengine.com/install.sh | sh -s -- v0.7.0
#   USAGI_INSTALL=$HOME/tools curl -fsSL https://usagiengine.com/install.sh | sh
#
# Source: https://github.com/brettchalupa/usagi/blob/main/website/install.sh

set -eu

GITHUB_REPO="brettchalupa/usagi"

err() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

info() {
  printf '%s\n' "$*"
}

warn() {
  printf 'warning: %s\n' "$*" >&2
}

need() {
  command -v "$1" >/dev/null 2>&1 || err "required command not found: $1"
}

need curl
need uname
need tar
need mktemp

if [ "$(id -u)" = "0" ]; then
  warn "running as root; Usagi installs per-user and does not need root"
fi

os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
  Darwin)
    case "$arch" in
      arm64|aarch64) target="macos-aarch64" ;;
      *) err "unsupported macOS architecture: $arch (only Apple Silicon is published)" ;;
    esac
    ;;
  Linux)
    case "$arch" in
      x86_64|amd64) target="linux-x86_64" ;;
      *) err "unsupported Linux architecture: $arch (only x86_64 is published)" ;;
    esac
    ;;
  *)
    err "unsupported OS: $os"
    ;;
esac

version="${1:-${USAGI_VERSION:-}}"

if [ -z "$version" ]; then
  info "Resolving latest release..."
  resolved="$(curl -fsSLI -o /dev/null -w '%{url_effective}' \
    "https://github.com/${GITHUB_REPO}/releases/latest")"
  version="${resolved##*/}"
fi

case "$version" in
  v[0-9]*) ;;
  *) err "invalid version: '${version}' (expected vMAJOR.MINOR.PATCH)" ;;
esac

ver_no_v="${version#v}"
archive="usagi-${ver_no_v}-${target}.tar.gz"
checksum="${archive}.sha256"
base_url="https://github.com/${GITHUB_REPO}/releases/download/${version}"

install_dir="${USAGI_INSTALL:-${HOME}/.usagi}"
bin_dir="${install_dir}/bin"
exe="${bin_dir}/usagi"

mkdir -p "$bin_dir"

tmp="$(mktemp -d 2>/dev/null || mktemp -d -t usagi-install)"
trap 'rm -rf "$tmp"' EXIT INT HUP TERM

info "Installing Usagi ${version} (${target}) to ${exe}"

info "Downloading ${archive}..."
curl -fsSL --proto '=https' --tlsv1.2 -o "${tmp}/${archive}" "${base_url}/${archive}"

info "Downloading ${checksum}..."
curl -fsSL --proto '=https' --tlsv1.2 -o "${tmp}/${checksum}" "${base_url}/${checksum}"

info "Verifying checksum..."
(
  cd "$tmp"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "$checksum" >/dev/null
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -c "$checksum" >/dev/null
  else
    err "neither sha256sum nor shasum is available; cannot verify download"
  fi
) || err "checksum verification failed for ${archive}"

info "Extracting..."
tar -xzf "${tmp}/${archive}" -C "$tmp"

src="${tmp}/usagi"
if [ ! -f "$src" ]; then
  src="$(find "$tmp" -name usagi -type f 2>/dev/null | head -n 1)"
fi
[ -n "${src:-}" ] && [ -f "$src" ] || err "could not find usagi binary inside archive"

# Quarantine xattr on macOS keeps Gatekeeper from blocking the binary that the
# user just explicitly downloaded and asked to install.
if [ "$os" = "Darwin" ] && command -v xattr >/dev/null 2>&1; then
  xattr -d com.apple.quarantine "$src" 2>/dev/null || true
fi

mv "$src" "$exe"
chmod +x "$exe"

info ""
info "Installed: ${exe}"

case ":${PATH-}:" in
  *":${bin_dir}:"*) ;;
  *)
    info ""
    info "Add ${bin_dir} to your PATH by appending this line to your shell profile"
    info "(~/.bashrc, ~/.zshrc, ~/.profile, etc.):"
    info ""
    info "  export PATH=\"${bin_dir}:\$PATH\""
    info ""
    info "Then restart your shell, or run that line directly to use Usagi now."
    ;;
esac

info ""
info "Get started: usagi help"
