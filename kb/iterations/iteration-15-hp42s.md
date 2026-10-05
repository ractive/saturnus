---
title: "Iteration 15: HP 42S (Pioneer series, Lewis chip)"
type: iteration
date: 2026-10-05
status: planned
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

- [ ] Research the Lewis hardware and write wiki pages (`hardware/hp42s`,
  `hardware/lewis` if the chip deserves its own page, source pages), with
  open questions filed; feasibility verdict before any code.
- [ ] ROM: the owner dumps his 42S (IR to the 48SX with INPRT and the Emu42
  upload tools, then Kermit to the PC); verify with the documented CRC;
  record size, revision and SHA-256 locally (never in the repo beyond the
  hash).
- [ ] `Model::Hp42s`: Lewis wiring behind a new hardware profile (memory
  controller, I/O registers, 131x16 display, keyboard, timers), boot to
  the two-line display, key input, a golden; state save/load.
- [ ] Skin from the owner's photos (two shots): dark case, 37 keys, the
  orange shift key, the two-line LCD; the saturnus logo; no HP marks.
- [ ] Web, CLI and MCP know the model (`--model 42s`); no Kermit (the 42S
  has no serial port), so the semantic MCP tools return "no Kermit server
  on this model".
- [ ] Optional: capture the infrared printer output as text.

## Acceptance criteria

- [ ] The owner's dumped ROM boots in saturnus to the 42S display, `2
  ENTER 3 +` shows 5, and the skin is operable by mouse.
