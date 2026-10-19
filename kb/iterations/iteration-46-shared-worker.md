---
title: "Iteration 46: one live calculator per model in a SharedWorker, used by the emulator and saturnus tx at once"
type: iteration
date: 2026-10-13
status: completed
branch: iter-46/shared-worker
tags:
  - iteration
  - saturnus-tx
  - web
---

# Iteration 46: one live calculator per model, shared by both pages

Iteration 41 shared one calculator per model between the emulator and
saturnus tx by handing it over: one page ran it at a time (Web Locks, a
BroadcastChannel, "In use by saturnus tx", Take it back). The owner
(2026-10-13): "Let's do the SharedWorker, yes!" Both pages now talk to
the same running calculator at once: the emulator page shows what
saturnus tx does, and saturnus tx sees what was typed in the emulator.

## Design

- **One SharedWorker per site origin** (`web/shared-worker.js`, served
  from the emulator's folder, so both pages name the same URL:
  `/saturnus/emulator/shared-worker.js` on the site, `/shared-worker.js`
  when web/ is served). It runs `web/hub.js`: one engine (the wasm
  `Host`, the protocol of web/protocol.md) per model, created when a
  page first boots or attaches that model, kept running while any page
  is connected to it, saved by the engine's auto-save as today and once
  more when its last page leaves. Pages speak the protocol of
  web/protocol.md to it as they spoke to the Worker.
- **Who owns what.** The hub owns the machines, the ROM slots
  (romstore.js) and the auto-saved states (the only writer, so no holder
  records). A page owns its connection: the hub tracks per connection
  which machine it shows, its speed choice, whether it is hidden,
  whether it watches the memory, and whether it holds the serial line.
- **Arbitration only where needed.** Keys from both pages go to the
  machine as they come (two hands). The serial line has one holder at a
  time (saturnus tx); a `serialOpen` from another page is refused with a
  sentence, and the emulator page's own memory writes (Kermit, hidden)
  are refused while saturnus tx holds the line ("saturnus tx is
  connected to this calculator…"). Typing SERVER, `probeServer` and
  `endServer` from saturnus tx need nothing more: the engine already
  orders sends and writes.
- **Speed.** The machine runs at the emulator page's speed. saturnus
  tx's choice (Max; 1x for a fast transfer) applies only while it moves
  bytes on the serial line (until 3 s after the last byte either way),
  or when no emulator page shows the machine. A calculator waiting for a
  packet sleeps, so between transfers the emulator's own setting is all
  the user sees.
- **Visibility.** A computing machine stops only when every page that
  shows it is hidden (saturnus tx never reports hidden: a transfer runs
  in a background tab, as before).
- **Pages that leave.** A SharedWorker hears no close of a port: each
  page holds a Web Lock of its own (`saturnus-page:<id>`) for its life,
  and the hub asks for the same lock, which it gets when the page is
  gone. saturnus tx also says `leave` when it disconnects. A leaving
  holder of the serial line gets its server ended (`serialClose`, then
  `endServer`); the last page to leave a machine has it saved and
  stopped.
- **Builds.** The page and the hub must be the same build: web/site.sh
  writes one build id into `shared/build.js` of both folders (the hash
  of everything it ships); a page whose id differs from the hub's is
  refused with the existing "This page's files are from different
  versions. Reload the page." The emulator's service worker precaches
  the SharedWorker script and its modules (it serves the worker, whose
  URL is in its scope).
- **Not shared.** A ROM file chosen in saturnus tx still boots a private
  calculator in tx/emulator-worker.js (no locks any more); the desktop
  apps are separate processes and keep their own engines (the
  emulator's native one, tx's Worker). A browser without SharedWorker
  (Samsung Internet) runs the emulator in a plain Worker (the same hub,
  one page) and saturnus tx says plainly that it cannot share the
  emulator's calculator there.
- **Removed**: `web/shared/calc-lock.js`, the holder records and
  `putIfHolder` in `web/shared/calc-states.js`, `away`, `IN_USE`, `ask`
  on `bootModel`, the emulator's "In use by…" and Take it back, tx's
  "…is elsewhere" card, `bootShared`/`handOver`/`wanted`/`taken`, and
  `?take=`: "Open in the emulator" opens the emulator on that model
  (`?model=48gx`). The state format is unchanged.

## Tasks

- [x] `web/hub.js`: machines per model, connections (join, leave, the page's lock), routing of replies and events (frames and status to every page of the machine, `serial` to the holder, `memoryChanged` to the pages that watch, an error to the page whose command caused it), the serial holder, speed and visibility per machine, auto-save, Remove ROMs
- [x] `web/shared-worker.js` (SharedWorker) and `web/worker.js` (plain Worker, one page) on the hub; `web/shared/calc-hub.js` for the pages (support check, the page's lock, join with the build id); `web/shared/build.js` written by web/site.sh
- [x] Emulator page: WorkerBackend on the SharedWorker (plain Worker without one); no `away`, In use or Take it back; `?model=` opens that model
- [x] saturnus tx: the kept-ROM calculator attaches to the hub's machine (`attach`), leaves with `leave`; tx/emulator-worker.js private only; the "elsewhere" card, `wanted`/`taken` gone; Open in the emulator with `?model=`; a plain message without SharedWorker
- [x] Remove calc-lock.js and the holder records; tests and docs that named them
- [x] Tests: unit tests of the hub's bookkeeping with a fake engine (join, leave by message and by lock, routing, serial holder, speed, visibility, last page saves and stops); the two-page test rewritten (emulator tab and tx tab on the 48GX at once: a variable typed in the emulator is listed in tx, tx's copy shows in the emulator's memory, closing either tab leaves the other running); existing worker tests on the hub
- [x] Docs: web/README.md, web/protocol.md, [[docs/saturnus-tx]], CHANGELOG (Unreleased), decision log; the time until tx is ready on a running calculator measured
- [ ] Owner: the emulator and saturnus tx side by side on the 48SX and the 49G as on the 48GX (a variable typed in one shows in the other at once)
- [ ] Owner: on the phone (Chrome on Android): both installed pages, a copy from tx while the emulator shows the calculator; Samsung Internet shows the plain message in tx
- [ ] Owner: Safari (16 or later): the same side by side
- [ ] Owner: the emulator tab in the background for a minute while tx copies: the copy runs, the emulator shows the result when brought back
- [ ] Owner: after a deploy with an emulator tab still open on the old build, a newly opened tx tab says "Reload the page" (and works once both are reloaded)

## Acceptance

- With the emulator and saturnus tx open on the 48GX, a variable typed
  in the emulator is listed in tx without any handover, and one copied
  in tx is in the emulator's memory and on its screen at once.
- Closing either tab leaves the other one's calculator running; closing
  both saves it, as the auto-save did.
- No Web Locks handover, no "In use…" text and no `?take=` remain.

## Out of scope

- Sharing between the desktop apps (separate processes).
- Several calculators per model.
