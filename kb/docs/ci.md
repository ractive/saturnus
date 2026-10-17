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
still going for the previous one. Each push to `main` runs in a
concurrency group of its own (`ci-<sha>`): its `changes` job checks only
that push's diff, so a newer push must not cancel it or replace it while
it is pending, or a change could go unchecked.
The satx end-to-end job is a workflow of its own, `e2e.yml` (below,
"What runs when").

## What runs when

The first job, `changes`, diffs the pull request against its base
(`base...head`) or the push against the previous `main` commit
(`before..after`) with `git diff --name-only`, a plain script rather than
a third-party path-filter action (one less action to pin and update;
[[docs/releasing]], "Pinning policy"). It sets three outputs:

| Output | Paths |
| --- | --- |
| `rust` | `crates/**` (the command reference in `crates/saturnus-cli/data/` included), `Cargo.toml`, `Cargo.lock`, `deny.toml`, `rustfmt.toml`, `LICENSE`, `web/test/protocol-script.json` and `.expected.json` (saturnus-drive's `tests/protocol_script.rs` runs them natively in `test`) |
| `web` | `web/**`, `scripts/*.py` (the generators of the page's data) |
| `kb` | `kb/**`, `.hyalo.toml` |

A change to `ci.yml` itself, a new branch or a range git cannot resolve
sets all three. Then:

- `fmt`, `clippy`, `test`, `test-musl`, `msrv`: when `rust`.
- `wasm` (the web app: `crates/saturnus-tx*` fall under `rust`,
  `web/` under `web`), `tauri` (its build script reads `web/`), `quality-gates` (it also
  checks the page's generated JSON): when `rust` or `web`.
- `lint-kb` (PRs) and `lint-kb-full` (pushes): when `kb`.
- `commit-email`: on every PR.

A job that does not apply shows as skipped, which the merge script
accepts; every PR has at least `changes` and `commit-email`. `main` has
no branch protection and there is no aggregate job. Files outside the
three sets (README, CHANGELOG, `packaging/`, other workflows) run no job.

| Case | Runs |
| --- | --- |
| kb-only PR | `changes`, `commit-email`, `lint-kb` |
| web-only PR | `changes`, `commit-email`, `wasm`, `tauri`, `tauri-windows`, `quality-gates` |
| Rust PR | `changes`, `commit-email`, every Rust job, `wasm`, `tauri`, `tauri-windows`, `quality-gates` |
| push to `main`, only `web/` | `changes`, `wasm`, `tauri`, `tauri-windows`, `quality-gates`; `e2e.yml` does not start |

`e2e.yml` (`satx-e2e`, 30-minute limit per job) runs

- on a push to `main` whose changes reach what it builds (GitHub's own
  `paths` filter against the previous commit): `crates/satx-core`,
  `crates/satx-cli`, `crates/kermit-proto`, `crates/xmodem-proto`,
  `crates/saturnus-core`, `crates/saturnus-objects`,
  `crates/saturnus-host`, `crates/saturnus-drive` (the `saturnus`
  feature of satx-core and satx), `tools/saturnng-oracle/`,
  `scripts/satx-e2e-cli.sh`, `Cargo.toml`, `Cargo.lock`, `e2e.yml`.
  Files (`crates/saturnus-tx*`, `web/tx/`) is not in the list on
  purpose: nothing in the job builds it, and its e2e against the emulated
  calculators is local (`just tx-e2e`);
- on a pull request with the label `e2e`: adding the label starts a run,
  and every later push to the PR runs it again while the label is on
  (`gh pr edit N --add-label e2e`); without it the job shows as skipped.
  Adding another label to a PR with `e2e` neither starts nor cancels an
  e2e run (that event's run skips the job, in a concurrency group of its
  own);
- by hand: `gh workflow run e2e.yml --ref <branch>`.

It is a separate workflow because a `labeled` trigger in `ci.yml` would
rerun all of CI for every label (dependabot adds two to each PR).

The Rust cache (`Swatinem/rust-cache`) is saved only on `main`
(`save-if`); pull requests restore `main`'s caches. When PRs saved their
own (about 4 GB per PR ref) the repository went over the 10 GB Actions
cache limit and evicted `main`'s. `pages.yml` and `desktop.yml` run by
hand only and keep saving.

## Jobs

| Job | Runs | What |
| --- | --- | --- |
| `fmt` | ubuntu | `cargo fmt --all -- --check` |
| `clippy` | ubuntu | `cargo clippy --workspace --exclude saturnus-tauri --all-targets --locked -- -D warnings`, then the same without the core's test targets (`--exclude saturnus-core` with `--all-targets`, and `-p saturnus-core --lib`), where the core's `internals` feature is off, so no host and no host test can lean on it |
| `test` | ubuntu, macOS, Windows | `cargo test --workspace --exclude saturnus-tauri --locked -q`, then `saturnus --help` |
| `test-musl` | ubuntu | the same for `--target x86_64-unknown-linux-musl` (musl-tools, natively): the static Linux CLI archive, which `release.yml` builds without tests |
| `tauri` | ubuntu | installs webkit2gtk-4.1, libxdo and OpenSSL headers, then clippy of `saturnus-tauri` in the debug and the release profile (release builds leave out the debug-only self-test hooks, so both must compile) and its tests (the ROM-gated ones skip; the page's wasm is not built here, so its build script warns and its `frontend` test checks the page files only) |
| `tauri-windows` | Windows | the tests of `saturnus-tauri` (WebView2 comes with the runner): paths differ there, and the 0.3.0 release dry run caught a Windows-only test bug the Linux job missed. No clippy, and its Rust cache is never saved (one more multi-GB entry would crowd main's out of the 10 GB limit); it builds cold in about 3.5 minutes |
| `msrv` | ubuntu | `cargo check --locked` of the published crates with Rust 1.88.0, the workspace `rust-version` (let-chains need 1.88; checked with that toolchain in iteration 23): the five saturnus crates (saturnus-core, saturnus-objects, saturnus-host, saturnus-drive, saturnus), kermit-proto, xmodem-proto and satx-core. A second step installs Rust 1.89.0 and checks satx (crates/satx-cli), whose own `rust-version` is 1.89 (rustyline 18 uses the standard library's file locks, stable since 1.89) |
| `wasm` | ubuntu | `cargo check -p saturnus-core -p saturnus-host -p kermit-proto -p xmodem-proto --target wasm32-unknown-unknown`, `cargo check` and clippy of `satx-core` without default features for wasm32, `cargo check -p saturnus-tx` and `-p saturnus-web` for wasm32 and the bindings' native tests, `web/build.sh` (wasm-pack), then `node --test` of every `web/test/*.test.mjs` with the runner's Node and Chrome (the one document's Calculator and Files, the hub, Files' protocol stack replaying `crates/saturnus-tx/traces` and its page with the recorded fake link of `web/test/fixtures/tx-page.trace`, the site; the `*e2e*` files skip without the ROMs: `just web-test-full DIR`, `just tx-e2e DIR` are local). One job since iteration 50d, which made the two pages one app |
| `commit-email` | pull requests | every commit in `origin/<base>..HEAD` (full history, `fetch-depth: 0`) has the author email james@ractive.ch; only `dependabot[bot]` is exempt. The base ref comes in through the environment, not a `${{ }}` expansion in the script. The rule is in CLAUDE.md |
| `satx-e2e` (`e2e.yml`) | ubuntu-24.04, a matrix of 48SX and 49G in parallel, 30 minutes each | each job builds the saturnng oracle image from `tools/saturnng-oracle/` (it downloads saturnng and the ROMs from hpcalc.org at build time; the image is never cached or pushed), starts its calculator (48SX on port 4848, 49G on 4850) and waits with `tools/saturnng-oracle/wait-ready.sh` (the container bridges only once its Kermit server answered an I packet, otherwise the emulator is started again, four starts in all; when it gives up, or after 300 s, the step fails with the container's log, [[backlog/satx-ci-emulator-flakes]]), runs satx-core's e2e suite against it (`SATX_E2E_ADDR`), then `scripts/satx-e2e-cli.sh`. The time is the tests' (the CLI script alone takes 6 to 7.5 minutes per model), so the split halves the wall time from about 18 minutes. Between the two, a Kermit probe checks the calculator; one that does not answer gets an "Oracle wedged" warning annotation, and it and the 49G (always, as before) get a fresh container ([[backlog/satx-flaky-xmodem-e2e]]). On failure it prints the calculator's screen and log |
| `lint-kb` | pull requests, kb changed | `hyalo lint --strict` on the kb files the PR changes |
| `lint-kb-full` | pushes to `main`, kb changed | `hyalo lint --strict` on the whole kb |
| `quality-gates` | Rust or web changed | `cargo deny check` (`deny.toml`), the publish rehearsal (`cargo package --locked` of the nine crates.io crates, satx's four included, each verified against the packages before it; [[docs/releasing]]), the published crates' docs with `-D warnings`, `scripts/about-json.py --check` (the About panel's generated `web/about.json` names no local path or e-mail address), `scripts/flags-json.py --check` (the flags panel's generated `web/flags.json` likewise, and every system flag covered once) |

Locally, `just gates` runs the same checks in one go (`just lint` is the
fast subset: fmt, cargo-deny, clippy). Keep the justfile and the workflow
in sync.

## The Tauri apps in the workspace

`crates/saturnus-tauri` (one desktop app since iteration 51, which
folded the former `crates/saturnus-tx-tauri` into it) links the system webview: WebView2 on Windows and
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
  `--exclude saturnus-tauri`; the `tauri` job
  installs the libraries and runs clippy and the tests for those crates
  alone, on ubuntu (the Linux build is the one with system
  dependencies); `tauri-windows` runs their tests on Windows too (paths
  differ there). The macOS build is exercised by `desktop.yml` when
  installers are made.
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
| `desktop.yml` | `release.yml` succeeded on a published release (`workflow_run`, its tag); manual (`workflow_dispatch`) | for the desktop app (one since iteration 51; a matrix of systems, the page's wasm built first) Tauri's official action builds the installers on macOS (aarch64: `.app`, `.dmg`), Windows (`.msi`, NSIS `.exe`) and Linux (ubuntu-22.04: `.deb`, `.rpm`, `.AppImage`) and uploads them as workflow artifacts (`contents: read`); with a release tag (the release's, or the `release-tag` input) the build checks out that tag and a separate job with `contents: write` attaches the installers to that existing release, never overwriting assets (no `--clobber`). |
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
