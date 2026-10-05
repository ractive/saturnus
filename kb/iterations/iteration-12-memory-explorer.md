---
title: "Iteration 12: Memory explorer layer (variables, directories, transfers)"
type: iteration
date: 2026-10-05
status: planned
branch: iter-12/memory-explorer
tags:
  - iteration
  - saturnus
---

# Iteration 12: Memory explorer layer (variables, directories, transfers)

Read first: `crates/saturnus-mcp/src/object.rs` (the exact object decoder
from iteration 9), wiki `protocols/hp-object-format`, `hardware/hp48sx`
(system RAM addresses known so far: KeyBuf #704EA, OUT shadow #704C3,
display ghost registers), the SDK's RPL documentation
(`~/devel/hp-literature/raw/saturn-hardware/hp48-sdk-1993/{RPLMAN.TXT,RPL.TXT}`)
and HP's entry tables for the directory structures, `kb/decision-log.md`
(iteration 9: server-mode cost), `~/devel/hpcomm` (the owner's connectivity
kit; the two-pane PC/calculator explorer is the interaction model).

## Context (2026-10-05)

- Owner: besides the pure calculator view, "another layer on top that
  shows you some infos like all variables, or the current folder, maybe
  you can even navigate the folders outside the calculator", modern
  version of the HP Connectivity Kit / hpcomm two-pane UI. Asked whether
  this needs server mode or whether the emulator has a side channel.
- Design agreed: **reads** go straight to RAM (the emulator owns it): the
  directory tree from HOME, variables with name, type, size, checksum, the
  current path, the stack; decoded with the iteration 9 decoder; no mode
  switch, instant, refreshed when the directory area changes; available to
  the web page through the wasm bindings and to Tauri alike. **Writes**
  (store, purge, rename, send a file) go through a hidden Kermit
  transaction at unlimited speed (about 15 s emulated per round trip is
  about 0.2 s wall-clock), so the ROM's memory manager stays consistent;
  a later refinement may inject key codes into the ROM's key buffer for
  command-line operations. Direct RAM writes are out.
- Open research: the per-model locations of the directory roots and the
  current-path pointer in system RAM (48SX, 48GX, 49G; the 38G/39G/40G
  have aplets, not a HOME tree, treat them later), and how to detect a
  change cheaply (hash of the directory region or a ROM write hook).

## Tasks

- [ ] Research and record in the wiki: system RAM pointers to HOME, the
  current directory and the stack for the 48SX, 48GX and 49G (from the SDK
  documentation and ROM tracing in saturnus), with tests that walk a
  known directory tree.
- [ ] Core or `saturnus-web` API: `memory_tree()` (directories and
  variables with name, type, size, checksum), `current_path()`,
  `stack_objects()`, decoded exactly; a change counter so the UI refreshes
  only when needed.
- [ ] Explorer layer in the web page (toggle): calculator on one side, the
  HOME tree and the variable list on the other, current folder
  highlighted, object preview (typed), copy-to-clipboard of an object's
  source.
- [ ] Writes through hidden Kermit at unlimited speed: store a file from
  the PC side, fetch a variable to a file, purge, rename, cd; a busy
  overlay while the transaction runs; the screen restored afterwards.
- [ ] Tauri: the same layer with native drag and drop between the PC file
  pane and the calculator pane.

## Acceptance criteria

- [ ] With a 48SX running, the explorer shows the HOME tree and variables
  live without server mode; creating a variable on the calculator shows up
  in the explorer within a second.
- [ ] Dropping a file onto a directory stores it, and fetching a variable
  saves it as a file, each in under a second of wall-clock time at
  unlimited speed, with the calculator screen back afterwards.
