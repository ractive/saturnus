---
type: iteration
title: "Iteration 20: Remember the ROM per model, and recognise the other ROMs"
date: 2026-10-06
status: completed
tags:
  - iteration
  - saturnus
branch: iter-20/remember-roms
---

# Iteration 20: Remember the ROM per model, and recognise the other ROMs

Needs iteration 12 (PR 24) merged: both touch the page's controls.
"App" means the web page and the desktop app alike.

Read first: `web/components/sat-controls.js`, `web/app.js`,
`web/backend.js`, `web/store.js`, `web/protocol.md`,
`crates/saturnus-tauri/src/lib.rs` (dialogs opened in Rust; the page
never names a file), `crates/saturnus-cli/src/rom.rs` (the known images
with size and SHA-256), `crates/saturnus-drive/src/` (ROM loading),
`kb/iterations/iteration-11-tauri-host.md` (Outcome: the review finding
on page-supplied paths).

## Context (owner, 2026-10-06)

"It would be amazing if you only need to select the rom once for each
calculator and that it then remembers which file you selected. I guess
in 99% of the cases you won't change that again. Maybe you can even guess
or predict which ROM to select for which model if a rom in a folder once
has been selected. I guess the other roms are normally kept in the same
folder. But memorizing it would be great."

## Design

- **Identify a ROM by its content**, in one place usable by every host
  (wasm-clean): SHA-256 against the images `saturnus rom fetch` knows
  (exact model and revision); otherwise size plus the image's own
  structure (packed or unpacked, the checks the loader already makes) to
  the set of models it fits. The result is `exact(model, revision)`,
  `fits([models])` or `unknown`. The 39G and 40G share one image: both
  slots are offered. An `unknown` or ambiguous image is never assigned
  silently.
- **Desktop app**: the chosen file's path is remembered per model in the
  app's settings (the platform's config directory; a small JSON file).
  Selecting a model with a remembered ROM boots it; "Change ROM..." picks
  another. When a ROM is chosen through the dialog, the other regular
  files in the same directory (not below it; bounded count and size, the
  read cap of iteration 11) are identified and offered for their models:
  exact matches are assigned to empty slots at once and listed in a
  short notice, `fits` matches are offered, nothing overwrites a slot the
  user set. Paths never come from the page (iteration 11's rule): the
  page learns `{model, fileName, state}` per slot and sends "boot model
  M" or "choose a ROM for M". A remembered file that is missing, or whose
  content no longer identifies as before, is reported and asked for
  again; nothing else is booted in its place. The last used model is
  remembered and booted at start if its ROM is there (a setting, on by
  default).
- **Web page**: a browser cannot remember a path, so the ROM's bytes are
  kept in this browser's IndexedDB (where saved states already are), per
  model, with the file's name and hash. The page's promise changes from
  "Read in this page only, never uploaded or stored" to "kept in this
  browser so you do not have to pick it again; never uploaded", with a
  "Forget ROMs" control that removes them (and says what else stays:
  saved states). Remembering is on by default (owner: "memorizing it
  would be great"); a failed or refused store (private window, quota)
  degrades to today's behaviour with a note. The file picker and drag and
  drop accept several files at once: each is identified and assigned as
  in the desktop app, so all ROMs can be given in one go.
- **Protocol**: the slots and the verbs are host-neutral (`romSlots`,
  `bootModel`, `chooseRom`, `forgetRom`), implemented by the Worker host
  over IndexedDB and by the Tauri host over the settings file; the
  control API is not concerned (the ROM is given on the command line).
- **No ROM leaves the machine and none is committed**: the settings file
  and the browser store live outside the repository; tests use temporary
  directories and synthetic images.

## Tasks

- [x] ROM identification (exact, fits, unknown) with unit tests on
  synthetic images and ROM-gated tests on the seven images.
- [x] Desktop app: settings file, remembered path per model, boot on
  model selection, "Change ROM...", the same-directory scan with its
  bounds, missing or changed file handling, last model at start.
- [x] Web page: IndexedDB store, the changed promise text, "Forget ROMs",
  several files at once in the picker and by drag and drop.
- [x] `<sat-controls>`: one row per model's slot state (remembered file
  name, or "choose"), the notice after a scan, errors; keyboard
  accessible.
- [x] `web/protocol.md`, README, `web/README.md`, the About panel's
  privacy sentence.
- [x] Verification: headless Chrome (choose once, reload, the model boots
  without a picker; several files at once; forget; a private window);
  the desktop app through its self-test hook and by hand (choose one ROM
  in a folder holding the others, restart the app, switch models without
  a dialog; move a file away and get asked).

## Acceptance criteria

- [x] After choosing a 48SX ROM once, restarting the desktop app or
  reloading the page and selecting the 48SX boots it without a file
  dialog.
- [x] Choosing one ROM in a folder that holds the others makes the other
  models bootable without a further dialog; an unrecognised file in that
  folder is not assigned to anything.
- [x] "Forget ROMs" leaves no ROM bytes in the browser's storage.
- [x] `just gates` passes.

## Outcome

**Identification** (`crates/saturnus-web/src/romid.rs`, wasm-clean, the
one list of known images; the CLI's table and SHA-256 moved there):
`identify` gives exact (SHA-256 known: models, revision), fits (models by
size and form) or unknown; `plan` assigns a batch with the rules in the
decision log. On the seven images: `sxrom-j` exact 48SX J, `gxrom-r`
exact 48GX R, `38G_A167.ROM` exact 38G A1.67, `rom.49g` exact 49G 2.15,
`rom-2.10.49g` exact 49G 2.10, `rom.39g` exact 39G and 40G,
`hp42s-c.rom` fits the 42S only (no published image to know it by).
Tests: `romid::tests` (synthetic sizes and forms, the folder plan,
chosen files, JSON), `rom_dir_images` (ROM-gated, the seven images).

**Desktop app** (`crates/saturnus-tauri/src/roms.rs`, the ROM commands
in `lib.rs`): `settings.json` in `app_config_dir` (macOS `~/Library/
Application Support/ch.ractive.saturnus/`, Linux
`~/.config/ch.ractive.saturnus/`, Windows
`%APPDATA%\ch.ractive.saturnus\`), 0600 in a 0700 directory, written
whole. Tests: `roms::tests` (choose, scan, offer, reopen, mode 0600, no
path in the slots, missing, changed, forget; refusals; scan bounds and
links; ROM-gated `rom_dir_folder`). Self-test `selftest-roms.js`
(`SATURNUS_SELFTEST_SCRIPT=roms`, phases `choose`, `hold38g`, `restart`,
`missing`) on a scratch copy of the ROM folder plus `notes.txt` and an
unknown 512 KB file, with `SATURNUS_SETTINGS_DIR` in scratch:
- choose: the 48SX ROM chosen once boots; the notice "Also found:
  38G_A167.ROM for the 38G, gxrom-r for the 48GX, rom-2.10.49g for the
  49G, rom.39g for the 39G and 40G. hp42s-c.rom could be the 42S ROM.";
  48GX, 49G, 39G then boot by selecting them, no dialog; the 42S by its
  offer button; the unknown file and `notes.txt` assigned to nothing;
  `bootModel` with `romPath` and `chooseRom` with `path` refused.
- hold38g then `kill -9`, relaunch: the 38G boots at start.
- missing (`gxrom-r` moved away, 48GX the last model): at start
  "gxrom-r, the HP 48GX ROM, is no longer where it was; choose it
  again.", nothing booted; selecting the 48GX reports it again and opens
  the dialog ("Choose the 48GX ROM").
- The iteration 11 self-test still passes (boot, keys, state, memory
  view, 1x 100.00 %, 4x 400.01 %).

**Web page** (`web/romstore.js` in the Worker, `<sat-controls>`): IndexedDB
`saturnus-roms`, the new promise text, "Forget ROMs", several files in
the picker and by drop, the slot table under "ROMs of every model", the
notice with offer buttons, a start-with-the-last-model switch. Node tests
`web/test/romstore.test.mjs` (kept and booted after a reload, several
files, a changed image, forget, a refusing store, a quota error).
Headless Chrome 154 over the DevTools protocol on a served `web/`:
choose `sxrom-j` once, reload: the 48SX boots with no picker, and
selecting it again boots it; the six other files plus `notes.txt` at
once: every model got its ROM (39G and 40G one copy), `notes.txt` "is
not a ROM image of a model saturnus runs"; each model then boots by
selection; Forget: the store holds no image and only the settings
record (5 570 560 bytes in 6 images before). A synthetic 512 KB file
dropped on the page while the 48GX is selected goes to the 48GX.
Incognito: works and keeps the ROM across reloads of the session.
IndexedDB throwing on access in the Worker (injected) and a quota error
on the image write (injected): the page boots the ROM, the panel says
the ROMs are not kept, and a reload starts empty with no half-written
slot (found here: a failed write first left the slot without its image;
the transaction is now aborted). Narrow sheet and Tab order checked.

**Protocol**: `romSlots`, `bootModel`, `chooseRom`, `forgetRom`,
`romSettings` in `web/protocol.md`, "ROM slots". README, `web/README.md`
and the About panel's ROM sentence (`scripts/about-json.py`; only that
line of `web/about.json` changed) updated.

**Deviations**: the 39G/40G image fills both slots at once (exact for
both) instead of being offered; a fifth command, `romSettings`; the
Worker now runs commands through a promise queue. **Not verified**: the
native dialogs themselves (the hook answers them; "Change ROM…" and the
dialog's start folder were not clicked by hand), real drag and drop from
the Finder (a synthetic drop event was dispatched), Windows and Linux
settings paths, a real Safari or Firefox private window.
