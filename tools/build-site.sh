#!/bin/sh
# Builds the static site for Cloudflare into dist/: compiles the renderer to WebAssembly,
# then renders the default view and copies the data and assets (src/site.rs).
set -eu
cd "$(dirname "$0")/.."
cargo rustc --lib --release --target wasm32-unknown-unknown --crate-type cdylib
cargo run --release -- site target/wasm32-unknown-unknown/release/canada_job_tracker.wasm dist
