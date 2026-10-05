---
title: "Iteration 13a: Command reference data (ROM catalog, generated examples, descriptions)"
type: iteration
date: 2026-10-05
status: planned
branch: iter-13a/command-data
tags:
  - iteration
  - saturnus
---

# Iteration 13a: Command reference data (ROM catalog, generated examples, descriptions)

The data half of [[iterations/iteration-13-command-reference]]; read that
plan's Context first (copyright position: names, categories and stack
effects are facts; HP's descriptions are not copied or closely paraphrased;
manuals are fact inputs and link targets; examples are generated on the
emulator). The UI half stays in iteration 13 and waits for iteration 11.

Read first: `crates/saturnus-mcp/src/{semantic.rs,object.rs,emulator.rs}`
(`eval`, typed objects, limits), `README.md` ("MCP server"), the manuals in
`~/devel/hp-literature/raw/manuals/` (`hp48gug.pdf`, `hp48gaur.pdf` the
Advanced User's Reference, `hp49g-um-en.pdf`, `hp49g-aug-en.pdf`; the 48SX
owner's manual online at literature.hpcalc.org), `kb/docs/clean-room-rule.md`.

## Tasks

- [ ] Extract the command name list per model (48SX, 48GX, 49G) from the
  ROM itself through the emulator (the catalog, or the ROM's name tables),
  with the menu category where the ROM has one; store as data
  (`data/commands/<model>.json`); a test regenerates and compares.
- [ ] A generator binary (`crates/saturnus-refgen` or an example) that
  runs curated inputs per command through `eval` on each model and writes
  examples (input stack, source, typed result, error text) as JSON;
  deterministic and re-runnable; commands that need interaction, plots or
  I/O are marked and skipped with a reason.
- [ ] Curated inputs and our own one- or two-sentence descriptions per
  command for the 48SX set first (then the GX and 49G additions), with the
  stack effect derived from the runs and checked against the manual as a
  fact source; a script flags any description that shares long word
  sequences with the manuals' text layer.
- [ ] A page index of the Advanced User's Reference and the user's guides
  (command heading to PDF page, from `pdftotext`), stored with each entry as
  a deep link (`<public URL>#page=N`); the public URLs recorded per manual.
- [ ] `saturnus-mcp`: the reference as an MCP resource and a `help
  {command}` tool.

## Acceptance criteria

- [ ] Every command in the 48SX ROM's catalog has an entry with a category,
  our description, a stack effect and at least one generated example or a
  stated reason why not; the generator reproduces its output byte for byte.
