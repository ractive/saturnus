# saturnus task runner. Install just: https://github.com/casey/just
# Needs cargo-deny (`cargo install cargo-deny --locked`), wasm-pack and the
# wasm32-unknown-unknown target for `gates`, hyalo for `lint-kb`.

default:
    @just --list

# Pre-PR gates: format, dependency policy (deny.toml), clippy.
lint:
    cargo fmt --all -- --check
    cargo deny check
    cargo clippy --workspace --all-targets --locked -- -D warnings

# Unit and integration tests; the ROM-gated ones skip without SATURNUS_ROM_DIR.
test:
    cargo test --workspace --locked -q
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

# The knowledgebase lints clean (CI's lint-kb-full).
lint-kb:
    hyalo lint --strict

# The core crate packages and verifies for crates.io (CI's quality-gates;
# --allow-dirty only so it runs before the commit, CI checks a clean tree).
publish-check:
    cargo publish --dry-run -p saturnus --locked --allow-dirty

# Everything CI checks (.github/workflows/ci.yml); keep both in sync.
gates: lint test wasm lint-kb publish-check
