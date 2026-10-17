---
type: iteration
title: "Iteration 52: the one-app sweep"
date: 2026-10-17
status: planned
tags:
  - iteration
  - docs
branch: iter-52/one-app-sweep
---

# Iteration 52: the one-app sweep

What [[iterations/iteration-50-one-app-web]] and
[[iterations/iteration-51-one-desktop-app]] left behind.

## Tasks

- [ ] Dead code: anything only the two-app layout used (grep for `transfer/`, `emulator/`, `tx.html`, `saturnus-tx-tauri`, `TAB_NAMES`, `joinHub`, `canShare`)
- [ ] Docs: one app document from [[docs/saturnus-tx]] and the emulator docs; [[docs/architecture]], [[docs/releasing]], [[docs/ci]], `web/README.md`, `web/protocol.md` ("Shared calculators")
- [ ] [[backlog/desktop-shared-calculator]] closed as superseded; iteration 47 and 49 owner checks marked superseded
- [ ] `hyalo lint` clean

## Owner's own cleanup (outside the repo)

- Uninstall the installed web apps "saturnus" and "saturnus tx" on every device. Do **not** clear ractive.ch's site data: kept ROMs, states and settings are in it. If an old service worker lingers: DevTools → Application → Service workers → Unregister.
- `brew uninstall --cask saturnus-tx` (or delete "saturnus tx.app"); delete `~/Library/Application Support/ch.ractive.saturnus-tx`; the same on Windows and Linux if installed there.
- Bookmarks to `/saturnus/emulator/` and `/saturnus/transfer/`.
- After the first deploy: check the FTP for empty `emulator/` and `transfer/` folders.
