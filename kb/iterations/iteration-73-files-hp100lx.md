---
type: iteration
title: "Iteration 73: the HP 100LX/200LX in Files"
date: 2026-11-08
status: planned
tags:
  - iteration
  - web
  - hp100lx
branch: iter-73/files-hp100lx
---

# Iteration 73: the HP 100LX/200LX in Files

After [[iterations/iteration-72-filer-proto]]: Files reaches a palmtop in
Filer server mode, in the web app (Web Serial) and the desktop app, as a
third kind beside the calculators on a Kermit server or XSERV and the
38G, 39G and 40G served a folder. One app, as decided in 0.6.0.

## Tasks

- [ ] **A palmtop device** in `crates/saturnus-tx` on `filer-proto`: the
  operations Files has (list, get, put, rename, delete, mkdir, rmdir,
  copy a tree), drives as the top level (the drive list), 8.3 names (a
  longer name is refused with a clear message, or shortened on request),
  DOS dates and attributes in the listing, progress and Stop like the
  Kermit device.
- [ ] **"Add a calculator"** gets a third choice, "An HP 100LX or 200LX
  palmtop", with what to do on the palmtop (Filer, MENU, Communications,
  Remote Settings: COM1, 19200, server mode on; Filer left in front) and
  the cable warning (the palmtop's cable must never go on a 48 or 49G).
- [ ] Speed: 19200 by default, faster as a setting with the warning that
  the stock UART may lose bytes above it; a failed connect suggests 19200.
- [ ] Tests: driver and page tests against the in-process server, the
  trace replay as for the other devices.
- [ ] Docs: `web/docs/pages/files.md`, the help, `kb/docs/saturnus-tx.md`.
- [ ] **Owner session with the real 100LX:** connect, list drives and
  directories, get and put both ways, rename, delete, a directory, the
  error cases (missing file, existing file, the ROM disk), the fastest
  speed that holds, auto-off while serving. Facts learned go to the wiki.

## Acceptance

- On the owner's 100LX: Files lists, transfers both ways byte for byte,
  renames, deletes and makes directories; errors say what happened.
- The calculators connect exactly as before. `just gates`, `hyalo lint`
  clean.
