# saturnus task runner. Install just: https://github.com/casey/just
# Needs cargo-deny (`cargo install cargo-deny --locked`), wasm-pack and the
# wasm32-unknown-unknown target for `gates`, hyalo for `lint-kb`.

default:
    @just --list

# Pre-PR gates: format, dependency policy (deny.toml), clippy.
lint:
    cargo fmt --all -- --check
    cargo deny check
    cargo clippy --workspace --exclude saturnus-tauri --all-targets --locked -- -D warnings

# Unit and integration tests; the ROM-gated ones skip without SATURNUS_ROM_DIR.
test:
    cargo test --workspace --exclude saturnus-tauri --locked -q
    cargo run -q --locked -p saturnus-cli --bin saturnus -- --help > /dev/null

# The ROM-gated end-to-end tests: `just e2e /path/to/roms`.
e2e rom_dir:
    SATURNUS_ROM_DIR="{{rom_dir}}" cargo test --workspace --locked -q --test e2e

# The core builds for wasm32, the web package builds, the bindings' tests pass.
wasm: web
    cargo check -p saturnus --target wasm32-unknown-unknown --locked
    cargo test -p saturnus-web --locked -q

# WebAssembly package for the browser UI into web/pkg/ (needs wasm-pack).
web:
    web/build.sh -- --locked

# The Tauri app (CI's tauri job): clippy and tests. On Linux it needs
# webkit2gtk-4.1 and libxdo (kb/docs/ci.md).
tauri:
    cargo clippy -p saturnus-tauri --all-targets --locked -- -D warnings
    cargo test -p saturnus-tauri --locked -q

# The desktop app in development, with the page in web/ (needs
# `cargo install tauri-cli --version 2.12.1 --locked`).
app:
    cd crates/saturnus-tauri && cargo tauri dev

# The about panel's source list from the hardware wiki (web/about.json).
about wiki="~/devel/hp-literature":
    scripts/about-json.py {{wiki}}

# The knowledgebase lints clean (CI's lint-kb-full).
lint-kb:
    hyalo lint --strict

# The core crate packages and verifies for crates.io (CI's quality-gates;
# --allow-dirty only so it runs before the commit, CI checks a clean tree).
publish-check:
    cargo publish --dry-run -p saturnus --locked --allow-dirty

# Everything CI checks (.github/workflows/ci.yml); keep both in sync.
gates: lint test tauri wasm lint-kb publish-check
