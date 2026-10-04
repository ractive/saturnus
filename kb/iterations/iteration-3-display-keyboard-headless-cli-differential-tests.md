---
title: "Iteration 3: Display, keyboard, headless CLI, differential tests"
type: iteration
date: 2026-10-04
status: planned
branch: iter-3/display-keyboard-headless-cli-differential-tests
tags:
  - iteration
  - saturnus
---

# Iteration 3: Display, keyboard, headless CLI, differential tests

Read first: wiki `hardware/display`, `hardware/keyboard`,
`questions/display-start-address-taplin`.

## Context from earlier iterations (2026-10-05)

- The CLI crate is `saturnus-cli` with binary `saturnus` (decision log); the
  core crate must stay I/O-free, so ROM loading, PNG and TCP live in the CLI.
- The container's screen dump format is produced by
  `~/devel/hptx/emulator/calc-screen.sh`; match it exactly so the diff script
  can compare text files.
- The disassembler (`saturnus::cpu::disassemble`) is available for a
  `saturnus disasm` subcommand and for trace output when debugging boot.

## Tasks

- [ ] Display controller: row refresh at 4096 Hz ticks, start address, offset,
  line count, menu area, annunciators, contrast (range per
  `questions/contrast-range-48gx`).
- [ ] Full keyboard matrix including ON and the shift keys (Mastracci has them
  swapped; the wiki page is settled).
- [ ] `saturnus-cli`: load ROM, run, press keys from a script, dump the screen
  as text (same 131x64 text form as the container's `calc-screen`) and PNG,
  save/load state.
- [ ] `scripts/diff-vs-saturnng.sh`: same key script on both, compare text
  screens. Three scenarios: boot, `6 7 * ENTER`, a menu walk.

## Acceptance criteria

the three scenarios match pixel for pixel.
