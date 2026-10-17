---
type: iteration
title: "Iteration 51: one desktop app"
date: 2026-10-17
status: planned
tags:
  - iteration
  - tauri
branch: iter-51/one-desktop-app
---

# Iteration 51: one desktop app

The desktop app becomes the one document of
[[iterations/iteration-50d-one-document]] in one Tauri app, with the
emulator's native engine, serial ports and the computer's files.
`saturnus-tx-tauri` folds into `saturnus-tauri` and is deleted. No
migration (owner, 2026-10-17).

## Design

- **Engine stays native** (owner, 2026-10-17): the runner thread keeps
  the calculator running while the window is hidden (`no_app_nap`) and
  survives a page reload. The bridge (`TauriBackend`, `command`,
  `order.rs`) stays.
- **Files on the running calculator: the main work of this iteration.**
  HubDevice cannot use `TauriBackend` as it is:
  - `fetchFile` and `backup` must return bytes; natively `fetchFile`
    opens a save dialog and writes to disk (`lib.rs` `Need::FetchFile`,
    `saturnus-drive` `runner.rs`). Add a bytes-returning form (base64,
    as every bytes field over `invoke`, `backend.js`) for `fetchFile`
    and `backup`; `storeFile` with base64 `data` exists.
  - The native side has no per-consumer state: a `watchMemory(false)`
    from the closing explorer would end HubDevice's `memoryChanged`. A
    small in-page multiplexer in front of the one `TauriBackend` merges
    `watchMemory` and `visibility` across the calculator view and
    HubDevice, as `hub.js` does per connection.
  This supersedes [[backlog/desktop-shared-calculator]] (no loopback, no
  token).
- **WebAssembly in the desktop app**: Files' protocol, aplet server and
  private calculator run in wasm workers. The app ships `web/pkg`
  (`build.rs`/`site.sh --list`, `desktop.yml` builds with wasm), and its
  CSP becomes `saturnus-tx-tauri`'s (`'wasm-unsafe-eval'`,
  `worker-src`). The private calculator on the desktop stays wasm, not
  the native engine. `tests/frontend.rs` and `selftest.js` follow (the
  selftest drives the emulator's DOM: it needs the route).
- **Moved from `saturnus-tx-tauri`**: `serial.rs`, `files.rs` (with the
  roots of iteration 49), `places.rs`, the `serial_*`, `folder_*`,
  `save_file` commands; `emulator_rom*` become reads of the app's own
  ROM library. `folders.json` moves to `ch.ractive.saturnus`'s config
  folder.
- **One window**; the identifier stays `ch.ractive.saturnus`, so ROMs
  and saved states carry over.
- **Release**: `desktop.yml` builds one app and moves one cask
  (`saturnus-app`); `release.yml` loses the tx assets; the
  `saturnus-tx` cask is deleted from the tap.

## Tasks

- [ ] Native `fetchFile`/`backup` returning base64 bytes; tests in `saturnus-tauri`
- [ ] In-page multiplexer over `TauriBackend` (`watchMemory`, `visibility`); HubDevice on it; a unit test with a fake backend
- [ ] `web/pkg` in the app; CSP; `desktop.yml` wasm; `tests/frontend.rs`; `selftest.js` on the route
- [ ] Move serial, files, places and their commands; delete `crates/saturnus-tx-tauri`
- [ ] `just`, `ci.yml`, `desktop.yml`, `release.yml`: one app, one cask; the tap's `saturnus-tx` cask removed
- [ ] `just tauri` green; the app builds on macOS; `desktop.yml` dry run green
- [ ] Owner: on macOS, Files on the running 48GX lists and copies; a cable calculator connects; a minimised calculator keeps time

## Acceptance

- One desktop app; `crates/saturnus-tx-tauri` gone; Files works on the running calculator and on a cable.

## Out of scope

- Moving the engine to WebAssembly on the desktop (revisit only after measuring hidden-window behaviour).
