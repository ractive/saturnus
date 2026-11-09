#!/bin/sh
# Build the palmtop of Files' tests (crates/filer-test-server: filer-proto's
# Filer server over its in-memory files) into web/test/pkg-filer/
# (gitignored). A package of its own, so the app's web/pkg/ (web/build.sh,
# what the site ships) holds no test server. `just web` builds both; the
# palmtop tests skip without it.
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
cd "$here/../../crates/filer-test-server"
wasm-pack build --target web --release --no-typescript --out-dir "$here/pkg-filer" "$@"
