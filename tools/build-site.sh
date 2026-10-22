#!/bin/sh
# Builds the static site for Cloudflare into dist/: compiles the renderer to WebAssembly,
# then renders the default view and copies the data and assets (src/site.rs).
# Cloudflare's builds (Workers Builds) run this through `npm run build`.
set -eu
cd "$(dirname "$0")/.."
# Cloudflare's build machines don't come with Rust.
if ! command -v cargo >/dev/null 2>&1; then
	curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
	. "$HOME/.cargo/env"
fi
rustup target add wasm32-unknown-unknown
cargo rustc --lib --release --target wasm32-unknown-unknown --crate-type cdylib
cargo run --release -- site target/wasm32-unknown-unknown/release/canada_job_tracker.wasm dist
