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
[[iterations/iteration-50-one-app-web]] in one Tauri app, with the
emulator's native engine, serial ports and the computer's files.
`saturnus-tx-tauri` folds into `saturnus-tauri` and is deleted. No
migration (owner, 2026-10-17).

## Design

- **Engine stays native** (owner, 2026-10-17): the runner thread keeps
  the calculator running while the window is hidden (`no_app_nap`) and
  survives a page reload. The bridge (`TauriBackend`, `command`,
  `order.rs`) stays.
- **Files on the running calculator**: the HubDevice talks to the
  native engine through the same `TauriBackend` the calculator view
  uses (memory commands of `web/protocol.md`). This supersedes
  [[backlog/desktop-shared-calculator]] (no loopback, no token).
- **Moved from `saturnus-tx-tauri`**: `serial.rs`, `files.rs`
  (with the roots of iteration 49), `places.rs`, the `serial_*`,
  `folder_*`, `save_file` commands; `emulator.rs`/`emulator_rom*`
  become direct reads of the app's own ROM library. `folders.json`
  moves to the app's own config folder (`ch.ractive.saturnus`).
- **One window**; the identifier stays `ch.ractive.saturnus`, so ROMs
  and saved states carry over.
- **Release**: `desktop.yml` builds one app and moves one cask
  (`saturnus-app`); `release.yml` loses the tx assets; the
  `saturnus-tx` cask is deleted from the tap (owner OK needed: another
  repository).

## Tasks

- [ ] Move serial, files, places and their commands into `saturnus-tauri`; delete `crates/saturnus-tx-tauri`
- [ ] HubDevice over `TauriBackend`; the engine's events reach every view
- [ ] `just`, `ci.yml`, `desktop.yml`, `release.yml`: one app, one cask
- [ ] `just tauri` green; the app builds on macOS
- [ ] Owner: the tap's `saturnus-tx` cask removed (PR to the tap)
- [ ] Owner: on macOS, Files on the running 48GX lists and copies; a cable calculator connects; a minimised calculator keeps time

## Acceptance

- One desktop app; `crates/saturnus-tx-tauri` gone; Files works on the running calculator and on a cable.
- `desktop.yml` dry run green.

## Out of scope

- Moving the engine to WebAssembly on the desktop (revisit only after measuring hidden-window behaviour).
