---
type: backlog
title: Save the state off the machine thread
date: 2026-10-25
status: completed
priority: low
tags:
  - backlog
  - autosave
---

# Save the state off the machine thread

Since 2026-10-25 the calculator auto-saves 1 s after the last change, not
5 s (decision log, 2026-10-25). On the desktop app the save is
`StateStore::save` (`StateDir::save`): a synchronous, fsynced write on the
runner thread in `crates/saturnus-drive/src/runner.rs`, the thread that
runs the machine. The 49G's state is about 2.6 MB (its 2 MB flash inside),
so it is written more often now, and a key pressed during the write waits
until it ends. Accepted for now; this item takes the write off that thread.

## Options

- **A writer thread, latest wins.** The runner hands the state to a writer
  thread in `saturnus-drive`'s runner and goes on; the writer keeps only
  the newest state it was given, so a save that comes while one is being
  written replaces the queued one instead of piling up. A boot, a restore
  or a fresh start waits for (or cancels) what is queued, so an old save is
  never written back over a new machine.
- **Split the 49G's flash from the rest.** Save the 2 MB flash only when
  it changed, under a version, and the rest (RAM, CPU, the small part)
  with a pointer to that flash version. Write the flash first, then the
  rest; keep the previous flash until the rest that points at the new one
  has landed, so a crash between the two leaves a consistent pair. Most
  saves then write a few hundred KB. Applies to the browser's IndexedDB
  store as well.

## Not solved by either

A reload within 1 s of a change still loses it in the browser: the save
on `pagehide` does not land when the tab's Worker dies with the page, and
the timed save is the one that counts. A shorter delay narrows the window,
it does not close it.

## Outcome

Done with the first option (decision log, 2026-10-31): the runner hands
each auto-save to `SaveWriter` (`crates/saturnus-drive/src/save_writer.rs`),
a thread of its own that keeps only the newest queued state per model; a
boot's read or delete of the slot, the page hidden and the machine thread
ending wait for what is queued. The second option is
[[backlog/save-49g-flash-apart]].
