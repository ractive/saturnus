#!/bin/sh
# Build the WebAssembly package (crates/saturnus-web: the emulator's and
# the transfer page's bindings) into web/pkg/ (gitignored).
# Needs wasm-pack (`cargo install wasm-pack`) and the wasm32-unknown-unknown
# target (`rustup target add wasm32-unknown-unknown`).
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
cd "$here/../crates/saturnus-web"
wasm-pack build --target web --release --no-typescript --out-dir "$here/pkg" "$@"
