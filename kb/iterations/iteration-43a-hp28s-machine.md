---
title: "Iteration 43a: HP 28S machine (two Lewis chips)"
type: iteration
date: 2026-10-12
status: in-progress
branch: iter-43a/hp28s-machine
tags:
  - iteration
  - saturnus
---

# Iteration 43a: HP 28S machine (two Lewis chips)

Read first: `kb/docs/clean-room-rule.md`, [[iterations/iteration-15-hp42s]],
[[iterations/iteration-42-pioneer-27s-17b]], [[research/more-saturn-models]]
(rank 3), `kb/docs/test-policy.md`; wiki `hardware/lewis`,
`hardware/saturn-models`, `questions/lewis-memory-map`,
`questions/lewis-registers`, `sources/hp28s-procnotes`,
`sources/emu42-pioneer-dump` (CLAMSHEL), `sources/emu42-changes`,
`sources/emu42-problems`, `sources/kml20`.

## Why two iterations

The 28S is clearly two pieces of work. This one is the machine: two Lewis
chips (master and slave) with configurable windows, which the 42S's fixed
map does not have; a 137x32 display split across the two chips, wider
than the 131-pixel rows every `Lcd` has today; a keyboard on more IN and
OUT lines. It ends with the 28S booting and computing in the CLI. The
second, [[iterations/iteration-43b-hp28s-skin-hosts]], draws the
clamshell skin and wires the web, desktop and MCP hosts. The 19BII
([[iterations/iteration-44-hp19bii]]) then reuses both.

## Goal

`Model::Hp28s` boots the owner's 28S ROM to its normal display (the
stack, four lines), does `2 ENTER 3 +` showing 5, passes its own self-test,
and saves and restores state. The wiki question `questions/lewis-memory-map`
is answered (or narrowed, with what is left filed) from the 28S ROM's
behaviour.

## What exists to reuse

- `crates/saturnus-core/src/io/lewis.rs`: `LewisIo` (1024-nibble display
  and register block, registers onto `Timers` and the CRC). The 28S needs
  two instances (master and slave); the slave's power bits read 0 (wiki:
  hardware/saturn-models notes, from Emu42 CHANGES).
- `crates/saturnus-core/src/machine/hardware.rs`: `lewis_select` and the
  fixed 42S map. It becomes a map the chips' window registers set (or a
  second fixed map for the 28S, if the ROM shows the windows never move
  after reset: decide from the trace, record in the decision log).
- `crates/saturnus-core/src/machine/model.rs`: `HardwareProfile` (`lewis`
  flag, CE/NCE roles), the controller CONFIG/UNCNFG/C=ID logic the 48
  models use (the 28S may execute them; the 42S never does).
- `crates/saturnus-core/src/machine/lcd.rs`: `Lcd`, `LCD_WIDTH` 131,
  `render_lewis`. Rows become as wide as the model's display: 137 on the
  28S. `Lcd::pixels` is `Vec<[bool; LCD_WIDTH]>` today, public API that
  satx and the hosts read; the decision log (iteration 15 entry) says
  consumers read sizes from values, so make rows a width the `Lcd`
  carries (`Lcd::width`), keep the 131 models unchanged, and check every
  consumer (PNG and text dumps, `screenText`, the web canvas, the GROB
  screen export of iteration 31, satx).
- `io::keyboard42` and the KML-table pattern for the 28S matrix.

## Facts needed, and where they are

| Fact | Where | State |
| --- | --- | --- |
| ROM 128 KB at #00000-#3FFFF; RAM 32 KB at #C0000 aliased at #D0000 | wiki sources/hp28s-procnotes "Overview" | known |
| Display RAM at #FF840 (left) and #FFC00 (right), two columns per 16 nibbles, 34x2 columns per half plus one | wiki sources/hp28s-procnotes (SCR) | known; how the halves map to the chips is inferred |
| Master and slave; slave power bits read 0 | wiki sources/emu42-changes via hardware/saturn-models | known, thin |
| Window logic: CONFIG, C=ID, UNCNFG on a Lewis; what the registers at the windows are | not published | missing: trace the 28S ROM in saturnus (log every CONFIG, C=ID, UNCNFG and the addresses each chip answers), file on `questions/lewis-memory-map` |
| Register block addresses of each chip (timers, CRC, contrast, DSPCTL) | inferred from the 42S layout moved to the windows the ROM sets | missing: trace |
| Keyboard matrix: OUT bits 1-7, IN #001-#800, ON on IN #8000 | KML 2.0 "OutIn Codes HP28S" (`raw/emulator-docs/kml20/KML_20.txt`, line 1533) | in raw, not on the wiki |
| Contrast 0-31, reset 22, keys 16-31 | KML 2.0 contrast table | in raw, not on the wiki |
| Annunciators of the 28S | owner's manual display chapter; the ROM's writes | missing: research |
| Self-test and reset chords, expected screens | HP 28S Owner's Manual (appendix) | missing: research |
| CPU clock | wiki questions/lewis-clock-and-rate (open; the self-test's SPD value) | open, not blocking |

Research reads published documents only (the 28S Owner's Manual and
Reference Manual, Gariepy's notes, KML 2.0, the Emu42 text files) and the
28S ROM's behaviour in saturnus; never emulator source, and no other
emulator as an oracle. Results go to the wiki first as commits: a
`hardware/hp28s` page, the two-chip facts on `hardware/lewis`, the
answer (or what is left) on `questions/lewis-memory-map`, a source page
per manual. Extend the wiki schema's `models` pattern with `28s` (and
`19bii` for the next iteration) if iteration 42 has not.

**Stop condition**: if after the trace the window logic cannot be
inferred well enough to boot (the ROM loops in its memory setup), stop,
write what was found to the wiki and the decision log, and report; do
not guess a map to make the screen appear.

## ROM and identification

- Owner puts the 28S image in `roms/`; the agent checks it (128 KB
  packed or 256 KB unpacked, Saturn code at the start, 28S message texts)
  and copies it to `roms/hp28s.rom`. Size, SHA-256 and revision (the 28S
  has several) go into the outcome and the wiki page.
- Identification follows the 42S: by size and form, plus a content
  signature, because 128 KB also fits the 19BII
  ([[iterations/iteration-44-hp19bii]]). No `KNOWN` entry (no download
  exists). An image matching no signature is offered (`plan`).

## Tasks

- [x] Research: the 28S pages on the wiki (manuals for the self-test,
  reset and annunciators; KML 2.0 keyboard and contrast), the schema's
  `models` pattern; a source page per manual.
- [x] Trace: run the 28S ROM in saturnus with a logging bus (a debug
  feature or a test harness, not shipped behaviour) on a provisional map
  from Gariepy's addresses; record every window and register access per
  chip; write the findings to `hardware/lewis` and
  `questions/lewis-memory-map`. Feasibility verdict in the PR before core
  code.
- [x] Core: two `LewisIo` (master, slave) behind a 28S profile; the window
  map the trace supports; RAM 32 KB at #C0000 with its alias; the display
  RAM halves into one 137x32 `Lcd` (`Lcd::width`, 131 models unchanged);
  the 28S keyboard table; model code 10 (after iteration 42's 7-9); state
  save/load with both chips' blocks.
- [x] `Lcd` width through every consumer: PNG and text dumps, the CLI's
  screen output, satx (wildcard arms, sizes from values), the web's
  `lcd_width()` beside `lcd_height()` (the web canvas itself in 43b).
- [x] CLI: `--model 28s` (run, disasm, screen); `rom fetch 28s` refuses
  with "dump your own" and points to `--rom`; `--serial`, `--autostart`
  refuse (no serial port).
- [x] Identification: the 28S signature in `romid`, synthetic-image unit
  tests.
- [x] Docs: decision log (the map, the `Lcd` width change, the core API
  change for satx), CHANGELOG, the wiki pages.
- [ ] Owner: put the 28S image in `roms/` and say where it came from
  (own dump over IR to the 48SX, or a circulating image).
- [ ] Owner: read the feasibility verdict and the map decision before the
  core work goes on (a comment on the PR is enough).

## Tests

ROM-gated by `SATURNUS_ROM_DIR` like the 42S:

- `saturnus-core/tests/e2e.rs`: boot to the stack display (golden),
  `2 ENTER 3 +` shows 5, a cold start screen, the self-test's result
  screen (golden), state round trip with both chips.
- CLI e2e: `--model 28s` boots and prints the 137x32 screen; `rom fetch
  28s` refuses.
- Unit tests (no ROM): the window map (an address lands in the right
  chip's block for each configuration the trace found); the 137-column
  `Lcd` render from both chips' display RAM; the keyboard table; `Lcd`
  width in the PNG and text dumps.

**Oracle**: the 28S ROM's own self-test and its ROM CRC, boot and
arithmetic; no other emulator. If the ROM CRC fails with the 48's CRC rule
here too, compare with iteration 42's result before suspecting the image.

## Acceptance criteria

- [ ] The owner's 28S ROM boots in the CLI to the stack, `2 ENTER 3 +`
  shows 5, the self-test's ROM step passes.
- [x] `questions/lewis-memory-map` is answered or narrowed on the wiki,
  with sources (ROM behaviour and published docs).
- [ ] Every 131-column model's goldens are unchanged.
- [x] `hyalo lint` clean here and in the wiki.

## Process

- Targeted tests while working.
- Once before the PR: `just gates`, `just web-test`, `just rom-tests
  roms` (one suite at a time).
- Review: one Opus reviewer plus GLM (the review-pr skill's GLM pass) for
  the two-chip memory work; no second review round.
- At most three agents: research and trace first, then core and the `Lcd`
  width in parallel.

## Outcome

Done 2026-10-13, except the self-test's ROM step (the Lewis CRC rule is
still open, as on the Pioneers).

- **ROM**: `roms/hp28s.rom`, already under that name (nothing copied),
  131072 bytes, SHA-256 `51589fa3…1e921c`, revision 2BB ("Version 2BB",
  "Copyright HP 1986, 1987", self-test text "OK-28S"). In dump order
  (256 units of 512 bytes, last first); the core reorders it. The 28C and
  19BII images in `roms/` are also in dump order; out of scope.
- **Feasibility verdict and map** (wiki: questions/lewis-memory-map,
  hardware/hp28s "Memory map"): the ROM, traced on a logging bus through
  cold boot, arithmetic, menus, typing, printing, System Halt, Memory
  Reset and the self-test, executes RESET at reset and never CONFIG,
  UNCNFG or C=ID, and touches only ROM #00000-#3FFFF, RAM #C0000-#DFFFF
  (32 KB twice), the slave's block #FF800-#FFBFF and the master's block
  at #FFC00-#FFFFF, each with the 42S's internal layout. So a second fixed
  map, no window registers; the stop condition did not apply.
- **Two chips**: the master's timers are the machine's, its CRC the
  self-test's; the slave has timers of its own (the ROM blinks the
  command-line cursor on them, every 0xE67 ticks), shares the interrupt
  line and wakes the CPU. Display: slave columns 0-67 from #FF840, master
  68-136 from #FFC00, eight nibbles per column. Annunciators: seven
  words in the slave's RAM from #FF800 (busy, alpha, battery, shift,
  RAD, suspended, printer), each seen lit.
- **Core**: `Model::Hp28s` (code 10), `LewisBoard`, `Layout::Hp28` (12 IN
  lines; menu keys `a`-`f`, letters `lettera`-`letterz`, new keys
  `hash`, `lbrace`, `lbracket`, `program`, `lc`, `cursor`, `trig`,
  `solv`, `user`, `drop`), `Lcd::width` with `Vec<Vec<bool>>` rows,
  `Model::lcd_size`, `Annunciators::suspended`.
- **Validation** (goldens recorded from saturnus, no oracle): cold boot
  shows "Memory Lost" over levels 3: to 1: (`28s-cold`); `2 ENTER 3 +`
  leaves 5 in level 1 (`28s-two-enter-three-plus`); state round-trips;
  shift lights its annunciator; the cursor blinks. The CLI boots it and
  prints 137x32 text and PNG.
- **Self-test**: ON + the fifth menu key runs it; the ROM step gets the
  CRC #185A instead of #FFFF and the pass shows "8 FAIL" (golden
  `28s-self-test`). Acceptance not met, as in iteration 42.
- **Hosts**: the web page, desktop app and MCP model lists leave the 28S
  out until its skin (43b): `skins::skin` is an `Option`,
  `skins::hosted_models`. The frame event and `storeScreen` already take
  the width from the `Lcd`; no separate `lcd_width()` binding was needed
  (the Worker has no `lcd_height()` either: the skin view carries the
  rows, the frames the size).
- **Identification**: "OK-28S" names a 128 KB image; an unsigned one
  fits no model (decision log).
- **Wiki**: calculator-knowledgebase PR #12 (`hardware/hp28s`, the two
  manuals, the memory map, `questions/lewis-memory-map` answered).
