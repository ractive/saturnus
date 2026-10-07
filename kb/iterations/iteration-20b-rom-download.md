---
type: iteration
title: "Iteration 20b: ROM download from hpcalc.org in the apps"
date: 2026-10-06
status: planned
tags:
  - iteration
  - saturnus
branch: iter-20b/rom-download
---

# Iteration 20b: ROM download from hpcalc.org in the apps

Follows [[iterations/iteration-20-remember-roms]] (the ROM slots) and
needs the owner's answer from hpcalc.org (below) only for the web page's
one-click form.

## Context (owner, 2026-10-06)

"The saturnus apps should have pointers to hpcalc.org to where you can
download the ROMs. Would it be even allowed (licence wise) to add direct
links so that you could have a one click installation?"

What is established: the ROM images are HP's copyright; hpcalc.org hosts
the 48 and 49 images with HP's permission for emulator use (since 2000;
`saturnus rom fetch` and the README rest on this), and the 38G and
39G/40G images are HP's own update files mirrored there. Linking is
fine. Downloading on the user's machine from hpcalc.org, as `rom fetch`
does after a confirmation, is the same act as the user clicking the link.
Hosting or proxying the images ourselves would be redistribution by us,
which no permission covers, and is out. The 42S ROM is not downloadable
anywhere; users dump their own.

## Design

- **Desktop app**: beside each model's ROM slot a "Download from
  hpcalc.org" control (not for the 42S). It does what `saturnus rom
  fetch` does, in Rust, from the same table of known images (`romid`):
  a confirmation dialog first, stating what is downloaded, from where,
  whose ROM it is and under what terms it is hosted; the download
  identifies itself honestly (hpcalc.org serves a gzip bomb to fake
  browser user agents; `rom fetch` identifies as curl or Wget); size and
  SHA-256 verified; the file stored in the app's data directory and the
  slot set, so the model boots at once. Failures are reported with the
  link, so the user can download by hand.
- **Web page**: the browser cannot fetch from hpcalc.org (no cross-origin
  header there), so each empty slot shows the link to the model's
  download page on hpcalc.org, the file name to expect, and the hint to
  drop the file on the page (iteration 20 assigns it by content). If the
  owner obtains hpcalc.org's agreement to a cross-origin header, a
  one-click fetch in the Worker follows the desktop app's rules.
- **Wording** in both apps: HP's ROM, hosted by hpcalc.org with HP's
  permission for emulator use; not part of saturnus.
- **Owner's action, outside the code**: ask Eric Rechlin (hpcalc.org)
  whether automated downloads from the apps are acceptable and whether
  he would add a cross-origin header for the web page.

## Tasks

- [ ] The known-images table (`romid`) carries each model's hpcalc.org
  URL, file name and the page to link (today in `saturnus-cli/src/rom.rs`;
  one place for CLI, app and page).
- [ ] Desktop app: the download control, the confirmation dialog, the
  download in Rust with the honest user agent and the verification, the
  slot set and booted; errors with the link. Tests with a local HTTP
  stub (no network in tests).
- [ ] Web page: the per-model link, file name and drop hint on empty
  slots; the About panel's ROM paragraph.
- [ ] README and `web/README.md`.

## Acceptance criteria

- [ ] In the desktop app with no ROMs, one click per model (and the
  confirmation) leaves the 48SX, 48GX, 49G, 38G and 39G/40G bootable.
- [ ] The web page links each model to its hpcalc.org download and a
  dropped file lands in the right slot.
- [ ] `just gates` passes.

## Outcome

(to be written)
