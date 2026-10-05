---
type: iteration
title: "Iteration 12c: RPL decompiler in saturnus-objects (text of programs, algebraics, units)"
date: 2026-10-05
status: completed
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

- [x] Research and wiki: built-in library name tables per model; how a ROM
  address maps to a library command; XLIB numbering; the structure words'
  internal objects.
- [x] Name table built from a loaded ROM; unit tests on synthetic tables;
  ROM-gated tests that well-known addresses resolve (the list is in the
  test as names only, compared with the 13a command lists for coverage:
  every command of the model resolves).
- [x] Decoder: ROM pointers become named commands; `command` entries carry
  `name`; the mis-typing as "program" is gone; shapes documented in
  `web/protocol.md` and the crate docs.
- [x] Decompiler for programs, algebraics, units, XLIB names; `source` and
  `unit` filled in `stack()`, `object_at()` and the tree's previews.
- [x] ROM-gated oracle test on the 48SX, 48GX and 49G: our text equals the
  ROM's ASCII form for the corpus; the mismatch list is empty or each
  remaining case is explained in the Outcome.
- [x] The web bindings, the runner commands and `saturnus ctl stack`
  return the text; the explorer shows it (a check in headless Chrome once
  iteration 12 is merged, or a note for the lead if it is not yet).

- [x] Follow-up experiment, after the decompiler works (owner, 2026-10-05:
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

- [x] `saturnus ctl stack --json` for `« 1 2 + »`, `'A+1'` and `{ 1 SIN }`
  on the 48SX returns their text and a named `SIN`.
- [x] The oracle test passes on the three models.
- [x] `just gates` passes; nothing ROM-derived is committed.

## Outcome

- **Name tables** (wiki: protocols/rpl-libraries, sources RPLMAN p. 13
  and 19-20, MAKEROM p. 1-2, layouts observed in the ROM images): every
  library starts with a 23-nibble header (number, offsets to hash,
  message and link tables and the configuration object); the hash table
  groups names by length and ends in a number table pointing back at each
  command's name; the link table points at each command's object, which
  is preceded by its 6-nibble XLIB body. `names::NameTable` finds the
  libraries by scanning the image (the 49G's flash banks included),
  needs no address list, and resolves a ROM pointer the ROM's way
  (prefix, then the link table must point back). 48SX J: 2 libraries, 410
  names; 48GX R: 42 (3 with names), 539; 49G (`rom.49g`): 49 (13 with
  names), 852. Every name of 13a's catalogs (410 / 531 / 852 XLIB
  numbers) resolves. Build: 4 / 7 / 23 ms native release, 8 / 10 / 33 ms
  in wasm (node), 16 / 60 / 90 KiB; built once per emulator on the first
  memory read.
- **Decoder**: ROM pointers to programs, code and primitives are
  commands (`{"type":"command","name":"SIN","address":111788}`), never
  followed; without a table they keep their address and no name. XLIB
  names are commands with their name (or `XLIB l c`). Argument counts
  come from the commands' CK0-CK4 dispatchers, unit operators from the
  ROM's own unit objects (both found at run time). The 49G's symbolic
  matrices decode as `array`. Shapes in `web/protocol.md`.
- **Decompiler** (`decompile.rs`): programs with every structure word,
  locals, quoted names, embedded programs; algebraics in infix with the
  ROM's binding and parentheses (`∂`, `Σ`, `|`, `NOT`, `√`, `!`, user
  functions, flag -53); units with prefixes and compound units; tagged,
  lists, arrays, strings, complex, binaries; numbers in STD, FIX, SCI and
  ENG with the fraction mark, the base and word size, the 49G's real
  points and digit grouping, the 48's left- and the 49G's right-grouping
  `^`. Output capped at 65536 characters with `…`; garbage gives an
  error or no text (unit tests with a self-referencing list, loose
  algebraics, a self-pointing program, deep trees).
- **Oracle** (`decompiler_matches_the_rom`, saturnus-mcp e2e): 382 / 389
  / 392 cases on the 48SX / 48GX / 49G at `SATURNUS_ORACLE_SCALE=20`
  (release, 55 s), no mismatch; the default debug run keeps the
  generated part small. A one-off run of the 828 sources from 13a's
  example files (149 / 181 / 498) also matched completely. The ASCII
  transfer's differences from the display (header, line breaks, unit
  quotes, the 49G's tag colon) are normalised in one documented function
  (wiki: protocols/hp-object-format); display modes are compared with
  the server's stack display, of which the 49G sends only 20 characters
  a level.
- **Acceptance**: `saturnus ctl stack --json` on the 48SX (state with the
  three objects, `run --serve --control 4893`) returned
  `[{"source":"« 1 2 + »","type":"program"},{"source":"'A+1'","type":"algebraic"},{"items":[{"type":"real","value":1.0},{"address":111788,"name":"SIN","type":"command"}],"type":"list"}]`.
- **Hosts**: the web bindings (`stack`, `object_at`, so the Worker, the
  runner, the control API and `ctl`) use the table; `object_at` now also
  sets the binary base. The MCP's own RAM reads are unchanged (no new
  public surface); its tests drive the decompiler directly.
- **Not covered**: the explorer check in headless Chrome (iteration 12 is
  not merged; note for the lead: the page can show `source`, `unit` and
  `name` as they are); the 49G's algebraic mode (-95 set; only RPN was
  tested); a top-level tagged object compared only through the stack
  display (STO strips tags); graphics, libraries, backups, directories
  and code objects keep the existing `unknown` shape (shown by their
  type name); the 48's FIX digit grouping right after a mode change in
  the same command line (the server's display lagged once, not
  reproduced in steady state); other ROM revisions.

### Follow-up experiment: menus read statically from the ROM (48SX)

Done as scratch work after the main tasks; no code from it is in this
iteration. Feasible: after `n MENU`, RAM #7061E holds the address of the
current menu's definition, and the definitions of menus 1 to 59 are the
elements of one list in ROM J at #3B234 (element n is menu n; menus 2 and
24 are pointers to definitions elsewhere), referenced from `MENU`'s own
code, so it can be located statically. A definition is a list of keys, or
a program building one; a key is a command, a `{ label action }` pair or
a unit-name string. Decoding the 59 definitions with the name table and
the decompiler places 334 of the 397 catalog commands in a menu, against
354 from iteration 13a's key-pressing crawl, with 315 in both. The 39
only the crawl had are keyboard functions that no numbered menu contains;
the 19 only the static decode has are entries the crawl missed (the plot
types, the STAT model fits, `ELSE`, `FOR`, `STEP` and others); and the
decode showed crawl errors where labels that are not commands (unit names,
a truncated `ATAN` for `ATANH`) had been matched to commands. Not done:
the 48GX and 49G, a generic way to locate the list, keyboard placement.
Facts: wiki `protocols/rpl-libraries`, "Built-in menus". Decision: the
decision log's "command palette" entry (categories).
