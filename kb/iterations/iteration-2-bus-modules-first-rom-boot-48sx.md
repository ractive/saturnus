---
title: "Iteration 2: Bus, modules, first ROM boot (48SX)"
type: iteration
date: 2026-10-04
status: completed
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

- [x] Memory controller with the 48SX default map (ROM 256 KB, 32 KB RAM at
#70000, I/O RAM at #100, CE1/CE2 card ports, NCE3 unused).
- [x] I/O RAM registers with correct reset values; TIMER1 and TIMER2 at
  8192 Hz with the wrap-through-zero interrupt; the interrupt entry sequence
  (#0000F handler, ON key, timer, card and UART sources; INTON/INTOFF per
  `questions/interrupt-maskability`).
- [x] Enough keyboard (IN/OUT scan) for the ROM's boot checks; display registers
  writable even before the display is drawn.

## Acceptance criteria

the 48SX ROM reaches "Try To Recover Memory?" and, after
  pressing NO, the stack display state in RAM matches what the saturnng
  container shows for the same sequence (compare via framebuffer in M3, via
  RAM dump of the display area in M2). This is the milestone where most
  bugs surface; budget for it.

## Outcome (2026-10-05)

- `bus/controller.rs`: `MemoryController` (five controllers HDW, NCE2, CE1,
  CE2, NCE3 in daisy-chain order; NCE1/ROM answers the rest; size as mask;
  priority HDW > NCE2 > CE2 > CE1 > NCE3; UNCNFG by priority; C=ID codes).
  `modules/`: `Rom` (packed images, mirrored), `Ram`.
- `io/`: `IoRegisters` (HDW window incl. CRC, display registers, row
  counter), `Timers` (TIMER1 16 Hz, TIMER2 8192 Hz, expiry on the MSB going
  set, SRQ bit computed), `Keyboard` (48 matrix, ON on IN bit 15).
- `machine/`: `Machine` (`Model::Hp48sx`, 2 MHz, time from CPU cycles),
  `Hardware` (the `Bus` impl), `Lcd` (131x64 text dump from display RAM).
  Interrupts: timers and ON are not masked by INTOFF; the 1 ms keyboard
  scan is. SHUTDN skips time to the next timer event.
- CPU change: `Bus::read_data` separates data reads from opcode fetches so
  the CRC follows data reads only.
- The 48SX ROM J configures exactly the documented default map (HDW #00100;
  NCE2 size #F0000 at #70000; CE1/CE2 size #C0000 at #80000/#C0000; NCE3
  size #FF000 at #D0000), reaches "Try To Recover Memory?" after about 40 M
  cycles, and after NO shows "Memory Clear" over the empty stack. Both
  screens are golden files in `crates/saturnus/tests/golden/`, checked by
  `tests/e2e.rs` when `SATURNUS_ROM_DIR` is set. No decoder gaps surfaced.
- Not done: the comparison with the saturnng container. This machine has
  no container runtime; the diff script is iteration 3's task and will
  cover the boot screens.
- Bring-up tool: `cargo run --release -p saturnus --example boot -- <rom>
  [--cycles N] [--keys "f@40000000"] [--trace N] [--watch-pc HEX] [--screen]`.
