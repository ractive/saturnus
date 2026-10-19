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
    cargo clippy --workspace --exclude saturnus-core --exclude saturnus-tauri --all-targets --locked -- -D warnings
    cargo clippy -p saturnus-core --lib --locked -- -D warnings
    scripts/about-json.py --check
    scripts/flags-json.py --check
    scripts/commands-json.py --check
    scripts/check-comments.py

# Unit and integration tests; the ROM-gated ones skip without SATURNUS_ROM_DIR.
test:
    cargo test --workspace --exclude saturnus-tauri --locked -q
    cargo run -q --locked -p saturnus --bin saturnus -- --help > /dev/null

# The page's functions and the Worker with the real wasm package (the
# cross-host protocol script), and the transfer page's tests (the same
# package); needs Node 20+ and the `web` recipe's tools. The fast default
# (kb/docs/test-policy.md, "Web tests"): every file but the emulated
# calculators' end-to-end ones (`*e2e*`, in web-test-full), three files
# at a time. The ROM-gated files run when SATURNUS_ROM_DIR is set.
web-test: web
    node --test --test-concurrency=3 --test-timeout=300000 $(ls web/test/*.test.mjs | grep -v e2e)

# Every web test file, the `*e2e*` ones included (tx-e2e against the
# emulated calculators, minutes): once before a PR. The ROMs from DIR
# (read only); without them the ROM-gated tests skip. The long `*e2e*`
# files run from the start beside the rest (two at a time; node --test
# would sort them last), their report after the rest's.
web-test-full dir=env("SATURNUS_ROM_DIR", "roms"): web
    #!/usr/bin/env bash
    set -u
    export SATURNUS_ROM_DIR="{{dir}}"
    log=$(mktemp)
    SATURNUS_AUDIT=1 node --test --test-timeout=600000 $(ls web/test/*.test.mjs | grep e2e) > "$log" 2>&1 &
    e2e=$!
    node --test --test-concurrency=2 --test-timeout=300000 $(ls web/test/*.test.mjs | grep -v e2e)
    rest=$?
    wait "$e2e"
    long=$?
    cat "$log"
    rm -f "$log"
    exit $(( rest != 0 || long != 0 ))

# The page on phones in headless Chrome (web/test/overflow.test.mjs): no
# horizontal overflow on any view at any width, the palette as a sheet
# with the keyboard up. Part of web-test where Chrome is found; here the
# absence of Chrome is a failure (SATURNUS_CHROME=/path/to/chrome).
web-audit: web
    SATURNUS_AUDIT=1 node --test web/test/overflow.test.mjs

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

# The core, the host crate and the two protocol crates build for wasm32,
# so do satx-core without its native features, saturnus-tx and the
# bindings (saturnus-web); the web package builds (their native tests are in `test`).
wasm: web
    cargo check -p saturnus-core -p saturnus-host -p kermit-proto -p xmodem-proto --target wasm32-unknown-unknown --locked
    cargo check -p satx-core --no-default-features --target wasm32-unknown-unknown --locked
    cargo clippy --locked -p satx-core --no-default-features --all-targets -- -D warnings
    cargo check -p saturnus-tx -p saturnus-web --target wasm32-unknown-unknown --locked

# tab-mcp's browser library for "Connect your AI tool" (iteration 55),
# copied into web/vendor/tab-mcp-client/ (committed) from the published npm
# package, its tarball checked against the pinned integrity. A new version:
# change both values here, run it, commit the files and vendor.json.
tab_mcp_client_version := "0.1.0-alpha.0"
tab_mcp_client_integrity := "sha512-F3Su289PNYPWcgJkAcgevibtAodtF4SiSjAkRfDt3bn47a32fp5iOhb1G/rAPcb73uMEPdaKrhx1KGD5WmQapw=="

vendor-tab-mcp:
    scripts/vendor-tab-mcp-client.sh --from-npm "{{tab_mcp_client_version}}" --integrity "{{tab_mcp_client_integrity}}"

# The same from a local tab-mcp checkout's build, for development against
# changes not yet published (vendor.json then names the commit).
vendor-tab-mcp-local dir=env("TAB_MCP_DIR", env("HOME", "") + "/devel/tab-mcp"):
    scripts/vendor-tab-mcp-client.sh --from-local "{{dir}}"

# The WebAssembly package for both pages (crates/saturnus-web) into
# web/pkg/ (needs wasm-pack).
web:
    web/build.sh -- --locked

# The transfer page against the emulated 48SX, 48GX and 49G in headless
# Chrome (web/test/tx-e2e.test.mjs): connect, detect, list, get, put,
# replace, round trips byte for byte; the ROMs (sxrom-j, gxrom-r,
# rom.49g) from DIR, read only; the fast transfer (XModem) on the 48GX
# and 49G (web/test/tx-e2e-xmodem.test.mjs); and a folder served to the
# emulated 38G and 39G (web/test/tx-serve-e2e.test.mjs; 38G_A167.ROM,
# rom.39g): an aplet to the folder and back byte for byte, by wire to the
# 39G. Needs Chrome and the wasm package.
tx-e2e dir=env("SATURNUS_ROM_DIR", "roms"): web
    SATURNUS_ROM_DIR="{{dir}}" SATURNUS_AUDIT=1 node --test --test-timeout=600000 web/test/tx-e2e.test.mjs web/test/tx-e2e-xmodem.test.mjs web/test/tx-serve-e2e.test.mjs

# The fast transfer's traces (crates/saturnus-tx/traces/*-xmodem.trace,
# committed; crates/saturnus-web/tests/xmodem.rs replays them): the
# page against the emulated 48GX and 49G (gxrom-r, rom.49g from DIR);
# serial bytes only, the 49G's XSEND padding zeroed.
tx-xmodem-traces dir=env("SATURNUS_ROM_DIR", "roms"): web
    SATURNUS_ROM_DIR="{{dir}}" SATURNUS_TX_TRACES=crates/saturnus-tx/traces SATURNUS_AUDIT=1 node --test --test-timeout=600000 web/test/tx-e2e-xmodem.test.mjs

# The transfer page's recorded fake link (web/test/fixtures/tx-page.trace,
# committed): the page test's session recorded against the emulated 48GX
# (gxrom-r from DIR); serial bytes only.
tx-page-trace dir=env("SATURNUS_ROM_DIR", "roms"): web
    SATURNUS_ROM_DIR="{{dir}}" node web/test/tx-record.mjs web/test/fixtures/tx-page.trace

# The desktop app (CI's tauri job): clippy in the debug and the release
# profile, and tests. On Linux it needs webkit2gtk-4.1 and libxdo
# (kb/docs/ci.md).
tauri:
    cargo clippy -p saturnus-tauri --all-targets --locked -- -D warnings
    cargo clippy -p saturnus-tauri --release --all-targets --locked -- -D warnings
    cargo test -p saturnus-tauri --locked -q

# The desktop app in development, with the page in web/ and its wasm
# package (Files' workers; needs `cargo install tauri-cli --version 2.12.1
# --locked`).
app: web
    cd crates/saturnus-tauri && cargo tauri dev

# The about panel's source list from the hardware wiki (web/about.json).
about wiki="~/devel/calculator-knowledgebase":
    scripts/about-json.py {{wiki}}

# The flags panel's system flag tables from the hardware wiki (web/flags.json).
flags wiki="~/devel/calculator-knowledgebase":
    scripts/flags-json.py {{wiki}}

# The command palette's reference data from data/commands/ (web/commands.json).
commands:
    scripts/commands-json.py

# Registers the decision log's merge driver (.gitattributes) in this
# clone's config, which its worktrees share; once per clone. A checkout
# without the script (an older branch) falls back to a plain union merge.
merge-driver:
    git config merge.decision-log.name "union merge, blank line before each heading"
    git config merge.decision-log.driver 'sh -c '\''[ -x scripts/merge-decision-log.sh ] && exec scripts/merge-decision-log.sh "$@" || exec git merge-file --union "$2" "$1" "$3"'\'' _ %O %A %B'

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
    rm -rf "${CARGO_HOME:-$HOME/.cargo}"/registry/src/-*/saturnus-* "${CARGO_HOME:-$HOME/.cargo}"/registry/src/-*/satx-* "${CARGO_HOME:-$HOME/.cargo}"/registry/src/-*/kermit-proto-* "${CARGO_HOME:-$HOME/.cargo}"/registry/src/-*/xmodem-proto-*
    cargo clean -q --target-dir target/rehearsal -p saturnus-core -p saturnus-objects -p saturnus-host -p saturnus-drive -p kermit-proto -p xmodem-proto -p satx-core
    for c in saturnus-core saturnus-objects saturnus-host saturnus-drive saturnus-cli kermit-proto xmodem-proto satx-core satx-cli; do cmp -s LICENSE crates/$c/LICENSE || { echo "crates/$c/LICENSE differs from LICENSE"; exit 1; }; done
    cargo package --locked --allow-dirty --target-dir target/rehearsal -p saturnus-core -p kermit-proto -p saturnus-objects -p saturnus-host -p saturnus-drive -p saturnus -p xmodem-proto -p satx-core -p satx

# The published crates' docs as docs.rs builds them, warnings denied; the
# two CLIs have no library, so theirs are their binaries'.
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked -p saturnus-core -p saturnus-objects -p saturnus-host -p saturnus-drive -p kermit-proto -p xmodem-proto -p satx-core -p saturnus -p satx

# Everything CI checks (.github/workflows/ci.yml); keep both in sync.
gates: lint test web-test tauri wasm lint-kb package doc

# satx, the transfer tool (crates/satx-core, crates/satx-cli). Its oracle is
# saturnng in Docker (tools/saturnng-oracle/) with the calculator's serial
# port on TCP; ports 4848-4852 belong to these containers (CLAUDE.md,
# "satx"). The image downloads the ROMs from hpcalc.org at build time and
# is never pushed.
satx_e2e_addr := env("SATX_E2E_ADDR", "tcp://localhost:4848")

# Build and start the oracle; model 49g, 48gx or 48sx. 49G: `just satx-oracle-up 49g 4852 calc49`
satx-oracle-up model="48sx" port="4848" name="calc":
    @# Fail fast if anything already listens on the port (another emulator would win on 127.0.0.1).
    @if command -v lsof >/dev/null && lsof -nP -iTCP:{{port}} -sTCP:LISTEN >/dev/null; then \
        echo "port {{port}} is already in use; pick another or stop it:" >&2; \
        lsof -nP -iTCP:{{port}} -sTCP:LISTEN >&2; exit 1; fi
    docker build -t hp49g-emu tools/saturnng-oracle
    @# No --rm: the log of a container that gave up stays readable (satx-oracle-down removes it).
    docker rm -f {{name}} >/dev/null 2>&1 || true
    docker run -d -p {{port}}:4848 -e MODEL={{model}} --name {{name}} hp49g-emu
    tools/saturnng-oracle/wait-ready.sh {{name}}

# Stop and remove the oracle containers.
satx-oracle-down:
    -docker rm -f calc calc49

# satx-core's end-to-end suite against a running oracle (`just satx-oracle-up` first).
satx-e2e:
    SATX_E2E_ADDR={{satx_e2e_addr}} cargo test --locked -p satx-core --test e2e -- --nocapture

# The XModem steps type on the calculator via `docker exec CONTAINER calc-keys`: CONTAINER is the
# argument, else SATX_E2E_CONTAINER, else calc for port 4848 and calc49 for 4852 (see satx-oracle-up).

# The CLI end-to-end script against a running oracle, e.g. `just satx-e2e-cli tcp://localhost:4852` (the 49G)
satx-e2e-cli addr=satx_e2e_addr container="":
    #!/usr/bin/env bash
    set -euo pipefail
    c="{{container}}"
    c=${c:-${SATX_E2E_CONTAINER:-}}
    if [[ -z $c ]]; then
        case "{{addr}}" in
            *:4848) c=calc ;;
            *:4852) c=calc49 ;;
        esac
    fi
    SATX_E2E_ADDR="{{addr}}" SATX_E2E_CONTAINER="$c" scripts/satx-e2e-cli.sh

# satx-core's suite against saturnus in-process, e.g. `just satx-e2e-saturnus 49g`;
# model 48sx (sxrom-j), 48gx (gxrom-r) or 49g (rom.49g), the ROMs from
# SATURNUS_ROM_DIR, else roms/.
satx-e2e-saturnus model="48sx" roms=env("SATURNUS_ROM_DIR", "roms"):
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{model}}" in
        48sx) rom=sxrom-j ;;
        48gx) rom=gxrom-r ;;
        49g) rom=rom.49g ;;
        *) echo "satx-e2e-saturnus: model is 48sx, 48gx or 49g, not {{model}}" >&2; exit 2 ;;
    esac
    dir=$(cd "{{roms}}" && pwd)
    SATX_E2E_ADDR="saturnus://{{model}}@$dir/$rom" cargo test --locked -p satx-core --features saturnus --test e2e -- --nocapture

# Record a Kermit trace from the running oracle to stdout,
# e.g. `just satx-record-trace dir > crates/kermit-proto/traces/48sx-dir.trace`
satx-record-trace +args:
    @cargo run -q -p kermit-proto --example record -- localhost:4848 {{args}}

# Record an XModem trace to stdout (start XRECV/XSEND on the calculator first),
# e.g. `just satx-record-xmodem-trace localhost:4852 send all.hp > crates/xmodem-proto/traces/49g-xrecv.trace`
satx-record-xmodem-trace addr +args:
    @cargo run -q -p xmodem-proto --example record-xmodem -- {{addr}} {{args}}

# saturnus-tx's traces (crates/saturnus-tx/traces/, committed): its test
# session and ops session (directories, rename, purge, backup, restore onto
# a fresh calculator, PICT) recorded from saturnus in-process on the 48SX
# (sxrom-j), 48GX (gxrom-r) and 49G (rom.49g), the ROMs read from DIR;
# serial bytes only. Then an emulated 48GX to 49G copy, live.
tx-traces dir=env("SATURNUS_ROM_DIR", "roms"):
    cargo run --locked -q -p saturnus-tx --example record-traces -- "{{dir}}" crates/saturnus-tx/traces

# The aplet server's traces (committed): the emulated 38G (38G_A167.ROM)
# and 39G (rom.39g) from DIR as its clients, list L1 sent to the folder
# and back, and by XModem to the 39G; into crates/saturnus-tx/traces/ and
# crates/kermit-proto/traces/. Serial bytes only, emulated time.
tx-aplet-traces dir=env("SATURNUS_ROM_DIR", "roms"):
    cargo run --locked -q -p saturnus-tx --example record-aplet-traces -- "{{dir}}" .
