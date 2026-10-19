---
type: iteration
title: "Iteration 52: the one-app sweep"
date: 2026-10-17
status: completed
tags:
  - iteration
  - docs
branch: iter-52/one-app-sweep
---

# Iteration 52: the one-app sweep

What [[iterations/iteration-50a-one-wasm]] to [[iterations/iteration-50d-one-document]] and
[[iterations/iteration-51-one-desktop-app]] left behind.

## Tasks

- [x] Dead code: anything only the two-app layout used (grep for `transfer/`, `emulator/`, `tx.html`, `saturnus-tx-tauri`, `TAB_NAMES`, `joinHub`, `canShare`)
- [x] Docs: one app document from [[docs/saturnus-tx]] and the emulator docs; [[docs/architecture]], [[docs/releasing]], [[docs/ci]], `web/README.md`, `web/protocol.md` ("Shared calculators")
- [x] [[backlog/desktop-shared-calculator]] closed as superseded; iteration 47 and 49 owner checks marked superseded
- [x] `hyalo lint` clean

## Local cleanup (done by Claude, owner 2026-10-17)

Checked on the owner's Mac on 2026-10-17: no saturnus desktop app or cask
installed, no Chrome-installed web app, no `ch.ractive.saturnus-tx` data;
only the webview data of `ch.ractive.saturnus` (dev runs), which the merged
app keeps using. So:

- [x] After iteration 51's release: remove the `saturnus-tx` cask from the tap and check `brew` no longer offers it
- [x] Moot: `emulator/` and `transfer/` on the FTP are not empty on purpose, they hold the kill-switch `sw.js` (removed on 2027-01-15, [[backlog/remove-kill-switch]])
- [ ] Left to the owner (other devices, not reachable from here): uninstall the installed web apps "saturnus" and "saturnus tx" on the phone and iPad, without clearing ractive.ch's site data (kept ROMs, states, settings live there). On iOS and iPadOS an installed home-screen app has storage of its own: ROMs kept inside it go with it; keep their files before uninstalling
