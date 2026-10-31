---
type: backlog
title: Save the 49G's flash apart from the rest of its state
date: 2026-10-31
status: planned
priority: low
tags:
  - backlog
  - autosave
---

# Save the 49G's flash apart from the rest of its state

The second option of [[backlog/save-off-the-machine-thread]], left open
when the first (a writer thread, latest wins) took the desktop app's
write off the machine thread (decision log, 2026-10-31). The 49G's state
is about 2.6 MB, its 2 MB flash inside, and each auto-save still writes
all of it, now on the writer's thread on the desktop and in IndexedDB
in the browser.

## The idea

Save the 2 MB flash only when it changed, under a version, and the rest
(RAM, CPU, the small part) with a pointer to that flash version. Write
the flash first, then the rest; keep the previous flash until the rest
that points at the new one has landed, so a crash between the two leaves
a consistent pair. Most saves then write a few hundred KB. Applies to the
browser's IndexedDB store as well.

## Not solved by it

A reload within 1 s of a change still loses it in the browser: the save
on `pagehide` does not land when the tab's Worker dies with the page, and
the timed save is the one that counts. A shorter delay narrows the window,
it does not close it.
