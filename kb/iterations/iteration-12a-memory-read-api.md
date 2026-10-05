---
title: "Iteration 12a: Memory read API (directories, variables, stack, flags from RAM)"
type: iteration
date: 2026-10-05
status: completed
branch: iter-12a/memory-read-api
tags:
  - iteration
  - saturnus
---

# Iteration 12a: Memory read API (directories, variables, stack, flags from RAM)

The data half of [[iterations/iteration-12-memory-explorer]]; read that
plan's Context first (the agreed design: reads straight from RAM, writes
through hidden Kermit, hpcomm's patterns). The UI half stays in iteration 12
and waits for the web restructuring of iteration 11.

Read first: `crates/saturnus-mcp/src/object.rs` (the exact object decoder
of iteration 9) and `semantic.rs` (`list_vars`, `stack` through Kermit:
the oracle for this work), wiki `protocols/hp-object-format`, `hardware/hp48sx`,
the SDK's RPL documentation in
`~/devel/hp-literature/raw/saturn-hardware/hp48-sdk-1993/{RPLMAN.TXT,RPL.TXT,RPL2.TXT,RPL3.TXT}`
and the entry lists in `raw/saturn-hardware/hp48-hw-notes/mlstarterkit/`,
`kb/docs/clean-room-rule.md`.

## Tasks

- [x] Research and record in the wiki (`hardware/hp48-system-ram` or per
  model pages, with sources): where the ROM keeps the pointers to the HOME
  directory, the current directory (context), the data stack, and the two
  64-bit flag words (system and user flags), for the 48SX, 48GX and 49G;
  the directory object layout (name/object chain, sub-directories); found
  from the SDK documentation and by tracing the ROM in saturnus (e.g.
  watch the RAM reads of `VARS`, `PATH`, `DEPTH`, `RCLF`).
- [x] Move the object decoder into a crate usable by the core's hosts
  without the MCP stack (e.g. `crates/saturnus-objects`, no_std-friendly,
  wasm-clean), keeping `saturnus-mcp` on it.
- [x] A read API over a paused `Machine` (in the new crate or
  `saturnus-drive`): `memory_tree()` (directories and variables with name,
  type, size in bytes, checksum as the ROM's `BYTES` reports it),
  `current_path()`, `stack_objects()`, `flags()`; no writes, no mode
  switch; a cheap change counter (hash of the directory region and the
  stack pointer) so a UI can refresh only when needed.
- [x] Tests, ROM-gated: build a known directory tree, stack and flags
  through the MCP `eval` path, then compare the RAM-read results with what
  Kermit `list_vars`/`stack`/`RCLF` report, on the 48SX, 48GX and 49G.
- [x] Expose the read API in `saturnus-web` (wasm bindings returning JSON)
  and in `saturnus-mcp` (`memory_tree`, `flags` tools that work without
  server mode); document both.

## Acceptance criteria

- [x] On the 48SX, 48GX and 49G the RAM-read directory tree, stack and
  flags equal the Kermit-read ones in the tests, without entering server
  mode, and the change counter moves when a variable is stored.

## Outcome

- Locations (wiki: hardware/hp48-system-ram, new source page
  sources/hp48-internals-address-list): HOME, end of HOME, current
  directory, saved D1 and stack end at #70592, #70597, #7059C, #70579
  and #7057E on the 48SX, and at #80711, #80716, #8071B, #806F8 and,
  for the stack end, #806FD on the 48GX and 49G; flags at #706C5
  and #706D5 (48SX), #80843 and #80853 (48GX), #80F02 and #80F22 with two
  words each (49G). The 48SX ones are
  in the 1991 address list and hold on ROM J; the 48GX and 49G ones were
  found in saturnus (RAM scans, SF/CF diffs, a trace of the ROM's D1
  restore). HOME's header carries a library count with 13 nibbles per
  library before the usual offset field; BYTES's size is the variable
  record over 2, its checksum the CRC of the object.
- `crates/saturnus-objects`: the decoder and object model (moved from
  `saturnus-mcp`, which re-exports them) and `ram::UserMemory` with
  `tree`, `current_path`, `stack`, `flags`, `change_counter`; builds for
  wasm32.
- MCP tools `memory_tree` and `flags` (no server mode, the calculator
  does not run); wasm `memory_tree()`, `stack()`, `flags()`,
  `object_at()`, `memory_changes()`. README and web/README document them.
- `ram_reads_match_kermit` (MCP e2e) passes on the 48SX, 48GX and 49G:
  tree with sizes and checksums in every directory, path, typed stack and
  RCLF words equal the Kermit reads with the server stopped; a `STO`
  moves the counter, idling does not.
- Open: other ROM revisions unchecked (48SX A-E, 48GX K-P, 49G 1.19-6 and
  2.15); the 49G's algebraic-mode stack (history entries next to the
  results) is shown raw; ports and attached libraries are not read;
  `start_server` fails on the 48SX with the clock display (-40) on (seen
  while writing the test, not investigated).
