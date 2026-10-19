---
title: "Iteration 45: saturnus tx, see what's happening (progress where you clicked, the cable console)"
type: iteration
date: 2026-10-13
status: completed
branch: iter-45/tx-progress-console
tags:
  - iteration
  - saturnus-tx
  - web
---

# Iteration 45: see what's happening

Owner requests of 2026-10-13, all in this iteration. Read first:
[[docs/saturnus-tx]] ("Layout", "Preview", "Shared calculators"),
[[docs/test-policy]], `web/README.md`, `web/protocol.md`.

1. **Progress where you clicked.** Owner: "When I click on an image in
   the left calculator pane, it takes some seconds to load it. I only
   see a small 'Readin...' at the very bottom."
2. **A cable console.** Every packet live, "a bit of a matrix feeling".
3. **Open in the emulator** from the pane that shows an emulated
   calculator. Owner: "Why not putting the 'Emulator' button on the left
   pane, where the actual emulator is running?"
4. **Firefox wording.** Where no folder can be picked, "This page's
   files" / "Kept in this browser for this site" sent the owner looking
   for EQ.hp in Downloads.

Another agent works on `fix/tx-emulated-fast` (`kermit-device.js`,
`calc-lock.js`, the Worker's typing and handover, speed): this
iteration does not restructure those files; it adds at most a field to
an event.

## Design

### 1. Progress where you clicked

- The preview opens at once on a click and says what it does: "Reading
  P from the 48SX… 412 of 1,106 bytes", with a bar filled from the
  device's packet progress (the size is known from the listing). The
  content replaces it when it is there. Waiting behind another job it
  says "Waiting for … to finish".
- A hint when it is slow: from the link's speed and the size, "Slow over
  the cable at 9600 baud: about 4 s." (only when the estimate is 2 s or
  more; the emulated calculator talks at the real one's speed and says
  so). Later in the read the estimate comes from the measured rate.
- The row being read or written shows a small progress marker in the
  list (a bar under its name, `<device-list>`'s `setBusy`); during a
  copy the source row and the target row (when it is already there, a
  replace, or the directory it goes into) do too.
- The pane's status line stays, larger (the body size, not the small
  print), with the same numbers.
- Progress events are drawn at most once per animation frame.

### 2. The cable console

- "Show the cable" in the page's header opens a panel under the panes:
  "The cable", with Pause, Clear, Copy log and Close.
- One line per packet, newest at the bottom: time, which pane, the
  direction (→ to the calculator, ← from it), the Kermit packet type
  with its sequence number and length (`S`, `F`, `D`, `Y`, `N`, `Z`,
  `B`, `E`, `I`, `G`, `R`, `X`, `A`), a plain-English description
  ("send-init", "file name P", "data, 412 of 1,106 bytes", "NAK: resend
  3", "resent"), and the payload as text and hex, truncated. XModem
  blocks the same way (block number, size, ACK, NAK, EOT, CAN, the
  receiver's "C"). Anything else as bytes. Retries, NAKs, errors and
  bad checks in red.
- The look is the panel's own on purpose: green monospace on black in
  both themes, with a faint glow on the newest line; no animation under
  `prefers-reduced-motion`; contrast well above 4.5:1.
- Built on a tap: every link a pane or the serving screen opens is
  wrapped (`TappedLink`) before the protocol sees it, so it works the
  same for Web Serial, the emulated calculator and the desktop app's
  native serial (`web/tx/tauri.js`'s `TauriSerialLink`), and for the
  38G/39G/40G serving screen.
- It must not slow transfers: the decoder runs on the page's side of
  the protocol Worker over bytes it already has; lines go into a ring
  buffer of 3,000; while the panel is closed nothing else happens; while
  open, new lines are drawn once per animation frame, and the panel
  keeps at most the ring buffer's lines.

### 3. Open in the emulator

- A pane that shows the emulator's own calculator (a shared one, from a
  ROM the emulator keeps; not one from a ROM file, which the emulator
  cannot have) gets "Open in the emulator" in its head, next to Change.
- It opens the emulator page in a new tab with `?take=<model>`. The
  emulator reads it at start and boots that model asking for it (the
  path of Take it back), so saturnus tx hands it over: its copies end,
  it leaves SERVER, saves and shows "In use by the emulator" with Take
  it back. The parameter is removed from the address afterwards, so a
  reload does not take it again.
- Not in the desktop app (no emulator page there). The page header's
  general "Emulator" link stays.

### 4. Files in this browser

- The place is named "Files in this browser" (switcher, card, pane
  head, serving screen), its status "Not on your disk. Use ⋯ → Download
  to save a file." (the in-memory fallback: "Lost when this page closes.
  Use ⋯ → Download to save a file.").
- After the first copy into it in a visit, the toast says the same once.

## Tasks

- [x] Plan (this file)
- [x] Preview: the reading state with the place, bytes, bar and slow hint; drawn per animation frame
- [x] `<device-list>` row progress marker (`setBusy`), for a preview read and both rows of a copy
- [x] The pane's status line larger
- [x] `web/tx/cable.js`: the tap, the Kermit and XModem decoder, the ring buffer (pure, unit-tested)
- [x] The cable panel (`web/tx/cable-view.js`): Show the cable, Pause, Clear, Copy log, Close; batched drawing; the look; reduced motion
- [x] Tap every link: the panes (cable, emulated, desktop) and the serving screen
- [x] "Open in the emulator" in the pane head; the emulator's `?take=` at start
- [x] "Files in this browser" wording, status line and the one-time note after a copy
- [x] Tests: decoder and ring buffer over the recorded page trace (`web/test/tx-cable.test.mjs`); the page test: the console lists the packets, the preview's progress, the new wording, the button; the emulator's `take` parameter
- [x] Docs: [[docs/saturnus-tx]], `web/README.md`, CHANGELOG
- [x] Two screenshots (1280 and 390 wide, light): the progress preview and the console
- [ ] Owner: on a real 48SX over the cable, click a picture: the preview says what it reads with the bar and the time; the console shows the packets, a NAK in red when the cable hiccups
- [ ] Owner: "Open in the emulator" on the left pane's emulated 48GX takes it over in the emulator tab; saturnus tx says "In use by the emulator"
- [ ] Owner: in Firefox, the right pane says "Files in this browser" and how to save a file

## Acceptance

- Clicking a variable that takes seconds shows at once, in the preview,
  what is read and how far, with a bar.
- The cable panel lists every packet of a session against the emulated
  48GX, with types, sequence numbers and descriptions, and a copy with
  the panel open takes no longer than without (the e2e suite's times).
- "Open in the emulator" moves the shared calculator to the emulator
  page.
- No page says "This page's files" any more.

## Out of scope

- Saving the cable log as a trace file to replay (the tests' recording
  link does that already).
- A packet-level view of the emulator's own memory view writes.
