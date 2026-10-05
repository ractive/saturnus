---
title: "Iteration 5: More models"
type: iteration
date: 2026-10-04
status: planned
branch: iter-5/more-models
tags:
  - iteration
  - saturnus
---

# Iteration 5: More models

Read first: wiki `hardware/hp48gx`, `hardware/hp49g`, `hardware/memory-controller`
(bank switching, DA19, Giesselink controller model), `hardware/keyboard`
(49G matrix), `questions/hp49g-bank-latch-bits`, `hp49g-flash-write`,
`hp49g-ram-controllers`, `da19-polarity`, `emulators/emu48` (facts only).

## Context from iterations 1-4 (2026-10-05)

- Today `Machine`/`Hardware` are 48SX-specific: `Model` has one variant,
  `Hardware::new(rom, ram)` wires NCE2 RAM, CE1/CE2 card slots and the HDW
  window; `io/registers.rs` is the HDW window incl. UART, timers, cards;
  `state.rs` saves it all (format version 1, unshipped, may change freely).
  The CLI (`saturnus run --model`, `rom fetch`) and `scripts/diff-vs-saturnng.sh`
  (scenario `config` files; the oracle container takes `MODEL=48sx|48gx|49g`)
  know only the SX.
- ROMs: 48GX ROM R is `https://www.hpcalc.org/hp48/pc/emulators/gxrom-r.zip`
  (file `gxrom-r`, 524288 bytes packed); 49G ROM 2.15 is
  `https://www.hpcalc.org/hp49/pc/rom/hp4950emurom.zip` (member `rom.49g`,
  2097152 bytes, the whole 2 MB flash image packed two nibbles per byte).
  Same curl default user agent rule; `roms/` is gitignored; record SHA-256s
  in `rom fetch`. The hptx container has all three models for the oracle.
- Yorke facts the GX needs (wiki: hardware/hp48gx, memory-controller):
  4 MHz; 512 KB ROM with DA19 (#129 bit 3) switching upper ROM (DA19=1) vs
  port 2 (DA19=0, lower ROM mirrored at #80000); 128 KB RAM at #80000; CE1
  is the bank latch (74HC174) usually at #7F000, a byte read at
  `base+#40+2n` latches bank n (A1-A5) and BEN (A6); CE2 = port 1, NCE3 =
  port 2 banks; `#11F` reads 8 on G/GX; empty slots configured as 2 KB at
  #7E000; a GX without cards shows ROM at #C0000-#FFFFF. Keyboard, display,
  timers and UART are the SX layout.
- 49G facts (wiki: hardware/hp49g): Yorke at 4 MHz; NCE1 = 2 MB flash in 16
  banks of 128 KB, two views through the CE1 latch (Giesselink: A1-A2 pick
  the #00000-#3FFFF bank 0-3, A3-A6 the #40000-#7FFFF bank 0-15); NCE2 =
  256 KB RAM at #80000 (HOME/port 0); CE2 and NCE3 = 128 KB each (port 1),
  unconfigured by default; flash writes go through NCE3 at #40000 with
  #11C bit 3 set, using the Intel 28F160S5 command set (public datasheet,
  not in raw/); different keyboard matrix (wiki: hardware/keyboard "HP49G
  matrix"); the ROM asks "Try To Recover Memory?" then shows "Memory Clear"
  with an OK softkey.
- 38G/39G/40G: the wiki pages are stubs. Research first (HP Journal 38G
  articles in `raw/saturn-hardware/hp-journal/hpj-38g/`, Emu48 KML model
  letters `A`/`6`/`E` in `wiki/emulators/emu48`), then decide whether a
  boot is feasible in this iteration; ROM availability on hpcalc.org is
  unchecked.
- Work split: one agent generalises the machine for a second model and
  lands the 48GX end to end; one builds the 49G's standalone parts (flash
  chip model, 49G keyboard table) and wires the 49G once the generalised
  machine exists; one does the 38G/39G/40G research in the wiki.

## Tasks

- [ ] 48GX: 128 KB RAM, bank-switched port 2 with the byte-read latch quirk,
  DA19 polarity (wiki settled it against Mastracci and Voyage).
- [ ] 49G: 512 KB RAM, 2 MB flash with banking and the write-enable path
  (`questions/hp49g-bank-latch-bits`, `hp49g-flash-write` to resolve by
  experiment), different keyboard.
- [ ] 38G, 39G/40G: research first (HP Journal 38G articles in raw/, Emu48
  documentation facts, ROM availability); fill the wiki pages; boot only if
  a ROM and the memory map are in hand, otherwise plan a follow-up.

## Acceptance criteria

- [ ] 48GX: boots to the stack, differential screens vs saturnng match,
  hptx e2e passes over TCP.
- [ ] 49G: boots to the stack, differential screens vs saturnng match,
  hptx e2e passes over TCP.
- [ ] 38G/39G/40G: wiki pages filled from sources; boot if feasible.
