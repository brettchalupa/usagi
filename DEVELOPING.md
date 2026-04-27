# Developing usagi

Guide to how to work on the engine.

## Dependencies

Install Rust: https://rustup.rs

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
3. Bump `version` in `Cargo.toml` and run `cargo update -p usagi` to refresh
   `Cargo.lock` before tagging. The tag should match the manifest version.
4. Update CHANGELOG.md

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

| File                               | Target                                  |
| ---------------------------------- | --------------------------------------- |
| `usagi-<ver>-linux-x86_64.tar.gz`  | Linux x86_64, glibc 2.35+               |
| `usagi-<ver>-macos-aarch64.tar.gz` | macOS, Apple Silicon                    |
| `usagi-<ver>-windows-x86_64.zip`   | Windows 10+                             |
| `usagi-<ver>-wasm.tar.gz`          | Web runtime (`usagi.js` + `usagi.wasm`) |

### Post Release

After the release is made, bump the version in `Cargo.toml` to the next version
that will be worked on and add the `-dev` suffix. So if `v1.1.0` was just
released, update it to `1.2.0-dev` and run `cargo update -p usagi`. Commit and
push this to GitHub. This helps make it clear that what's on `main` is not the
published version nor the upcoming version (yet).

## Build Environment Notes

- The Linux runner is `ubuntu-22.04` (glibc 2.35) for portability. Binaries
  should run on Debian 12+, RHEL 9+, Fedora, Arch, openSUSE Leap 15.4+.
- `macos-latest` is Apple Silicon. No Intel mac binary is produced.
