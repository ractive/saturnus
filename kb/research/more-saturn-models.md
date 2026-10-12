---
title: "More Saturn models: what else saturnus could emulate"
type: research
date: 2026-10-12
status: active
tags:
  - research
  - saturnus
  - models
  - rom
---

# More Saturn models: what else saturnus could emulate

Owner's question: which further Saturn-based HP calculators could
saturnus emulate, how easy is it to get their ROMs, and how hard is each
to add. The facts, with citations, are on the wiki page
`hardware/saturn-models` (calculator-knowledgebase PR 9); this note
ranks the candidates and recommends.

## The candidate list, checked

Saturn hardware not yet in saturnus: **71B** (1984); **18C** and
**28C** (1986); the Lewis models **28S**, **19B**, **19BII**, **17B**,
**17BII** and **27S** (1988-1990); the Sacajawea models **14B**, **22S**,
**32S** and **32SII**; and the Bert models **10B**, **20S** and **21S**.
The owner's list was complete. Wikipedia also names an "HP-44A" on the
1LF2, but no other source mentions it and no emulator or ROM is known, so
it is dropped.

Dropped as not Saturn hardware: the 39g+ (and the 39gs, 40gs, 48gII,
49g+ and 50g), which run an emulated Yorke on an ARM920T; the later
17bII+ (Sunplus), 10BII (Hitachi H8) and 10bII+ (ARM). Skipped as
variants: 48S, 48G, 48G+ and 38G+.

## What saturnus already has that applies

- The Saturn CPU applies to every model. The 71B's early 1LF2 lacks
  some later instructions (PC=(A) came with the RPL machines, wiki:
  sources/hpmuseum-forum F2). This would matter only if we restricted
  the instruction set, which we would not.
- The Lewis work from the 42S (decision log, iteration 15):
  `HardwareProfile::lewis`, `io::lewis::LewisIo`, the 131x16 LCD
  (`Lcd::render_lewis`), the 8192 Hz timers and the CRC. The 17B, 17BII
  and 27S use the same board as the 42S: ROM at #00000, display at
  #40000, 8 KB RAM at #50000. The 28S and 19B/19BII use two Lewis chips,
  master and slave, at other addresses: on the 28S, RAM is at #C0000 and
  the display at #FF840 and #FFC00. The 42S runs on a fixed map. How a
  Lewis moves its windows is the open wiki question
  `questions/lewis-memory-map`.
- The 48's daisy-chain memory controller. The 71B's memory chips are
  soft-configured the same way: each answers an ID with its type and
  size, and the OS assigns base addresses. The 48 inherited this scheme.
- Nothing for HP-IL, the card reader, segment displays or the IR
  printer: saturnus has no 82240 printer output. None of these models has
  a serial port or Kermit, so satx does not apply.

Every model also needs the per-model plumbing the 42S needed: a `Model`
variant, keys, layout and skin, ROM identification, a state model code,
and gating in the CLI, web, desktop and MCP. Skins are a real cost.
saturnus's skins come from the owner's photographs, and the owner owns
none of these models.

## Ranking by complexity

Estimates are in saturnus iterations (one PR each). They include the
plumbing above and a drawn skin instead of a photographed one.

| Rank | Model | Chip | ROM class | Complexity | Estimate | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 27S | Lewis (one) | b, c | Low: the 42S's board with another keyboard | 1 (with 17B/17BII) | Same 37-key Pioneer case and 131x16 LCD; dump over IR into an HP 48 |
| 2 | 17BII, 17B | Lewis (one) | b, c | Low, as the 27S. The international ROM is 96 KB (a second ROM at #20000), and there is a clock with alarm | Same iteration as the 27S, +0.5 for the international ROM | English 64 KB, international 96 KB |
| 3 | 28S | two Lewis | b, c | Medium-high: master and slave chips, configurable windows (undocumented), a 137x32 LCD split across the chips, a clamshell keyboard | 2-3 | The best known of these; settles `lewis-memory-map`; dump over IR into an HP 48 (about 133 KB free, or in pieces) |
| 4 | 19BII | two Lewis | b | Low after the 28S: same hardware, other keyboard | +1 after the 28S | No dump method without decapping (no memory scanner). Images exist only through the Emu42 author, who got them from people at HP |
| 5 | 19B | two Lewis | d | As the 19BII | +0.5 after the 19BII | No image known anywhere: not doable |
| 6 | 28C | 1LK7 + 1LP2 + 1LP4 | b, c | High: two companion chips with no public description, so everything comes from the ROM's behaviour | 2-3 | Dumpable over IR into an HP 48; Emu28's manual only names the chips |
| 7 | 18C | 1LK7 + 1LP2 + 1LP4 | d | Low after the 28C | +0.5 after the 28C | No image known; Emu28's author never had one: not doable |
| 8 | 32SII, 32S, 22S, 14B | Sacajawea | b | High: a new chip with ROM and RAM on board, a character-cell display driven as segments, only register names published | 2-3 for the chip, +0.5 per model | No IR port and the ROM is inside the chip: an owner cannot dump it without decapping or an undocumented debug mode |
| 9 | 10B, 20S, 21S | Bert | b | High: a new chip, a seven-segment display, only register names published | 2 for the chip, +0.5 per model | As the Sacajawea; low-value models |
| 10 | 71B | 1LF2/1LK7 + 1LF3 + 1LG7/1LG8 | b, c | Very high: display-driver chips, timers and clock, QWERTY plus numeric keypad, four module ports. HP-IL (the 82401A has its own I/O processor) and the card reader come on top | 4-6 for the core, +3-4 for HP-IL and the card reader | Best documented (HP's IDS volumes and Hardware Design Specification); ROM images offered openly; dump over HP-IL |

The ranking is by complexity only. The 19B and 18C (ranks 5 and 7)
cannot be done without a ROM image, however simple they are.

## ROM availability

Classes: (a) distributed with HP's permission, (b) circulating without
stated permission, (c) only by dumping your own calculator, (d) unknown.
**No model here is class (a).** HP's mid-2000 permission on hpcalc.org
covers only the 48 ROMs.

- **71B: b, c.** Garnier's Emu71 page offers the 71B and module images.
  He says he has no formal authorisation from HP and relies on HP having
  published the ROM source listings (IDS Volume III). The HP-71B
  Compendium shares the images "for personal and non-commercial use",
  based on its compiler's own understanding. Own dump: a BASIC `PEEK$`
  loop sends the ROM to an HP-IL printer device, which a PC captures
  through an HP-IL interface (the PIL-Box). This needs the 82401A HP-IL
  module. HP has made no statement.
- **28C, 28S: b, c.** Images circulate among hobbyists. Own dump:
  typed-in machine code sends the ROM as IR printer data to an HP 48
  running a binary-safe INPRT (Gießelink's CPROMUPL, `CLAMSHEL.TXT`).
  It takes about an hour with a 48GX or a 48 with a merged 128 KB card,
  longer in pieces. HP has made no statement.
- **17B, 17BII, 27S: b, c.** As for the 42S: the built-in memory scanner
  plus about 50 bytes of typed-in code send the ROM over IR to an HP 48
  (`PIONEER.TXT`). HP has made no statement.
- **42S (done): b, c.** In 2004 HP was said to be "looking into"
  releasing the ROM. Nothing followed.
- **19BII: b.** No memory scanner, so no dump method short of
  decapping. The Emu42 author says, in 2014, that he got his images
  about ten years earlier from people at HP who were interested in an
  emulator, and that some others have since been lost.
- **14B, 22S, 32S, 32SII, 10B, 20S, 21S: b.** The ROM is inside the chip
  and there is no IR port. The only methods are decapping the chip and
  reading the ROM optically, which destroys the calculator, or a
  pin-selected debug mode that HP's internal documents mention without
  giving the pinout. The images that exist came from HP to the Emu42
  author or from bus captures.
- **18C, 19B: d.** No image is known.

The owner's 48SX can serve as the IR receiver for the Lewis and the
28C/28S dumps, with a merged 128 KB card or in pieces. But the owner
owns none of these calculators.

## Audience and testability

- **Audience**, by how a user can get a ROM. The 71B is the only model
  whose images are offered openly with a reasoned argument, and its
  collector and HP-IL community is the most active of these. Any owner
  with an HP 48 can dump a 28S, 17BII or 27S. The low-end Pioneers, the
  19BII, the 18C and the 19B give users no path to a ROM.
- **Testability**: the owner owns a 48SX, 38G, 49G and 42S, none of the
  candidates. No oracle exists: saturnng covers only the 48/49 family.
  The Emu42, Emu28 and Emu71 binaries could serve as black-box oracles.
  The clean-room rule allows black-box runs, and their sources stay
  closed to us. But they are Windows programs and would need Wine, so
  using them is an owner decision. Without either, correctness rests on
  each ROM's self-test (CRC or checksum) and on documented behaviour, as
  on the 42S.

## Recommendation

1. **If anything comes next, do the single-chip Lewis family**: the 27S,
   17BII and 17B in one iteration. It is cheap because it is the 42S
   board, but it adds little: two financial calculators and a scientific
   one that the 42S supersedes. Do it only if the owner wants breadth.
2. **The 28S is the one Lewis model worth real effort** (2-3
   iterations). It is the RPL ancestor of the 48, users can dump it, and
   the work settles the open Lewis memory-map question. The 19BII can
   follow in one more iteration, but only if the owner accepts images
   that are class b only. Buying a 28S would give a real unit to test
   against and photograph, and a ROM of our own.
3. **Do not do the Bert and Sacajawea models.** They are new chips with
   only register names published, their ROMs cannot be dumped by users,
   and their segment displays are of little interest. The effort does not
   pay.
4. **Skip the 18C and 19B** (no ROM). **Skip the 28C** too, unless the
   28S is done and the owner wants it: it has undocumented companion
   chips, and the 28S supersedes it.
5. **The 71B is a separate project, not a next step.** It has the best
   documentation and the most available ROM, but it is a different
   machine (BASIC, QWERTY, ports, HP-IL) and would take 7-10 iterations
   to be useful. If the owner wants it, start by ingesting the IDS
   Hardware Design Specification into the wiki, before any code.
