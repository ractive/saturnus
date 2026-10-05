---
type: iteration
title: "Iteration 16: CI and supply chain"
date: 2026-10-05
status: planned
tags:
  - iteration
  - infrastructure
branch: iter-16/ci-and-supply-chain
---

# Iteration 16: CI and supply chain

Requested 2026-10-05 after the hptx dependency audit: saturnus has no CI
yet, and hptx-core depends on this repository through a git dependency, so
its dependency tree is part of hptx's supply chain. Model: `~/devel/hyalo`
(`deny.toml` and the `quality-gates` job in `.github/workflows/ci.yml`),
kept small so CI stays fast.

Renumbered from 12 to 16 on 2026-10-05 (12 is the memory explorer). Notes
from the saturnus side: `saturnus-mcp` depends on `hptx-core` through a git
dependency pinned by rev, so `[sources]` needs an allow-list entry for
`https://github.com/ractive/hptx` (and hptx in turn pins saturnus the same
way); the workspace has a `wasm32-unknown-unknown` check for the core crate
and `web/build.sh` (wasm-pack) that CI should run; ROM-gated tests are
skipped without `SATURNUS_ROM_DIR`. Iteration 11 adds the Tauri build
matrix and GitHub Pages on top of this workflow.

## Tasks

- [ ] `.github/workflows/ci.yml`: on push to `main` and on pull requests;
  job `check` on ubuntu-latest: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace -q` with `Swatinem/rust-cache`; ROM-dependent
  tests gated by an env var and skipped in CI (no ROMs in the repo).
- [ ] Every action pinned to a commit SHA with the version in a comment, as
  hyalo does (`actions/checkout`, `dtolnay/rust-toolchain`,
  `Swatinem/rust-cache`, `EmbarkStudios/cargo-deny-action`).
- [ ] `deny.toml` after hyalo's: `[advisories]` with no ignores,
  `[licenses]` allow-list (MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception,
  BSD-2/3-Clause, Zlib, Unicode-3.0; add what the tree actually needs after
  `cargo deny check licenses`), `[bans] multiple-versions = "warn"`,
  `[sources]` crates.io only, `unknown-git = "deny"`.
- [ ] `cargo-deny-action` step in the `check` job (`command: check`); it
  runs in seconds, the advisory database fetch is the only network cost.
  Measure the job's wall time before and after and record it here.
- [ ] `just lint` (or the equivalent recipe) runs `cargo deny check` locally
  so the gate is the same on a laptop and in CI; CLAUDE.md lists it among
  the pre-PR gates.
- [ ] `Cargo.lock` committed and `--locked` in CI builds, so the tree CI
  tests is the tree the lockfile describes.
- [ ] Decision-log entry: supply-chain policy (crates.io only, no git
  dependencies without an allow-list entry, advisories block the merge).

## Acceptance criteria

CI green on a PR; `cargo deny check` clean locally and in CI; the `check`
job stays under about two minutes on a warm cache.
