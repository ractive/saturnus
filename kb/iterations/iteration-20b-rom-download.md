---
type: iteration
title: "Iteration 20b: ROM download from hpcalc.org in the apps"
date: 2026-10-06
status: completed
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

- [x] The known-images table (`romid`) carries each model's hpcalc.org
  URL, file name and the page to link (today in `saturnus-cli/src/rom.rs`;
  one place for CLI, app and page).
- [x] Desktop app: the download control, the confirmation dialog, the
  download in Rust with the honest user agent and the verification, the
  slot set and booted; errors with the link. Tests with a local HTTP
  stub (no network in tests).
- [x] Web page: the per-model link, file name and drop hint on empty
  slots; the About panel's ROM paragraph.
- [x] README and `web/README.md`.

## Acceptance criteria

- [x] In the desktop app with no ROMs, one click per model (and the
  confirmation) leaves the 48SX, 48GX, 49G, 38G and 39G/40G bootable.
- [x] The web page links each model to its hpcalc.org download and a
  dropped file lands in the right slot.
- [x] `just gates` passes.

## Outcome

Owner's decision (2026-10-08): "Do the direct linking. We will not have
soooo many visitors." The desktop app downloads after a confirmation, the
page links; the question to hpcalc.org about a cross-origin header stays
open (the page's one-click form waits for it). Decisions in the decision
log, 2026-10-08 (iteration 20b: ROM download).

**The table** (`crates/saturnus-host/src/romid.rs`, `KNOWN` of
`KnownRom`): per image SHA-256, models, revision, file name, size, zip URL
and hpcalc.org details page (48SX 4371, 48GX 4368, 38G 4775, 49G 2.15
6744, 49G 2.10 8888, 39G/40G 6739); `download(model)` and
`download_json(model)`. The CLI's `RomSource` table is gone. Tests:
`romid::tests::known_images_are_exact_and_fit_their_models`,
`downloads_per_model`.

**The fetch** (`crates/saturnus-drive/src/fetch.rs`, shared by `saturnus
rom fetch` and the app): system `curl -q` with its own user agent, no
compression asked, http/https only, at most 8 MiB; the zip member read in
Rust (`miniz_oxide`), size and SHA-256 checked before the image is written
whole. `unzip`/`tar` are no longer used. Tests against a local HTTP stub
(no network): `fetch::tests::fetch_downloads_verifies_and_keeps` (curl's
user agent, no Accept-Encoding, kept when present, a bad file refused or
replaced), `nothing_stored_on_failure` (wrong SHA-256, 404, over the cap,
member missing, `file://`), `unzip_reads_stored_and_deflated_members`,
`known_images_convert`; CLI `rom::tests`.

**Desktop app**: `downloadRom` (a ROM command in its sequencer turn):
native confirmation (`confirm_download`), the download into `roms/` in
`app_data_dir`, then `Library::choose` and the boot, sent only while the
turn holds; errors name the hpcalc.org page. Each slot carries
`download`. Controls: "Download…" on empty slots and in the empty state.
Tests: `roms::tests::a_download_sets_the_slot` (stub: a 500 and a wrong
image store nothing and name the page; the right one is stored, chosen,
booted, then found present), `slots_carry_the_download`,
`tests::a_failed_boot_after_a_choice_keeps_the_notice`. Self-test phase
`download` (`SATURNUS_SELFTEST_SCRIPT=roms`, by hand only): on empty
scratch settings and data directories, the five Download buttons with the
confirmation answered by the hook downloaded from hpcalc.org and booted
48SX (`sxrom-j`), 48GX (`gxrom-r`), 38G (`38G_A167.ROM`), 49G (`rom.49g`,
2.15) and 39G (`rom.39g`, the 40G filled with it); 42S left empty, no
Download button left; `saturnus rom fetch` then found all five present
and verified. Scratch copies deleted.

**Web page**: the Worker's slots carry `download` from the wasm core's
`rom_download`; an empty slot shows the file name linked to its hpcalc.org
page, the empty state "Get sxrom-j from hpcalc.org, unzip it and drop the
file here.", a source hint under the slots, and the About panel's ROM
paragraph (`scripts/about-json.py`). Tests: `web/test/download.test.mjs`
(real wasm), `norom.test.mjs`, `romstore.test.mjs`; headless Chrome at
390 and 1280 px shows the links without horizontal overflow, and the
overflow test passes.

Open: the acceptance checks by hand (the real confirmation dialog in the
app; a file downloaded from a link and dropped on the page). Clicking an
external link in the app's webview behaves as the About panel's links do
(not changed here).
