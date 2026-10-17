---
type: iteration
title: "Iteration 50a: one WebAssembly package"
date: 2026-10-17
status: completed
tags:
  - iteration
  - web
branch: iter-50a/one-wasm
---

# Iteration 50a: one WebAssembly package

First step of the one app ([[decision-log]], "2026-10-17 (one app)").
Mechanical: `crates/saturnus-tx-web` merges into `crates/saturnus-web`.
The site keeps its two pages; the existing tests cover the change.

## Design

- No export clashes (`Host`, `TxDevice` and the free functions differ;
  no `wasm_bindgen(start)` in either). One `web/build.sh`, one
  `web/pkg/`; `web/tx-build.sh` and `web/tx/pkg/` go; every import of
  `tx/pkg/saturnus_tx_web.js` points at `pkg/saturnus_web.js`.
- **Cost, measured and written here**: today 887 KB + 759 KB. The
  merged package is compiled in up to five places (the hub worker,
  Files' main thread, the protocol, aplet and private-calculator
  workers). Measure its size once; if it is over 1.4 MB, note it for
  the owner (keeping two packages stays possible).
  **Measured** (`web/build.sh`, release, wasm-opt): the merged
  `saturnus_web_bg.wasm` is 1,332,798 bytes (1.33 MB; 507 KB gzipped),
  under the 1.4 MB line; the JavaScript glue 41 KB.

## Tasks

- [x] Merge the crate; delete `crates/saturnus-tx-web`, `web/tx-build.sh`; imports, `site.sh`, `justfile`, `ci.yml`, `pages.yml`, `desktop.yml`, `saturnus-tx-tauri`'s `beforeBuildCommand` follow
- [x] Size measured and noted here
- [x] `just gates` and the tests that load the package green; deploy

## Acceptance

- One package; both pages work as before.
