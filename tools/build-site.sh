#!/bin/sh
# Builds the static site for Cloudflare into dist/: the page (static/index.html) and the data it
# draws, data.json (src/site.rs). Cloudflare's builds (Workers Builds) run this through `npm run build`.
set -eu
cd "$(dirname "$0")/.."
# Cloudflare's build machines don't come with Rust.
if ! command -v cargo >/dev/null 2>&1; then
	curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
	. "$HOME/.cargo/env"
fi
cargo run --release -- site dist
