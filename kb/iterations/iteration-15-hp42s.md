---
title: "Iteration 15: HP 42S (Pioneer series, Lewis chip)"
type: iteration
date: 2026-10-05
status: completed
branch: iter-15/hp42s
tags:
  - iteration
  - saturnus
---

# Iteration 15: HP 42S (Pioneer series, Lewis chip)

Read first: wiki `hardware/saturn-cpu` (chip versions: the Lewis companion
IC of the 17B/19B/27S/28S family), `kb/docs/clean-room-rule.md` (ROMs are
never shipped; emulator documentation is a fact source, emulator source is
not), `machine/model.rs` (`HardwareProfile`), `crates/saturnus-web/src/skins/`.

## Context (2026-10-05)

- Owner: owns a real HP 42S (photos in `~/Downloads/HP Taschenrechner/`,
  two shots) and wants it emulated: "I think there are emulators for it. Do
  some research and maybe you find a ROM."
- Research (web, 2026-10-05): the 42S is a Pioneer-series machine with a
  Saturn CPU in the Lewis chip at about 1 MHz, 64 KB ROM (revisions A, B,
  C), 8 KB RAM (a known 32 KB upgrade exists), a 131x16 dot-matrix LCD
  (two lines of 22 characters; each of the 131 columns is 4 nibbles), no
  I/O except the infrared printer output. Christoph Gießelink's Emu42
  emulates it (and the 10B, 14B, 17B, 17BII, 20S, 21S, 27S, 28S, 32SII).
  Free42 is a rewrite, not ROM-based, and is no source for us.
- ROM: HP never released the Pioneer ROMs; Emu42 ships none ("copyrighted
  by Hewlett Packard and I have no license to distribute them"). The
  legitimate route is to dump the owner's own 42S: the documented method
  sends the ROM over the 42S's infrared printer port to an HP 48 running
  HP's INPRT program, with the tools from Gießelink's Emu42 site (the "ROM
  upload packet"; `LEWISCRC` validates a dump; `Emu2rom`/`Rom2Emu` convert
  between packed and unpacked). The owner has both a 42S and a 48SX; the
  dump then goes from the 48SX to the PC over Kermit (hptx). The dump stays
  local like every ROM; `rom fetch` cannot offer the 42S, only `--rom FILE`.
- Unknowns to research first (wiki pages `hardware/hp42s` and a source page
  per document): the Lewis chip's memory controller and I/O register map
  (it differs from the Clarke/Yorke HDW window), the display controller
  (131x16, where the display RAM sits, contrast), the keyboard matrix (37
  keys), timers, the beeper, the IR printer output (useful later to
  capture printouts). Sources: the Emu42 manual and its KML documentation
  (facts only), the HP Museum article "HP-42S: New Facts", the HP 28S
  processor notes already in `raw/` (same chip family), service-manual
  scans if any.

## Tasks

- [x] Research the Lewis hardware and write wiki pages (`hardware/hp42s`,
  `hardware/lewis` if the chip deserves its own page, source pages), with
  open questions filed; feasibility verdict before any code.
- [x] ROM: the owner already has an image from 1999, `HP42S-C.ROM`
  (revision C; 65536 bytes, packed two nibbles per byte = 131072 nibbles;
  SHA-256 `f4c5f9f0e1d89074b7ca49add99b3ea72ed7fae9370b421de20a0cd8384c08f3`;
  file date 1999-04-05). Checked 2026-10-05: it starts with Saturn code
  (`P= 3`, `GOTO ...`) and holds the 42S message texts ("Machine Reset",
  "Memory Clear", "Clear All Memory?", "Stat Math Error", "Invalid Forecast
  Model"). Copied to the gitignored `roms/hp42s-c.rom`; it is HP's
  copyrighted code and is never committed or shipped. Emu42's `LEWISCRC`
  tool can validate it if a second opinion is wanted; a fresh dump from
  the owner's unit (IR to the 48SX with INPRT and the Emu42 upload tools)
  remains the fallback.
- [x] `Model::Hp42s`: Lewis wiring behind a new hardware profile (memory
  controller, I/O registers, 131x16 display, keyboard, timers), boot to
  the two-line display, key input, a golden; state save/load.
- [x] Skin from the owner's photos (two shots): dark case, 37 keys, the
  orange shift key, the two-line LCD; the saturnus logo; no HP marks.
- [x] Web, CLI and MCP know the model (`--model 42s`); no Kermit (the 42S
  has no serial port), so the semantic MCP tools return "no Kermit server
  on this model".
- [-] Optional: capture the infrared printer output as text.

## Acceptance criteria

- [x] The owner's dumped ROM boots in saturnus to the 42S display, `2
  ENTER 3 +` shows 5, and the skin is operable by mouse.

## Outcome

Done 2026-10-05; the optional IR printer capture is not done.

- **Research** (wiki: hardware/lewis, hardware/hp42s, seven new source
  pages, four questions). Sources: Garnier's "HP-42S: New Facts" (the live
  HP Museum page answers 403; the Wayback copy was used), Hosoda's 42S
  hardware notes, the Emu42 manual, PIONEER.TXT, LEWISCRC.TXT,
  PROBLEMS.TXT and the changelog (facts only; no Emu42 or LEWISCRC source
  opened), the KML 2.0 42S tables, Gariepy's 28S notes, and traces of the
  ROM in saturnus. Verdict: feasible.
- **Core**: `Model::Hp42s` with a fixed Lewis map (`HardwareProfile::
  lewis`), `io::lewis::LewisIo` (display RAM and registers; timers and CRC
  shared with the 48 code), 8 KB RAM mirrored through #5FFFF, the 131x16
  LCD (`Lcd::render_lewis`, `LCD_HEIGHT_42S`), seven annunciators, the 42S
  keyboard (`io::keyboard42`, new keys `sigmaplus` `xeq` `rcl` `rdn` `swap`
  `rs`, alias `exit`), state save/load. 1 MHz, SASM counts, no factor, no
  stall (uncalibrated).
- **Validation**: the ROM boots to "Memory Clear", `2 ENTER 3 +` shows
  `x: 5.0000` (goldens `42s-memory-clear`, `42s-two-plus-three`), state
  round trip, shift annunciator. The self-test (EXIT + LN) runs SPD, BEEP,
  DISP, ROM, DRAM, URAM; the ROM step computes CRC #1BE8, not #FFFF, so
  the summary reads FAIL (golden `42s-self-test-rom`): the 1999 image may
  have bad bits, or the Lewis CRC differs (wiki: questions/hp42s-rom-crc;
  run LEWISCRC or dump again).
- **Hosts**: CLI `--model 42s` (run, disasm); `rom fetch` refuses with a
  pointer to `--rom`; `--autostart` refuses. MCP `boot` takes `42s`; the
  semantic tools answer "no Kermit server on this model"; `type_text`
  refuses letters. Web: `lcd_height()`, the 42S annunciator strip, and a
  skin drawn from the owner's photo (16.51.44 (3), confirmed against (4)),
  checked in headless Chrome: mouse clicks on the drawn 2, ENTER, 3, +
  give 5.
- **Open**: CPU clock and RATE, Lewis cycle counts, memory-window logic,
  some register bits (wiki: questions/lewis-*); the 32 KB RAM option; IR
  printer output.
