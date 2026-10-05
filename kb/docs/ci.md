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
| `clippy` | ubuntu | `cargo clippy --workspace --all-targets --locked -- -D warnings` |
| `test` | ubuntu, macOS, Windows | `cargo test --workspace --locked -q`, then `saturnus --help` |
| `wasm` | ubuntu | `cargo check -p saturnus --target wasm32-unknown-unknown`, `web/build.sh` (wasm-pack), `cargo test -p saturnus-web` |
| `lint-kb` | pull requests | `hyalo lint --strict` on the kb files the PR changes |
| `lint-kb-full` | pushes to `main` | `hyalo lint --strict` on the whole kb |
| `quality-gates` | always | `cargo deny check` (`deny.toml`), `cargo publish --dry-run -p saturnus --locked` |

Locally, `just gates` runs the same checks in one go (`just lint` is the
fast subset: fmt, cargo-deny, clippy). Keep the justfile and the workflow
in sync.

Line endings: `.gitattributes` keeps every text file LF in all checkouts
(Windows included), because the golden screens are compared byte for byte
and the shell scripts must run as checked out.

Not in CI: ROMs (the e2e tests skip without `SATURNUS_ROM_DIR`; run
`just e2e <dir>` locally) and the saturnng differential script (Docker and
ROMs).

## Supply chain

`deny.toml` is the policy (decision log, iteration 16): crates.io only,
git sources only by `allow-git` (just `https://github.com/ractive/hptx`),
advisories block the merge with no ignores, a licence allow-list of what
the tree needs plus one crate-scoped exception (serialport, MPL-2.0).
`Cargo.lock` is committed and every CI cargo command uses `--locked`.
Dependabot (`.github/dependabot.yml`) proposes weekly action and crate
updates; hptx-core moves by hand.

Pinning of actions: see [[docs/releasing]], "Pinning policy".
