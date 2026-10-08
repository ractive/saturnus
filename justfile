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
    cargo clippy --workspace --exclude saturnus --exclude saturnus-tauri --all-targets --locked -- -D warnings
    cargo clippy -p saturnus --lib --locked -- -D warnings
    scripts/about-json.py --check
    scripts/flags-json.py --check
    scripts/commands-json.py --check

# Unit and integration tests; the ROM-gated ones skip without SATURNUS_ROM_DIR.
test:
    cargo test --workspace --exclude saturnus-tauri --locked -q
    cargo run -q --locked -p saturnus-cli --bin saturnus -- --help > /dev/null

# The page's pure functions (object text, previews, flag rows); needs Node 20+.
web-test:
    node --test web/test/*.test.mjs

# The ROM-gated end-to-end tests: `just e2e /path/to/roms`.
e2e rom_dir:
    SATURNUS_ROM_DIR="{{rom_dir}}" cargo test --workspace --locked -q --test e2e

# The release checklist's ROM-gated run (kb/docs/releasing.md): with the
# Tauri runner's and the full reference regeneration (minutes) included.
# Every ROM-gated test: `just rom-tests /path/to/roms`.
rom-tests rom_dir:
    SATURNUS_ROM_DIR="{{rom_dir}}" cargo test --workspace --exclude saturnus-tauri --locked -q
    SATURNUS_ROM_DIR="{{rom_dir}}" cargo test -p saturnus-tauri --locked -q
    SATURNUS_ROM_DIR="{{rom_dir}}" cargo test --release --locked -p saturnus-refgen -- --ignored

# The core and the host crate build for wasm32, the web package builds, the
# bindings' and the host crate's tests pass.
wasm: web
    cargo check -p saturnus -p saturnus-host --target wasm32-unknown-unknown --locked
    cargo test -p saturnus-web -p saturnus-host --locked -q

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

# The flags panel's system flag tables from the hardware wiki (web/flags.json).
flags wiki="~/devel/hp-literature":
    scripts/flags-json.py {{wiki}}

# The command palette's reference data from data/commands/ (web/commands.json).
commands:
    scripts/commands-json.py

# The knowledgebase lints clean (CI's lint-kb-full).
lint-kb:
    hyalo lint --strict

# The publish rehearsal (CI's quality-gates): the crates.io crates, in the
# order of release.yml's publish-crates, packaged and each verified by a
# build against the packages before it (a temporary local registry, no
# crates.io; kb/docs/releasing.md). --allow-dirty only so it runs before
# the commit; CI checks a clean tree. Cargo treats the temporary
# registry's packages like crates.io's, as immutable per version: it reuses
# their unpacked sources ($CARGO_HOME/registry/src/-<hash>/) and their
# build artifacts, so a second run would verify against the first run's
# code. Both are removed first, the artifacts in the rehearsal's own target
# directory (CI starts empty).
package:
    rm -rf "${CARGO_HOME:-$HOME/.cargo}"/registry/src/-*/saturnus-*
    cargo clean -q --target-dir target/rehearsal -p saturnus -p saturnus-objects -p saturnus-host -p saturnus-drive
    for c in saturnus saturnus-objects saturnus-host saturnus-drive saturnus-cli; do cmp -s LICENSE crates/$c/LICENSE || { echo "crates/$c/LICENSE differs from LICENSE"; exit 1; }; done
    cargo package --locked --allow-dirty --target-dir target/rehearsal -p saturnus -p saturnus-objects -p saturnus-host -p saturnus-drive -p saturnus-cli

# The published crates' docs as docs.rs builds them, warnings denied. The
# CLI separately: its binary is called `saturnus`, like the core library.
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked -p saturnus -p saturnus-objects -p saturnus-host -p saturnus-drive
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked -p saturnus-cli --bin saturnus

# Everything CI checks (.github/workflows/ci.yml); keep both in sync.
gates: lint test web-test tauri wasm lint-kb package doc
