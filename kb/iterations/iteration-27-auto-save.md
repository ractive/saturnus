---
type: iteration
title: "Iteration 27: The calculator keeps its state"
date: 2026-10-08
status: completed
tags:
  - iteration
  - saturnus
branch: iter-27/auto-save
---

# Iteration 27: The calculator keeps its state

Owner (2026-10-08): a reload boots the ROM again with empty RAM: the ROM
asks "Try To Recover Memory?" and the stack and variables are gone unless
the user pressed Save state and then Load state. A real calculator keeps
its memory when turned off; saturnus should too, in the browser (also
the installed app on a phone, which the system may kill when it is in
the background) and in the desktop app.

Read first: `web/backend.js`, `web/worker.js`, `web/romstore.js`,
`web/app.js`, `web/components/sat-controls.js`, `web/protocol.md`,
`crates/saturnus-host/src/protocol/`, `crates/saturnus-drive/src/runner.rs`,
`crates/saturnus-tauri/src/lib.rs`; iterations 22 (the installed app) and
28b (persistent storage).

## Design

- **When.** About 5 s after the last change, and at once when the page
  is hidden (`visibilitychange`, `pagehide`; the desktop window closing).
  Only when the machine changed: a dirty mark set by what the page or the
  outside does (keys, sends and writes at their start and end,
  `loadState`, `reset`, `poke`, `keyScript`, serial input), never by the
  machine keeping time, so an idle calculator is never written (the 49G's
  state carries its 2 MB flash). A hash of RAM and CPU was the other
  option; it moves while the calculator idles (clock, cursor, timers).
- **Never half done.** The save waits until the machine has settled: no
  send or write in progress, the CPU asleep with no key down or queued,
  not halted. Hidden mid-transfer, it is saved as soon as the transfer
  ends. In the browser a computation stops while the page is hidden, so
  its save waits until the page is shown again (said in the code).
- **One protocol.** The rules are the state machine's
  (`crates/saturnus-host/src/protocol/autosave.rs`), for every host that
  keeps states (`Engine::set_auto_save`): it hands the state out as
  `Output::Save` and boots with a kept state (`Engine::boot_restoring`).
  The Worker keeps it in IndexedDB (`saturnus`/`states`, `auto:<model>`),
  the desktop app in `states/<model>.auto.state` in its data folder
  (`StateDir`, `Runner::set_store`). Then the page hears `autoSaved`.
- **Two slots per model.** The auto slot never overwrites the user's
  Save state, which stays where it was.
- **Restore on boot.** `bootModel`, `chooseRom`, `downloadRom` (and the
  app's `boot`) restore the kept state before the machine runs a cycle:
  no recover question. A state that does not load (another ROM, a format
  change) leaves the cold boot; `restoreError` says why, the host logs
  it, no dialog.
- **Start fresh.** A palette command; the page asks in its own card
  ("Start the HP 48SX fresh? …", Cancel / Start fresh, Escape cancels),
  then `bootModel` with `fresh`: a cold boot, the kept state deleted.

## Tasks

- [x] `protocol/autosave.rs`: the dirty mark, the delay, hidden, the
  settled test; `Output::Save`, `Engine::boot_restoring`,
  `Booted.restored`/`restoreError`, `set_auto_save`; the change points in
  the engine.
- [x] wasm `Host`: auto-save on; `drain` hands out `autoSave` with the
  bytes; `boot` takes the kept state.
- [x] `web/states.js` (the states database, shared by the page and the
  Worker); `worker.js` keeps `auto:<model>` one write after the other,
  posts `autoSaved`, boots with the kept state, deletes it for `fresh`,
  writes no 49G state after Forget ROMs; `romstore.js` passes `fresh`.
- [x] Page: `backend.startFresh`, `pagehide`/`pageshow`, the "Start
  fresh" palette command and its in-page question (`web/fresh.js`), Forget
  ROMs deletes both 49G slots.
- [x] Native: `StateStore`/`StateDir` (`saturnus-drive`), the runner's
  store, restore and `fresh` on `boot`, `autoSaved`; the Tauri app's
  `states` folder, `fresh` through `bootModel`, a save when the window
  closes.
- [x] Tests: the policy (`autosave.rs`), the engine on a ROM that only
  sleeps (idle never saved; one key, one save after the delay; keys in a
  row, one save; hidden at once; a key down waits; nothing during a send
  or a computation; a model switch; a restore that does not load), the
  ROM-gated `crates/saturnus-host/tests/autosave.rs` (48SX, 48GX, 49G),
  the Worker against fakes (`web/test/autosave-worker.test.mjs`), headless
  Chrome with a ROM (`web/test/autosave.test.mjs`), the Tauri runner
  (`keeps_the_calculator_across_restarts`).
- [x] `web/protocol.md` ("Auto-save"), `web/README.md`, `README.md`, the
  decision log.

## Acceptance criteria

- [x] An idle calculator writes nothing (engine test, ROM-gated test over
  120 s, headless Chrome and the Tauri runner over 7 s).
- [x] One key press leads to exactly one write after the delay; a hidden
  page saves at once; nothing is saved mid-transfer, mid-send or while
  computing.
- [x] On the 48SX, 48GX and 49G a state saved after `42 'V' STO 1 2`
  restores a fresh machine with the same stack, variable and screen, no
  recover question, and it computes on (`+` gives 3).
- [x] In headless Chrome `1 ENTER 2`, a reload: the same stack and
  command line; Start fresh asks in the page, cold-boots and deletes the
  kept state.
- [x] The desktop runner keeps the state in its folder, restores it on
  the next start, saves on hidden, forgets it for `fresh`.
- [x] The user's Save state and Load state are unchanged and in their own
  slot.
- [x] The owner checks on the phone: the installed app killed and
  reopened shows the same stack.

## Outcome

Done as designed. The engine's dirty mark and settled test decide; the
Worker and the desktop app only store. The ROM-gated test restores
states of 33 KB (48SX), 131 KB (48GX) and 2.6 MB (49G); the 49G's is
written only after a change, never while it idles. A change made while
the page is hidden mid-computation waits for the page to be shown again
(the browser stops the computation), which leaves the last settled state
for a page the system kills then. The Tauri app's states are not
deleted by Forget ROMs, like its state files. The owner confirmed on
the phone that auto-save works (2026-10-09).
