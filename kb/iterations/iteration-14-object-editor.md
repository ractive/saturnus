---
title: "Iteration 14: Editing calculator objects in the explorer (RPL editor)"
type: iteration
date: 2026-10-05
status: planned
branch: iter-14/object-editor
tags:
  - iteration
  - saturnus
---

# Iteration 14: Editing calculator objects in the explorer (RPL editor)

Read first: `kb/iterations/iteration-12-memory-explorer.md` (the explorer
layer and its write path), `iteration-13-command-reference.md` (the command
data the editor can use for completion and hover help), wiki
`protocols/kermit-hp` (ASCII transfer: the ROM decompiles an object to text
on send and compiles text on receive; a syntax error aborts the transfer),
`protocols/hp-object-format`, hptx `calculator-quirks.md`.

## Context (2026-10-05)

- Owner: "When we can show the file system of the calculator, we may want
  to make the files directly editable in a nice rich text editor, maybe
  even with syntax highlighting." Yes, for everything the ROM can
  represent as text: programs, algebraics, lists, strings, names, units,
  directories' listings; GROBs as a pixel editor and matrices as a grid
  are separate, simpler editors.
- Mechanism: fetch the object as text through the hidden Kermit path in
  ASCII mode (the ROM's own decompiler produces the canonical source),
  edit in the page, save by sending the text back in ASCII mode (the ROM
  compiles it; "Invalid Syntax" means the save failed, shown in the editor
  with the ROM's error, and the original object is untouched because the
  ROM rejects the transfer). Round trips are sub-second at unlimited speed.
- Editor: a code editor component (CodeMirror 6 is the obvious candidate;
  a plain static page can load it from a CDN or vendor it) with an RPL
  mode: `« »`, `' '`, `" "`, `{ }`, `[ ]`, `:: ;` nesting, command names
  from the iteration 13 data for highlighting, completion and hover help,
  the HP character set (`→`, `Σ`, `∫`, `√`, `π`, `«`, `»`) through a
  palette and keyboard shortcuts, ASCII transliterations (`\\->`, `\\GS`)
  shown as the real characters.
- Model differences: the 49G decompiles with its own spelling (e.g. `«`
  vs `\\<<` headers, flag -dependent formats); the editor stores and sends
  exactly what the ROM gives, with a header line (`%%HP: T(3)A(R)F(.);`)
  preserved and shown as metadata, not as editable text.

- Write path (owner, 2026-10-05, with the retirement of saturnus-mcp):
  `hptx-core` is not compiled into the page or any saturnus host. The
  "hidden Kermit" of this plan is implemented with `kermit-proto` from
  crates.io (protocol only, no serial-port dependency) plus the HP file
  header and text encoding that live in `saturnus-objects`; keystrokes
  are the fallback where no Kermit server exists. Blocked on
  `kermit-proto` being published; the read-only parts are not.

## Tasks

- [ ] Fetch-as-text and save-as-text in the explorer for text-representable
  objects, with the ROM's error surfaced on a failed compile; a dirty
  indicator; save on Cmd/Ctrl+S.
- [ ] An RPL syntax mode for the editor (highlighting, bracket matching,
  indentation of nested `« »` and `IF THEN ELSE END` blocks), the HP
  character palette, transliteration display.
- [ ] Completion and hover help from the command reference data.
- [ ] GROB pixel editor and matrix grid editor (small, separate views).
- [ ] Tests: round trip of a program through fetch, no-op edit, save, on
  the 48SX and 49G; a syntax error surfaces and leaves the object intact.

## Acceptance criteria

- [ ] Opening a program from the explorer shows highlighted RPL; editing
  and saving changes the program on the calculator (`RCL` shows the new
  code); a syntax error is shown in the editor and nothing changes.
