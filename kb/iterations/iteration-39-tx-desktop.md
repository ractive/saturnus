---
type: iteration
title: "Iteration 39: saturnus-tx desktop app with native serial, release, signing and cask"
date: 2026-10-10
status: planned
tags:
  - iteration
  - saturnus-tx
  - tauri
  - release
branch: iter-39/tx-desktop
---

# Iteration 39: the desktop app

saturnus-tx as a Tauri app for macOS, Windows and Linux, built from the
same page, the way saturnus's desktop app is built and shipped
([[docs/releasing]], "Desktop app"). The app adds what browsers cannot
do: native serial in every webview and on Linux without Chrome, and a
folder without the File System Access API. The protocol stays in the
page's wasm ([[docs/saturnus-tx]], "Crates and folders"); native code
only moves bytes and files.

## What it does

- `crates/saturnus-tx-tauri`, a workspace member but not a default
  member (as `saturnus-tauri`, [[docs/ci]]): the tx page embedded with
  the `tx-site.sh --list` rule, Tauri commands `serial_ports`,
  `serial_open`, `serial_write`, `serial_close` and an event with the
  received bytes (`serialport`, a reader thread per open port), and the
  folder commands scoped to the folder the user picked in Tauri's dialog.
- The page picks `TauriSerialLink` and `TauriFolder` when
  `window.__TAURI__` exists, as saturnus's `backend.js` picks its
  backend.
- The port list names the adapters (USB vendor and product where the OS
  gives them); the last port is remembered.
- The emulated calculator works as in the browser (the saturnus-web
  wasm in the page), with a ROM file chosen from disk.

## Tasks

- [ ] `crates/saturnus-tx-tauri`: the app, the serial and folder commands, `tauri.conf.json` (identifier decided by the owner before the first build, [[docs/saturnus-tx]] open question 6), icons, the embedded page and a `frontend` test comparing it with `web/tx-site.sh --list`
- [ ] Web: `TauriSerialLink`, `TauriFolder`, the port list with the adapters' names
- [ ] Tests: the serial link over a pseudo-terminal pair to saturnus-drive's runner with its serial bridge (ROM-gated, Linux and macOS); the `tauri` CI job extended to the new crate (clippy in both profiles, tests)
- [ ] `desktop.yml` builds both apps (an `app` input or a matrix), unsigned dry run first; signing and notarisation as saturnus's (the same secrets); the Homebrew cask `saturnus-tx` written on release as saturnus's
- [ ] Versions and tags as the owner decides ([[docs/saturnus-tx]] open question 3); if tx has its own tags, `release.yml` must not run for them
- [ ] Linux: the `.deb`, `.rpm` and AppImage name the `dialout` group in their description; no file shared with the saturnus packages
- [ ] Docs: [[docs/releasing]] (the tx app: checklist steps, recovery), [[docs/ci]], README (download links, the cable notes), CHANGELOG
- [ ] Owner: the macOS app opens without the quarantine dance (signed and notarised), installs from the cask; on the 48SX, 49G and 38G with the app: connect, a drag each way, a backup; the Windows and Linux installers on a real port if a machine is at hand

## Acceptance

- The three installers build in `desktop.yml` and the macOS one is
  signed and notarised.
- Everything the web page does with a cable works in the app on macOS
  with the owner's calculators.
- The app and saturnus's app install side by side on all three systems.
