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
- Iteration 2 already landed much of the display and keyboard plumbing:
  `io/registers.rs` (DON, bit offset, contrast, DISP1CTL, LINENIBS,
  LINECOUNT, DISP2CTL, annunciator nibbles), `io/keyboard.rs` (`Key` enum
  for all 49 keys with `matrix()`, `name()`, `from_name()`; ON on IN bit
  15), `machine/lcd.rs` (`Lcd::render` from the bitmaps, `to_text` with
  `#`/`.`), `machine/mod.rs` (`Machine::{new, reset, step, run_cycles,
  key_down, key_up, peek, lcd}`), and `examples/boot.rs` as a bring-up tool.
  What is missing for this iteration: the row counter running at 4096 Hz
  with its readback, the refresh stall, a framebuffer type that carries the
  annunciators and contrast, the card-detect path, and save/load state.
- The iteration 2 child reported "no container runtime", which was wrong:
  Docker is Rancher Desktop at `~/.rd/bin/docker` (put `~/.rd/bin` on PATH).
  The oracle image is built from `~/devel/hptx/emulator/` (`docker build -t
  hp49g-emu .`, downloads the ROMs itself), run with `-e MODEL=48sx`, and
  driven with `docker exec <name> calc-keys ...` / `calc-screen` (TUI pixel
  character is a block, one char per pixel; trailing blanks stripped). Its
  README documents the key map. Normalise both sides to the same text form
  before diffing.
- The ROM only sees a key after about 10 ms of debouncing (five identical
  2 ms samples), so a scripted key press must be held for tens of
  milliseconds of emulated time; the container waits for the LCD to settle
  between keys.
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

- [ ] Carried over from iteration 2: the boot scenario of the diff script
  also covers iteration 2's open acceptance criterion ("Try To Recover
  Memory?", NO, "Memory Clear" over the empty stack must match saturnng).
- [ ] Carried over from iteration 2: card-detect interrupt source and card
  status register (#10F); today the ports are always empty and #10F reads 0.
- [ ] Carried over from iteration 2: check the inferred hardware behaviour
  against saturnng and record the results in the wiki: the open-bus value for empty CE1/CE2/NCE3
  (now 0), SHUTDN with OUT = 0 (the SASM "cold start" case, not modelled),
  CE2-above-CE1 priority (`bus-priority-ce1-ce2` in
  `docs/open-hardware-questions`).

## Acceptance criteria

the three scenarios match pixel for pixel.
