---
title: CI
type: docs
date: 2026-10-05
status: active
tags:
  - infrastructure
  - testing
---

# CI

`.github/workflows/ci.yml` runs on pull requests to `main` and `iter-*/**`
and on pushes to `main`, with `permissions: contents: read`. The structure
follows `~/devel/hyalo`'s workflow. A new push to a PR cancels the run
still going for the previous one.

| Job | Runs | What |
| --- | --- | --- |
| `fmt` | ubuntu | `cargo fmt --all -- --check` |
| `clippy` | ubuntu | `cargo clippy --workspace --exclude saturnus-tauri --all-targets --locked -- -D warnings`, then the same without the core's test targets (`--exclude saturnus-core` with `--all-targets`, and `-p saturnus-core --lib`), where the core's `internals` feature is off, so no host and no host test can lean on it |
| `test` | ubuntu, macOS, Windows | `cargo test --workspace --exclude saturnus-tauri --locked -q`, then `saturnus --help` |
| `test-musl` | ubuntu | the same for `--target x86_64-unknown-linux-musl` (musl-tools, natively): the static Linux CLI archive, which `release.yml` builds without tests |
| `tauri` | ubuntu | installs webkit2gtk-4.1, libxdo and OpenSSL headers, then clippy of `saturnus-tauri` in the debug and the release profile (release builds leave out the debug-only self-test hooks, so both must compile) and its tests (the ROM-gated one skips) |
| `msrv` | ubuntu | `cargo check --locked` of the published crates with Rust 1.88.0, the workspace `rust-version` (let-chains need 1.88; checked with that toolchain in iteration 23): the five saturnus crates (saturnus-core, saturnus-objects, saturnus-host, saturnus-drive, saturnus), kermit-proto, xmodem-proto and satx-core. A second step installs Rust 1.89.0 and checks satx (crates/satx-cli), whose own `rust-version` is 1.89 (rustyline 18 uses the standard library's file locks, stable since 1.89) |
| `wasm` | ubuntu | `cargo check -p saturnus-core -p saturnus-host -p kermit-proto -p xmodem-proto --target wasm32-unknown-unknown`, `web/build.sh` (wasm-pack), `cargo test -p saturnus-web -p saturnus-host`, `node --test web/test/*.test.mjs` (the page's pure functions, with the runner's Node) |
| `commit-email` | pull requests | every commit in `origin/<base>..HEAD` (full history, `fetch-depth: 0`) has the author email james@ractive.ch; only `dependabot[bot]` is exempt. The base ref comes in through the environment, not a `${{ }}` expansion in the script. The rule is in CLAUDE.md |
| `satx-e2e` | ubuntu-24.04, 30 minutes | builds the saturnng oracle image from `tools/saturnng-oracle/` (it downloads saturnng and the ROMs from hpcalc.org at build time; the image is never cached or pushed), starts a 48SX on port 4848 and a 49G on port 4850, runs satx-core's e2e suite against both (`SATX_E2E_ADDR`), then `scripts/satx-e2e-cli.sh` on each. The 49G is restarted before its CLI script, because after the satx-core suite it is often wedged ([[backlog/satx-flaky-xmodem-e2e]]). On failure it prints both calculators' screens and logs |
| `lint-kb` | pull requests | `hyalo lint --strict` on the kb files the PR changes |
| `lint-kb-full` | pushes to `main` | `hyalo lint --strict` on the whole kb |
| `quality-gates` | always | `cargo deny check` (`deny.toml`), the publish rehearsal (`cargo package --locked` of the nine crates.io crates, satx's four included, each verified against the packages before it; [[docs/releasing]]), the published crates' docs with `-D warnings`, `scripts/about-json.py --check` (the About panel's generated `web/about.json` names no local path or e-mail address), `scripts/flags-json.py --check` (the flags panel's generated `web/flags.json` likewise, and every system flag covered once) |

Locally, `just gates` runs the same checks in one go (`just lint` is the
fast subset: fmt, cargo-deny, clippy). Keep the justfile and the workflow
in sync.

## The Tauri app in the workspace

`crates/saturnus-tauri` links the system webview: WebView2 on Windows and
WKWebView on macOS come with the OS, but on Linux the build needs the
webkit2gtk-4.1 development packages (and GTK 3, libsoup 3, libxdo). An
ubuntu runner does not have them, so `cargo clippy --workspace` would fail
in `clippy`, `test` and the release pipeline's Linux builds. Chosen
arrangement (iteration 11):

- The crate is a workspace **member** (one `Cargo.lock`; `cargo deny`,
  `cargo audit` and Dependabot see its dependency tree) but not a
  **default member**: plain `cargo build`, `cargo test` and the shared
  release workflow's `cargo build --release` leave it out.
- `--workspace` includes every member, so `clippy` and `test` pass
  `--exclude saturnus-tauri`; the `tauri` job installs the libraries and
  runs clippy and the tests for that crate alone. It runs on ubuntu only:
  the Linux build is the one with system dependencies, and the macOS and
  Windows builds are exercised by `desktop.yml` when installers are made.
- The crate has its own `rust-version` (1.88): the fixed `time` and
  `plist` releases its tree needs require it; the workspace's is 1.88 too
  (let-chains), checked by the `msrv` job.
- `just gates` runs the `tauri` recipe too (it builds on macOS and
  Windows without extra packages; on Linux install them first).

Rejected: a separate workspace for the app (a second lockfile that
`cargo deny` and Dependabot would not see unless configured twice), and
installing webkit2gtk in every job (slower, and the CLI's builds would
depend on it for nothing).

## Other workflows

| Workflow | Trigger | What |
| --- | --- | --- |
| `desktop.yml` | manual (`workflow_dispatch`) | Tauri's official action builds the installers on macOS (aarch64: `.app`, `.dmg`), Windows (`.msi`, NSIS `.exe`) and Linux (ubuntu-22.04: `.deb`, `.rpm`, `.AppImage`) and uploads them as workflow artifacts (`contents: read`); with a `release-tag` input the build checks out that tag and a separate job with `contents: write` attaches the installers to that existing release, never overwriting assets (no `--clobber`). |
| `pages.yml` | manual | Builds the wasm package, assembles the page with `web/site.sh` (which fails when a module the page imports is missing) and publishes it by FTPS to ractive.ch (no GitHub Pages since 2026-10-08) ([[docs/releasing]]). |
| `release.yml` | release published, manual dry run | The CLI's release pipeline, see [[docs/releasing]]. |
| `publish-crates.yml` | manual | The crates.io recovery path: publishes the crates of `release.yml`'s list from a given ref, skipping those already up ([[docs/releasing]]). |

Both new workflows are manual until the owner enables Pages and decides
on signing ([[docs/releasing]]); the release tag reaches the upload
script through the environment, never through a `${{ }}` expansion in
`run:`.

Line endings: `.gitattributes` keeps every text file LF in all checkouts
(Windows included), because the golden screens are compared byte for byte
and the shell scripts must run as checked out.

Not in CI: ROMs (the e2e tests skip without `SATURNUS_ROM_DIR`; run
`just e2e <dir>` locally) and the saturnng differential script (Docker and
ROMs).

## Supply chain

`deny.toml` is the policy (decision log, iteration 16): crates.io only,
no git sources (the last, the transfer tool's core, went with saturnus-mcp in iteration
18), advisories block the merge (since iteration 11 with six dated
"unmaintained" ignores in the Tauri tree, pending the owner's decision), a
licence allow-list of what the tree needs (MIT, Unicode-3.0) plus
crate-scoped exceptions (since iteration 11 the Tauri tree's MPL-2.0,
BSD-3-Clause, Zlib and LLVM-exception crates, since iteration 18 its
Apache-2.0-only crates, each named; see `deny.toml`).
`Cargo.lock` is committed and every CI cargo command uses `--locked`.
Dependabot (`.github/dependabot.yml`) proposes weekly action and crate
updates.

Pinning of actions: see [[docs/releasing]], "Pinning policy".
