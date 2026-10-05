---
type: iteration
title: "Iteration 20: Remember the ROM per model, and recognise the other ROMs"
date: 2026-10-06
status: planned
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

- [ ] ROM identification (exact, fits, unknown) with unit tests on
  synthetic images and ROM-gated tests on the seven images.
- [ ] Desktop app: settings file, remembered path per model, boot on
  model selection, "Change ROM...", the same-directory scan with its
  bounds, missing or changed file handling, last model at start.
- [ ] Web page: IndexedDB store, the changed promise text, "Forget ROMs",
  several files at once in the picker and by drag and drop.
- [ ] `<sat-controls>`: one row per model's slot state (remembered file
  name, or "choose"), the notice after a scan, errors; keyboard
  accessible.
- [ ] `web/protocol.md`, README, `web/README.md`, the About panel's
  privacy sentence.
- [ ] Verification: headless Chrome (choose once, reload, the model boots
  without a picker; several files at once; forget; a private window);
  the desktop app through its self-test hook and by hand (choose one ROM
  in a folder holding the others, restart the app, switch models without
  a dialog; move a file away and get asked).

## Acceptance criteria

- [ ] After choosing a 48SX ROM once, restarting the desktop app or
  reloading the page and selecting the 48SX boots it without a file
  dialog.
- [ ] Choosing one ROM in a folder that holds the others makes the other
  models bootable without a further dialog; an unrecognised file in that
  folder is not assigned to anything.
- [ ] "Forget ROMs" leaves no ROM bytes in the browser's storage.
- [ ] `just gates` passes.

## Outcome

(to be written)
