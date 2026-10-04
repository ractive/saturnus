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

## Context from iteration 1 (2026-10-05)

- The CPU is done: `saturnus::cpu::{Cpu, Bus, FlatMemory, Registers,
  Instruction, decode, disassemble}`. `Cpu::step(&mut bus) -> Step { cycles,
  event }` executes one instruction; `Cpu::interrupt()` performs the entry
  (push PC, `in_interrupt`, jump to #0000F) and does not check
  `interrupts_enabled`, so the machine filters maskable sources. RTI re-enters
  when `interrupt_pending` was latched. Events: `Shutdown`, `InvalidOpcode`,
  `Rti`.
- The `Bus` trait already has the chip-interface hooks the controller needs:
  `read_nibble`, `write_nibble`, `read_in`, `write_out`, `config(addr)`,
  `unconfig(addr)`, `read_id`, `reset`, `shutdown`, `service_request`,
  `bus_command`, `interrupt_pending` (used by RSI). Implement the memory
  controller as a `Bus` impl; keep `Cpu` unchanged unless a fact forces it.
- Cycle counts in `Step` are approximate (SASM manual table); drive TIMER1/2
  from them for now and note the error.
- An `InvalidOpcode` event during the ROM boot almost certainly means a
  decoder gap, not ROM behaviour: stop and fix the decoder, do not paper over
  it.
- No ROM is on this machine. The 48SX ROM J is
  `https://www.hpcalc.org/hp48/pc/emulators/sxrom-j.zip` (file `sxrom-j`,
  262144 bytes, one byte per two nibbles; the hptx container uses the same
  URL). Download with curl's default user agent into `roms/` (gitignored),
  verify the size, and point `SATURNUS_ROM_DIR` at it; never commit it. The
  e2e test binary boots only when that variable is set (test policy).
- Clean room still applies: `~/devel/hp-emulator-refs/` is off limits; the
  wiki page `emulators/emu48` holds the facts already extracted from Emu48's
  documentation.

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
