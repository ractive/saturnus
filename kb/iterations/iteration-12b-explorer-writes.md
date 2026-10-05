---
type: iteration
title: "Iteration 12b: Explorer writes (store, fetch, purge, rename, flags)"
date: 2026-10-05
status: planned
tags:
  - iteration
  - saturnus
branch: iter-12b/explorer-writes
---

# Iteration 12b: Explorer writes (store, fetch, purge, rename, flags)

Split from [[iterations/iteration-12-memory-explorer]] on 2026-10-05: the
read-only layer could start at once; everything that changes calculator
memory waits here.

**Blocked until `kermit-proto` is on crates.io** (see the decision-log
entry "control API replaces MCP"): no `hptx-core` in the page or in any
saturnus host. The hidden Kermit transaction is built on `kermit-proto`
plus the HP file header and text encoding in `saturnus-objects`;
keystrokes are the fallback where no Kermit server exists. Direct RAM
writes stay out, so the ROM's memory manager stays consistent.

## Tasks

- [ ] Hidden Kermit transaction at unlimited speed in the shared runner
  and the Worker host: enter server mode, do the transfer, leave, restore
  the screen; a busy overlay in the page while it runs.
- [ ] Explorer writes: store a file from the PC side, fetch a variable to
  a file, purge, rename, change directory.
- [ ] Flags panel: toggling a flag (`SF`/`CF`) and re-reading it.
- [ ] Desktop app: native drag and drop between the PC file pane and the
  calculator pane.
- [ ] The control API gains the same write commands (one protocol).

## Acceptance criteria

- [ ] Toggling the clock-display flag from the panel shows the clock on
  the LCD.
- [ ] Dropping a file onto a directory stores it, and fetching a variable
  saves it as a file, each in under a second of wall-clock time at
  unlimited speed, with the calculator screen back afterwards.

## Outcome

(to be written)
