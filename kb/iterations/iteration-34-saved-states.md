---
type: iteration
title: "Iteration 34: Saved states, the same on both hosts"
date: 2026-10-09
status: completed
tags:
  - iteration
  - saturnus
  - web
  - tauri
branch: iter-34/saved-states
---

# Iteration 34: Saved states, the same on both hosts

Owner (2026-10-09): "Save state" and "Load state" behave the same in the
browser and in the desktop app, and both can also write and read files.
Before, the browser kept one IndexedDB slot per model and loaded it at
one click with no question, replacing the earlier save in silence; the
desktop app went through a file dialog each time.

Read first: `web/backend.js`, `web/states.js`,
`web/components/sat-controls.js`, `web/app.js`, `web/components/confirm.js`,
`web/components/menu.js`, `crates/saturnus-tauri/src/lib.rs`,
`crates/saturnus-drive/src/files.rs` and `runner.rs`; iteration 27
(auto-save).

## What it does

- **Save state** keeps the state in a per-model slot inside the host, at
  one click, no question: IndexedDB in the browser (key `<model>` of
  `saturnus`/`states`, as before), `states/<model>.state` in the app's
  data folder, next to the auto-saved `<model>.auto.state` and never the
  same file. The status line says what it replaced: "State saved; it
  replaces the one from 9 Oct, 14:02." (the year only when it is not
  this year), or "State saved." the first time.
- **Load state** asks first in the shared modal: "Load the state saved
  on 9 Oct 2026, 16:40?", "It replaces the calculator's memory, stack
  and screen.", Load / Cancel. Off, with the reason "No saved state for
  this model", when there is none; its tooltip names the save's date.
- **Save state to a file…**: a download named `48gx-2026-10-09-1640.sat`
  in the browser, the save dialog with that name in the app. The bytes
  are `Machine::save_state`'s, the format the app's dialogs wrote and
  the CLI reads (`saturnus run --load FILE`, `--save FILE`; the brief's
  `--state` is `--load`). The tooltip says a 49G state contains the
  49G's ROM.
- **Load state from a file…**: the browser's file picker or the app's
  open dialog (`.sat`, and `.state` for the files the app wrote before),
  then the same question naming the file. The page reads the state's
  header first (`SATURNUS`, version, model): a broken file says "This
  file is not a saturnus state."; a state of another model is
  **refused** in plain words ("This state is from an HP 48SX. Switch to
  the HP 48SX first, then load it."). Offering the switch is not done:
  the switch would also need that model's ROM to be the one the state
  was saved from, which only the load can tell; a follow-up if wanted.
  A state of another ROM of the same model is refused by the core
  ("This state was saved with another ROM of the HP 48GX.").
- **Panel**: Save state and Load state stay the visible buttons; a "⋯"
  beside them opens the menu (web/menu.js, as the ROM rows) with the two
  file commands. All four are in the palette.

## Protocol (desktop app)

The page still names no path. `saveState` and `loadState` take
`slot: true` with `model` (the host's slot file, no dialog; a save for
a model other than the running one is refused) and `saveState` takes
`name` (a save dialog offering it). Without either, `saveState` answers
the bytes (as the Worker and HTTP do) and `loadState` takes them in
`state`. Two host commands are answered by the app itself:
`stateSlot` (`{saved}`, the slot file's time in ms, or null) and
`openState` (an open dialog; `{name, state}` with the bytes in base64,
or null). In the browser the `WorkerBackend` answers the same calls
from IndexedDB and a file input; the Worker's commands are unchanged.

## Migration

The browser's slot keeps its key and record (`{state, saved, cycles}`):
a state saved before still loads. The app had no slot (each save was a
file the user chose), so there is nothing to migrate; those files load
with "Load state from a file…".

## Tasks

- [ ] `web/savedstates.js` (pure): dates, file name, header check,
  questions and messages; `web/test/savedstates.test.mjs`
- [ ] Backends: `stateSlot`, `saveStateSlot`, `loadStateSlot`,
  `saveStateFile`, `openStateFile`, `loadStateBytes` on both
- [ ] Panel: the confirm on Load, the replace message, the ⋯ menu with
  the file commands, the file input; palette entries
- [ ] App: `StateDir::saved_file`, slot and name in `need`, `stateSlot`,
  `openState`, the runner's model check; Rust tests; `selftest.js`
- [ ] Page tests in Chrome: confirm and Cancel leaves the machine, the
  replace message, the disabled reason, wrong model, broken file
- [ ] ROM-gated round trip: a file the page saves loads in the CLI, and
  one the CLI saves loads in the page
- [ ] `web/README.md`, `web/protocol.md`, README, CHANGELOG, decision log
- [ ] Owner: the four commands in a browser and in the desktop app, on
  a phone too
