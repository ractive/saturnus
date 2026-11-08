#!/bin/sh
# Build the WebAssembly package (crates/saturnus-web: the emulator's and
# the transfer page's bindings) into web/pkg/ (gitignored).
# Needs wasm-pack (`cargo install wasm-pack`) and the wasm32-unknown-unknown
# target (`rustup target add wasm32-unknown-unknown`).
# SATURNUS_WEB_PKG_PREBUILT=1 keeps a web/pkg/ that is already there
# instead: desktop.yml sets it when it downloaded CI's package of the
# same commit (scripts/ci-web-pkg-run.sh), as the Tauri build runs this
# script itself (beforeBuildCommand).
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
if [ "${SATURNUS_WEB_PKG_PREBUILT:-}" = 1 ]; then
  for f in saturnus_web.js saturnus_web_bg.wasm; do
    [ -f "$here/pkg/$f" ] || { echo "web/build.sh: SATURNUS_WEB_PKG_PREBUILT=1 but no web/pkg/$f" >&2; exit 1; }
  done
  echo "web/build.sh: SATURNUS_WEB_PKG_PREBUILT=1, keeping web/pkg/ (CI's build)"
  exit 0
fi
cd "$here/../crates/saturnus-web"
wasm-pack build --target web --release --no-typescript --out-dir "$here/pkg" "$@"
