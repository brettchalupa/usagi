//! Embed the prebuilt web runtime files into the native release binary so
//! `usagi compile --target web` (and the default `--target all`) can write
//! a shippable web build from anywhere, without needing the project source
//! checkout or emcc on the user's machine.
//!
//! Behavior depends on `PROFILE`:
//!
//! - **release**: copy `usagi.{wasm,js}` from
//!   `target/wasm32-unknown-emscripten/release/` and `web/shell.html` into
//!   `OUT_DIR/web_runtime/`. If those files don't exist yet, write empty
//!   stubs and emit a build warning. The release binary then carries
//!   ~5 MB of wasm runtime; that's the cost of `usagi compile --target web`
//!   working out of the box.
//! - **debug**: always write empty stubs. Keeps `cargo build` and
//!   `just ok` fast. `--target web` falls back to reading the runtime
//!   from `target/web/` on disk in debug mode (see `compile_web` in
//!   `src/main.rs`); with that fallback in place a debug build still
//!   works locally.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR not set by cargo"));
    let runtime_out = out_dir.join("web_runtime");
    fs::create_dir_all(&runtime_out).expect("create OUT_DIR/web_runtime");

    let manifest = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set by cargo"),
    );
    let profile = env::var("PROFILE").unwrap_or_default();
    let target = env::var("TARGET").unwrap_or_default();

    // Skip the embed when we're building the wasm runtime itself: the
    // wasm binary doesn't have a CLI, so it doesn't need the runtime
    // baked in, and we'd be looking for wasm artifacts that don't exist
    // yet (chicken and egg).
    let cross_to_wasm = target.contains("emscripten");
    let embed = profile == "release" && !cross_to_wasm;

    println!("cargo:rerun-if-changed=build.rs");

    if embed {
        // Native release: the cargo target dir is OUT_DIR.ancestors()[4]
        // (target/<profile>/build/<crate>-<hash>/out -> target).
        let target_dir = out_dir
            .ancestors()
            .nth(4)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| manifest.join("target"));
        let wasm_src = target_dir.join("wasm32-unknown-emscripten/release/usagi.wasm");
        let js_src = target_dir.join("wasm32-unknown-emscripten/release/usagi.js");
        let html_src = manifest.join("web/shell.html");
        println!("cargo:rerun-if-changed={}", html_src.display());
        println!("cargo:rerun-if-changed={}", wasm_src.display());
        println!("cargo:rerun-if-changed={}", js_src.display());
        copy_or_stub(&wasm_src, &runtime_out.join("usagi.wasm"));
        copy_or_stub(&js_src, &runtime_out.join("usagi.js"));
        copy_or_stub(&html_src, &runtime_out.join("index.html"));
    } else {
        write_stub(&runtime_out.join("usagi.wasm"));
        write_stub(&runtime_out.join("usagi.js"));
        write_stub(&runtime_out.join("index.html"));
    }
}

fn copy_or_stub(src: &Path, dst: &Path) {
    match fs::copy(src, dst) {
        Ok(_) => {}
        Err(_) => {
            println!(
                "cargo:warning=usagi: web runtime artifact not found at {}. \
                 `usagi compile --target web` (and `--target all`) will error \
                 from this binary until you run `just build-web-release` and rebuild.",
                src.display()
            );
            write_stub(dst);
        }
    }
}

fn write_stub(dst: &Path) {
    fs::write(dst, []).expect("write empty stub");
}
