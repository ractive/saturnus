#!/bin/sh
# Build the transfer page's WebAssembly bindings (crates/saturnus-tx-web)
# into web/tx/pkg/ (gitignored). Needs wasm-pack and the
# wasm32-unknown-unknown target, as web/build.sh.
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
cd "$here/../crates/saturnus-tx-web"
wasm-pack build --target web --release --no-typescript --out-dir "$here/tx/pkg" "$@"
