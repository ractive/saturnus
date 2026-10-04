---
title: "Iteration 2: Bus, modules, first ROM boot (48SX)"
type: iteration
date: 2026-10-04
status: planned
branch: iter-2/bus-modules-first-rom-boot-48sx
tags:
  - iteration
  - saturnus
---

# Iteration 2: Bus, modules, first ROM boot (48SX)

Read first: wiki `hardware/memory-controller`, `hardware/io-ram`,
`hardware/interrupts`, `hardware/timers`, `hardware/hp48sx`,
`sources/hpregint-en` (Duchesne), `emulators/emu48`.

## Tasks

- [ ] Memory controller with the 48SX default map (ROM 256 KB, 32 KB RAM at
  #70000, I/O RAM at #100, CE1/CE2 card ports, NCE3 unused).
- [ ] I/O RAM registers with correct reset values; TIMER1 and TIMER2 at
  8192 Hz with the wrap-through-zero interrupt; the interrupt entry sequence
  (#0000F handler, ON key, timer, card and UART sources; INTON/INTOFF per
  `questions/interrupt-maskability`).
- [ ] Enough keyboard (IN/OUT scan) for the ROM's boot checks; display registers
  writable even before the display is drawn.

## Acceptance criteria

the 48SX ROM reaches "Try To Recover Memory?" and, after
  pressing NO, the stack display state in RAM matches what the saturnng
  container shows for the same sequence (compare via framebuffer in M3, via
  RAM dump of the display area in M2). This is the milestone where most
  bugs surface; budget for it.
