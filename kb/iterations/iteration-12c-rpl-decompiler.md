---
type: iteration
title: "Iteration 12c: RPL decompiler in saturnus-objects (text of programs, algebraics, units)"
date: 2026-10-05
status: planned
tags:
  - iteration
  - saturnus
branch: iter-12c/rpl-decompiler
---

# Iteration 12c: RPL decompiler in saturnus-objects

Read first: `crates/saturnus-objects/src/{object.rs,ram.rs}`,
`kb/iterations/iteration-12a-memory-read-api.md` (Outcome),
`kb/iterations/iteration-13a-command-data.md` (how command names were
extracted from the ROMs), wiki `protocols/hp-object-format`,
`hardware/hp48-system-ram`, the SDK's RPL documentation in the literature
library (`raw/saturn-hardware/hp48-sdk-1993/{RPLMAN.TXT,RPL.TXT}`).

## Context (2026-10-05)

Found by iteration 12: the read API decodes data objects exactly but has
no text for programs, algebraics, units and ROM commands.

- `Program` and `Algebraic` come back with `source: None`. The text used
  to come from a Kermit ASCII transfer (the ROM decompiles), which needs
  server mode and hptx-core; neither is available to the page.
- A ROM command inside a composite loses its name and is mis-typed: `SIN`
  in `{ 1 SIN }` comes back as `{"type":"program"}`, because the pointer
  is followed into ROM and the target carries a program prolog. Other
  built-ins come back as a `command` without a name.
- Unit objects carry the number but no unit text; XLIB names have no
  text.

The explorer (iteration 12) shows such objects without a text form until
this lands, and uses `source`, `unit` and a command's `name` as soon as
they are present.

## Design

- **Name table from the ROM, at run time.** The mapping from a ROM address
  (and from an XLIB library and command number) to the command's name is
  read from the ROM image the user loaded: the libraries' own name
  tables, as the ROM's decompiler uses them. Nothing ROM-derived is
  committed: no address lists, no name tables. Built once per loaded ROM
  and cached; wasm-clean, no I/O. Research first: where the built-in
  libraries' hash and link tables are on the 48SX (ROM J), 48GX (ROM R)
  and 49G (2.10), written to the wiki with sources (the SDK's RPL
  documents; observation in saturnus), then implemented from the wiki.
- **Pointers into ROM are commands, not objects to follow.** A pointer in
  a composite that targets ROM is rendered as the command it names; only
  pointers into RAM are followed. A ROM address without a name is an
  unnamed command with its address, never a nested decode.
- **Decompiler.** Programs (with the structure words: `IF THEN ELSE END`,
  `START`/`FOR` loops, `DO UNTIL`, `WHILE REPEAT`, `CASE`, local-variable
  blocks `->`), algebraics (infix with the ROM's precedence and
  parenthesisation), unit objects, tagged objects, names (global, local),
  XLIB names, embedded data objects. Output is the text the calculator
  itself shows for the object in its standard mode (number format and
  binary base as the flags say, which `ram.rs` already reads).
- **Oracle.** The ROM's own decompiler is the reference: for a corpus of
  objects, the Kermit ASCII read of each object must equal our text
  (modulo the transfer's character translation, which is known). The
  corpus comes from iteration 13a's generated examples (hundreds of
  programs and algebraics per model) plus hand-written structure cases.
  Where the text differs, ours is wrong.
- **Bounded work.** The decode budget of iteration 12a applies; text
  output is capped with a truncation marker; garbage RAM gives an error or
  an "unknown" element, never a panic.

## Tasks

- [ ] Research and wiki: built-in library name tables per model; how a ROM
  address maps to a library command; XLIB numbering; the structure words'
  internal objects.
- [ ] Name table built from a loaded ROM; unit tests on synthetic tables;
  ROM-gated tests that well-known addresses resolve (the list is in the
  test as names only, compared with the 13a command lists for coverage:
  every command of the model resolves).
- [ ] Decoder: ROM pointers become named commands; `command` entries carry
  `name`; the mis-typing as "program" is gone; shapes documented in
  `web/protocol.md` and the crate docs.
- [ ] Decompiler for programs, algebraics, units, XLIB names; `source` and
  `unit` filled in `stack()`, `object_at()` and the tree's previews.
- [ ] ROM-gated oracle test on the 48SX, 48GX and 49G: our text equals the
  ROM's ASCII form for the corpus; the mismatch list is empty or each
  remaining case is explained in the Outcome.
- [ ] The web bindings, the runner commands and `saturnus ctl stack`
  return the text; the explorer shows it (a check in headless Chrome once
  iteration 12 is merged, or a note for the lead if it is not yet).

- [ ] Follow-up experiment, after the decompiler works (owner, 2026-10-05:
  "Maybe you can decompile the ROM to get the list of commands incl.
  categories?"): the built-in menus are data in the ROM (label and action
  pairs). Find how a menu number leads to its definition and decode the
  definitions with the decompiler, giving each command's menu statically,
  per ROM, without pressing keys. Report feasibility and coverage on the
  48SX; if it works, it becomes the per-ROM source of categories for the
  command reference, with the manuals' placement (iteration 13a) as the
  cross-check. Not part of this iteration's acceptance; a short report in
  the Outcome is enough.

## Acceptance criteria

- [ ] `saturnus ctl stack --json` for `« 1 2 + »`, `'A+1'` and `{ 1 SIN }`
  on the 48SX returns their text and a named `SIN`.
- [ ] The oracle test passes on the three models.
- [ ] `just gates` passes; nothing ROM-derived is committed.

## Outcome

(to be written)
