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

# Release-build the usagi binary and copy it to ~/.local/bin/ for testing.
install:
    cargo build --release
    mkdir -p ~/.local/bin
    cp target/release/usagi ~/.local/bin/
    @echo "[usagi] installed to ~/.local/bin/usagi"

# Run a specific example in dev mode (live-reload on).
# Works for flat files (`just example hello_usagi`) and directory examples
# (`just example spr` -> examples/spr/main.lua).
example name:
    cargo run -- dev examples/{{ name }}

examples:
    just example hello_usagi
    just example input
    just example spr
    just example sound
