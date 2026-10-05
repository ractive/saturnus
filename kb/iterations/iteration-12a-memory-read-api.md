---
title: "Iteration 12a: Memory read API (directories, variables, stack, flags from RAM)"
type: iteration
date: 2026-10-05
status: planned
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

- [ ] Research and record in the wiki (`hardware/hp48-system-ram` or per
  model pages, with sources): where the ROM keeps the pointers to the HOME
  directory, the current directory (context), the data stack, and the two
  64-bit flag words (system and user flags), for the 48SX, 48GX and 49G;
  the directory object layout (name/object chain, sub-directories); found
  from the SDK documentation and by tracing the ROM in saturnus (e.g.
  watch the RAM reads of `VARS`, `PATH`, `DEPTH`, `RCLF`).
- [ ] Move the object decoder into a crate usable by the core's hosts
  without the MCP stack (e.g. `crates/saturnus-objects`, no_std-friendly,
  wasm-clean), keeping `saturnus-mcp` on it.
- [ ] A read API over a paused `Machine` (in the new crate or
  `saturnus-drive`): `memory_tree()` (directories and variables with name,
  type, size in bytes, checksum as the ROM's `BYTES` reports it),
  `current_path()`, `stack_objects()`, `flags()`; no writes, no mode
  switch; a cheap change counter (hash of the directory region and the
  stack pointer) so a UI can refresh only when needed.
- [ ] Tests, ROM-gated: build a known directory tree, stack and flags
  through the MCP `eval` path, then compare the RAM-read results with what
  Kermit `list_vars`/`stack`/`RCLF` report, on the 48SX, 48GX and 49G.
- [ ] Expose the read API in `saturnus-web` (wasm bindings returning JSON)
  and in `saturnus-mcp` (`memory_tree`, `flags` tools that work without
  server mode); document both.

## Acceptance criteria

- [ ] On the 48SX, 48GX and 49G the RAM-read directory tree, stack and
  flags equal the Kermit-read ones in the tests, without entering server
  mode, and the change counter moves when a variable is stored.
