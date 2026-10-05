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

Split 2026-10-05: the research and the read API are [[iterations/iteration-12a-memory-read-api]];
this plan is the read-only UI layer (explorer and flags panel); the write
path is [[iterations/iteration-12b-explorer-writes]] and waits for
`kermit-proto` on crates.io. Iterations 11 and 12a are merged.

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

- Write path (owner, 2026-10-05, with the retirement of saturnus-mcp):
  `hptx-core` is not compiled into the page or any saturnus host. The
  "hidden Kermit" of this plan is implemented with `kermit-proto` from
  crates.io (protocol only, no serial-port dependency) plus the HP file
  header and text encoding that live in `saturnus-objects`; keystrokes
  are the fallback where no Kermit server exists. Blocked on
  `kermit-proto` being published; the read-only parts are not.

## Tasks

- [ ] Protocol: the read commands reserved in `web/protocol.md`
  (`memoryTree`, `stack`, `flags`, `objectAt`) and the `memoryChanged`
  event implemented in the Worker host and the shared runner (Tauri,
  HTTP), fed by `saturnus-objects`; the change counter is polled on the
  machine side, the page is told only when it moved (and at most a few
  times per second). Models without RPL memory (38G, 39G, 40G, 42S)
  answer with the existing refusal and the page hides the layer.
- [ ] Explorer layer in the web page and the desktop app (a toggle; the
  calculator stays usable beside it): the HOME tree with the current
  directory highlighted, the variable list of the selected directory
  (name, type, size, checksum), a typed object preview, copy-to-clipboard
  of an object's text form, the stack as typed levels. A `<sat-explorer>`
  component on the shared store, following hpcomm's two-pane model
  (`~/devel/hpcomm`) where it applies to reading. Layout for desktop
  (side by side) and narrow screens (the layer over the calculator).
- [ ] Flags panel (owner, 2026-10-05: "The UI could also handle all flags
  that you can set"), read-only in this iteration: system flags -1 to -64
  with their meaning per model and user flags 1 to 64, read live, grouped
  by topic with the current value (the 49G's second flag words included).
  The flag meanings are transcribed as facts, in our own words, into the
  wiki first (one page per model family, citing the user's guides), and
  shipped as data generated from those pages.
- [ ] Verification in headless Chrome on the 48SX, 48GX and 49G: create a
  variable and a directory on the calculator, change directory, push
  objects, set a flag by keys: the explorer, the stack view and the flags
  panel follow within a second without any key being sent by the page;
  the calculator's speed and idle behaviour are unchanged with the layer
  open (0 run passes while idle; emulated time equals wall time).

## Acceptance criteria

- [ ] With a 48SX running, the explorer shows the HOME tree, the
  variables, the stack and the flags live without server mode; creating a
  variable on the calculator shows up in the explorer within a second.
- [ ] With the layer open and the calculator idle, the page still sleeps
  (no run passes), and `just gates` passes.

## Outcome

(to be written)
