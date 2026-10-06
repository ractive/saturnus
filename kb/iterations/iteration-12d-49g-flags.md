---
type: iteration
title: "Iteration 12d: The 49G's system flags from the calculator's own list"
date: 2026-10-06
status: planned
tags:
  - iteration
  - saturnus
branch: iter-12d/49g-flags
---

# Iteration 12d: The 49G's system flags from the calculator's own list

The 49G's Advanced User's Guide refers to the HP 49G Pocket Guide for
the list of system flags, and the library has no copy (iteration 12,
`questions/hp49g-system-flags` in the wiki). Owner (2026-10-06): do the
fallback. If the Pocket Guide turns up later, its table replaces this.

Read first: wiki `hardware/system-flags-49g` and
`questions/hp49g-system-flags`, `scripts/flags-json.py`, `web/flags.json`,
`crates/saturnus-objects/src/ram.rs` (flag words), the typing verbs and
`commandLine` of iteration 19 (`web/protocol.md`), `saturnus ctl`.

## Design

- The 49G's MODE → FLAGS browser lists each system flag with a one-line
  description whose wording changes with the flag's state (set or clear).
  Drive the emulator through `saturnus run --serve` and `saturnus ctl`:
  open the browser, walk the list, read the two wordings per flag from
  the screen (or from the ROM's message strings through the name table
  and decompiler of iteration 12c, if they are addressable; prefer that,
  it avoids OCR), toggle through `SF`/`CF` between passes.
- Each description is restated in our own words on the wiki page as a
  fact ("observed on ROM 2.10, MODE FLAGS browser"), with the two states
  and the flag number; flags the browser does not list (those on input
  forms, -95 among them, per the guide) stay unknown unless the user's
  manual or the guide names them.
- `scripts/flags-json.py` regenerates `web/flags.json`; the panel's
  basis line for the 49G names the source.
- The extraction is a script in `scripts/` (or a refgen step) that can
  be rerun on another ROM revision; nothing ROM-derived beyond flag
  numbers and our wording is committed.

## Tasks

- [ ] Find where the browser's strings live (message table) or drive the
  browser and read the screen; decide and record on the wiki.
- [ ] Extract both wordings for every listed flag on ROM 2.10; count what
  the browser lists and what it does not.
- [ ] Rewrite `hardware/system-flags-49g`; regenerate `web/flags.json`;
  the panel's basis line.
- [ ] Verification in the browser: the 49G flags panel shows the meanings;
  setting a flag by keys updates it.

## Acceptance criteria

- [ ] The 49G panel shows a meaning for every flag the calculator's own
  browser lists; the remaining unknowns are named with the reason.
- [ ] `just gates` passes; `hyalo lint` clean in the kb and the wiki.

## Outcome

(to be written)
