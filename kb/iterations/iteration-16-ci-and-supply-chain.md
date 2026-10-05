---
type: iteration
title: "Iteration 16: Full CI pipeline with supply-chain checks"
date: 2026-10-05
status: completed
tags:
  - iteration
  - infrastructure
branch: iter-16/ci-and-supply-chain
---

# Iteration 16: Full CI pipeline with supply-chain checks

Owner (2026-10-05): "We first need to set up the full CI pipeline. This
should then also contain the supply chain issues (deny.toml etc.). Check
out hyalo for a blueprint how to implement the CI with the reusable
workflows." This iteration goes first: every later PR runs through it.

The supply-chain part was drafted by the hptx session after its dependency
audit (hptx-core depends on saturnus through a git dependency, so this
tree is part of hptx's supply chain); renumbered from 12.

## Blueprint: `~/devel/hyalo`

- `.github/workflows/ci.yml`: separate jobs `fmt`, `clippy`, `test` (matrix
  ubuntu/macos/windows), `lint-kb` (pull requests: diff-aware `hyalo lint
  --strict --files-from - --format github` through `ractive/setup-hyalo@v1`)
  and `lint-kb-full` (push to main), `quality-gates` (cargo-deny through
  `EmbarkStudios/cargo-deny-action`, then project gates); triggers: pull
  requests to `main` and `iter-*/**`, pushes to `main`; `permissions:
  contents: read`; every third-party action pinned to a commit SHA with
  the version in a comment.
- `.github/workflows/release.yml`: a thin caller of the shared reusable
  workflow `ractive/release-workflows/.github/workflows/release.yml@<tag>`
  (first-party, pinned by tag; latest tag at the time of writing to be
  looked up) with `bin-name`, `version-package`, `publish-crates`, a
  `targets` matrix and `dry-run` on `workflow_dispatch`.
- `.github/dependabot.yml` (github-actions and cargo, weekly, minor/patch
  grouped, first-party reusable workflows ignored), `.github/release.yml`
  (changelog categories), `deny.toml`, `docs/ci.md`, `docs/releasing.md`
  ("Pinning policy"), `justfile` (`gates` mirrors CI).

## Saturnus specifics

- Workspace: `saturnus` (core, must build for `wasm32-unknown-unknown`),
  `saturnus-objects` may arrive with iteration 12a, `saturnus-drive`,
  `saturnus-cli` (binary `saturnus`), `saturnus-mcp` (binary
  `saturnus-mcp`; depends on `hptx-core` by git rev, so `deny.toml` needs
  an `allow-git` entry for `https://github.com/ractive/hptx` and crates.io
  publishing of that crate is not possible while the git dependency
  stays), `saturnus-web` (wasm-bindgen; `web/build.sh` runs wasm-pack).
- ROM-gated tests skip themselves without `SATURNUS_ROM_DIR`; CI never has
  ROMs. The saturnng differential script needs Docker and ROMs: not in CI.
- The kb is hyalo-driven (`.hyalo.toml`, `dir = "kb"`): the `lint-kb` jobs
  apply as in hyalo.
- Later iterations add to this pipeline rather than create their own:
  iteration 11 adds the Tauri build matrix and GitHub Pages for `web/`.

## Tasks

- [x] `.github/workflows/ci.yml` after hyalo's structure: `fmt`; `clippy`
  (`--workspace --all-targets --locked -- -D warnings`); `test` on
  ubuntu-latest, macos-latest and windows-latest (`cargo test --workspace
  --locked`; fix what does not pass on Windows or Linux, e.g. path or
  line-ending assumptions, or state exactly what is excluded and why);
  `wasm` (`cargo check -p saturnus --target wasm32-unknown-unknown
  --locked`, then `web/build.sh` with wasm-pack and the native tests of
  `saturnus-web`); `lint-kb` and `lint-kb-full` through
  `ractive/setup-hyalo@v1`; `quality-gates` with cargo-deny. Triggers and
  permissions as in hyalo; `Swatinem/rust-cache`; actions pinned to SHAs
  with version comments.
- [x] `deny.toml` after hyalo's: `[advisories]` with no ignores;
  `[licenses]` allow-list limited to what `cargo deny check licenses`
  needs, each entry justified in a comment; `[bans] multiple-versions =
  "warn"`; `[sources]` crates.io only plus `allow-git` for the hptx
  repository, `unknown-git = "deny"`. `cargo deny check` clean locally;
  an advisory that cannot be fixed by upgrading is reported to the owner,
  not ignored silently.
- [x] `.github/dependabot.yml` (github-actions, cargo; first-party
  `ractive/release-workflows*` and `ractive/setup-hyalo*` ignored) and
  `.github/release.yml` (changelog categories).
- [x] `.github/workflows/release.yml` calling the shared
  `ractive/release-workflows` release workflow for the `saturnus` CLI
  (and `saturnus-mcp` if the shared workflow supports a second binary;
  otherwise note it): read the reusable workflow's inputs in the
  `ractive/release-workflows` repository first; `dry-run` on
  `workflow_dispatch`; no crates.io publishing, winget, AUR or Cloudsmith
  yet (leave those inputs off and say so); a `targets` matrix like
  hyalo's with tests on the native targets.
- [x] `justfile`: `lint`, `test`, `gates` (the same sequence as CI
  including `cargo deny check`), `e2e` (ROM-gated), `web`; `CLAUDE.md`'s
  pre-PR gate list gains `cargo deny check` (one line; the file is the
  owner's).
- [x] `docs/ci.md` and `docs/releasing.md` in the kb (`kb/docs/`), short,
  with the pinning policy; decision-log entry (supply-chain policy:
  crates.io only, git dependencies only by allow-list, advisories block
  the merge; CI structure).
- [x] After the first run on the PR: record each job's wall time here; fix
  what fails on the runners; propose (do not apply) the branch-protection
  settings for `main` (required checks) for the owner.

## Acceptance criteria

- [x] CI is green on this iteration's PR on all three operating systems,
  including cargo-deny, the wasm job and the kb lint; `cargo deny check`
  and `just gates` are clean locally.
- [x] `release.yml` passes a `workflow_dispatch` dry run, or its first run
  is documented as pending with the reason.

## Outcome

Local, 2026-10-05 (macOS arm64): `just gates` (fmt, `cargo deny check`,
clippy, `cargo test --workspace --locked`, `web/build.sh`, the wasm32
check, the web bindings' tests, `hyalo lint --strict`) passes in 9.4 s
wall time with a warm `target/`. `cargo deny check`: advisories, bans,
licences and sources ok; six duplicate-version warnings (base64,
bitflags, miniz_oxide, nix, syn, windows-sys; mostly via serialport),
which warn by policy. `cargo audit` clean. `hyalo lint --strict`: no
issues. `actionlint`: clean. `cargo check --workspace --all-targets
--target x86_64-pc-windows-msvc` compiles; Windows test results only come
with the first CI run.

- `.gitattributes` (`* text=auto eol=lf`) keeps the golden screens and
  scripts LF on Windows checkouts; the PNG test in `saturnus-drive`
  closes its file before removing its scratch directory.
- Pending the first CI run: the Windows and macOS test jobs, the wasm job
  on the runner (wasm-pack 0.15.0 through `taiki-e/install-action`), the
  cargo-deny action, `lint-kb`, job wall times. `release.yml` can only be
  dispatched (dry run) once it is on `main`.
- The `roms` symlink to `/Users/james/devel/saturnus/roms`, committed by
  mistake in iteration 9 (`.gitignore`'s `roms/` does not match a link),
  is removed from the index; the ignore entry is now `roms`.
- Owner decisions: the crate-scoped MPL-2.0 exception for `serialport`
  (alternative: make serialport optional in hptx-core); the
  `HOMEBREW_TAP_TOKEN` and `SCOOP_BUCKET_TOKEN` secrets before the first
  real release; shipping `saturnus-mcp` needs multi-binary support in
  `ractive/release-workflows` (follow-up there).
- Added on the owner's request: the core crate `saturnus` has publishing
  metadata and its own README, packages without `tests/`, and
  `cargo publish --dry-run -p saturnus --locked` runs in `quality-gates`
  and `just gates` (passes locally).

First CI run on PR 12, all jobs green; wall times: `fmt` 14 s, `clippy`
39 s, `test` ubuntu 49 s, macOS 1 min 47 s, Windows 2 min 8 s, `wasm`
46 s, `lint-kb` 9 s, `quality-gates` 30 s. Nothing needed fixing on the
runners. The `cargo publish --dry-run` step in `quality-gates` was added
after that run and first runs with the next push. `release.yml`'s
`workflow_dispatch` dry run is pending: a dispatch needs the workflow on
`main`, so it runs after the merge.

Proposed branch protection for `main` (for the owner, not applied):
require a pull request and an up-to-date branch, and the checks `fmt`,
`clippy`, `test (ubuntu-latest)`, `test (macos-latest)`,
`test (windows-latest)`, `wasm`, `lint-kb` and `quality-gates`
(`lint-kb-full` runs only on pushes to `main`, so it cannot be required).
