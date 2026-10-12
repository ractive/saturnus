---
title: "Iteration 42: HP 27S, 17BII and 17B (one Lewis, the 42S board)"
type: iteration
date: 2026-10-12
status: in-progress
branch: iter-42/pioneer-27s-17b
tags:
  - iteration
  - saturnus
  - web
---

# Iteration 42: HP 27S, 17BII and 17B

Read first: `kb/docs/clean-room-rule.md`, [[iterations/iteration-15-hp42s]]
and its decision-log entry (2026-10-05), [[research/more-saturn-models]]
(rank 1 and 2), `kb/docs/test-policy.md`; wiki `hardware/lewis`,
`hardware/hp42s`, `hardware/saturn-models`, `sources/emu42-pioneer-dump`,
`sources/pioneers-rom-dumping`, `sources/kml20`, `questions/hp42s-rom-crc`.

## Goal

Three more Pioneer models on the 42S's board: the 27S (scientific), the
17BII and the 17B (financial). One Lewis chip, ROM at #00000, display and
registers at #40000, 8 KB RAM at #50000, the 131x16 LCD, 37 keys in the
same case (wiki: hardware/saturn-models "Models" and the notes under it).
New over the 42S: other keyboards and labels, the 17BII's international
ROM of 96 KB (a second 32 KB ROM at #20000), and the 17B/17BII clock with
alarm. Each boots to its normal display, does `2 ENTER 3 +` (or its
algebraic equivalent, see below) and passes its own self-test.

## What exists to reuse

- `crates/saturnus-core/src/machine/model.rs`: `HardwareProfile` with
  `lewis: true` (`const HP42S`), `Model::Hp42s` and its per-model answers
  (`rom_bytes`, `clock_hz`, cycle table `Sasm`, contrast range 15..=31,
  reset 22, `has_serial` false, `Layout::Hp42`). The new models are three
  profiles of the same shape; the test `filter(lewis).count() == 1`
  becomes a list.
- `crates/saturnus-core/src/machine/hardware.rs`: the fixed Lewis map
  (`lewis_select`, `LEWIS_IO_BASE`, `LEWIS_RAM_BASE`, the 32 KB RAM
  decode and the 8 KB mirror). The map already names #20000 as the
  optional second ROM; the 96 KB image fills it.
- `crates/saturnus-core/src/io/lewis.rs` (`LewisIo`: display RAM,
  registers, timers and CRC through `Timers`), `Lcd::render_lewis` and
  `LCD_HEIGHT_42S` in `machine/lcd.rs`: unchanged, the display is the
  42S's.
- `crates/saturnus-core/src/io/keyboard42.rs` and `io::Layout::Hp42`:
  the pattern for one keyboard table per model, from the KML OutIn table.
- `crates/saturnus-host/src/skins/hp42s.rs` (case, wells as skirts,
  bezel, label bands) and `skins/mod.rs` (the skin model and its unit
  tests); `crates/saturnus-host/src/romid.rs` (`identify`,
  `fitting_models`, `plan`).
- Host gating already written for the 42S: `crates/saturnus-cli/src/rom.rs`
  (`fetch` refuses, test `the_42s_has_no_download`) and `main.rs` (the
  "no download: read the ROM out of your own 42S" line), `web/norom.js`,
  `web/savedstates.js` (`MODEL_CODES`), `web/screenshot.js` (`STRIP_42S`),
  `web/components/sat-calculator.js` (`ANNUNCIATORS_42S`),
  `web/tx/emulator.js` ("has no serial port"), `crates/saturnus-tx-web`
  (`copy_plan_json` refuses the 42S), `crates/saturnus-tauri/src/roms.rs`
  (slot with no download), `web/style.css` and `web/landing/style.css`
  (`[data-logo=<model>]`), `web/landing/shots.mjs`.

## Facts needed, and where they are

| Fact | Where | State |
| --- | --- | --- |
| Board, memory map, RAM 8 KB | wiki hardware/saturn-models (the 17BII memory editor, from sources/pioneers-rom-dumping) | known |
| Keyboard matrices | KML 2.0 "OutIn Codes HP17B / HP17BII" and "OutIn Codes HP27S" (`raw/emulator-docs/kml20/KML_20.txt`, lines 741 and 1240) | in raw, not yet on the wiki |
| Contrast range and reset | KML 2.0 contrast table: 17B, 17BII, 27S 0-31, reset 22, keys 15-31 | in raw, not yet on the wiki |
| ROM size and the international ROM | wiki saturn-models (64 KB, 96 KB); Emu42 CHANGES v1.24 (all High-End Pioneers have room for a 32 KB extra ROM) | known; the image form (one 96 KB file, or 64 + 32) is open |
| Self-test chord, expected screens, cold start and reset chords | each model's owner's manual (appendix on the self-test and memory reset) | missing: research |
| Key labels, shifted labels, case colours | owner's manual keyboard figure; published photographs | missing: research |
| Clock and alarm: which timer holds time, how the alarm wakes | not published; trace the 17BII ROM in saturnus (as iteration 15 traced the 42S) | missing: research |
| Which annunciators each model has | KML 2.0 (seven on the Lewis); each manual's display chapter | partly |
| Typing: does any of them type letters from keys | manuals (ALPHA menus as on the 42S) | missing: research |

Research reads published documents only (owner's manuals, KML 2.0,
Emu42's text files already in the wiki's `raw/`), never emulator source.
Results go back to the wiki as commits: a `hardware/hp17bii` page
(covering the 17B), a `hardware/hp27s` page, source pages per manual,
and the KML tables' facts in our own words. The wiki's `.hyalo.toml`
allows only `48sx|48gx|49g|38g|39g|40g|42s` in `models`: extend the
pattern with `27s|17b|17bii` first.

## ROMs and identification

- Owner puts the images in `roms/` (never committed). The agent checks
  each (size, the start is Saturn code, message texts of the model) and
  copies it to a fixed name the tests use: `roms/hp27s.rom`,
  `roms/hp17bii.rom` (English, 64 KB), `roms/hp17bii-intl.rom` (96 KB, if
  there is one), `roms/hp17b.rom`. Record each image's size, SHA-256 and
  revision in the iteration's outcome and on the wiki page (no image
  content).
- **Identification follows the 42S, by size and form, plus content**:
  a 64 KB image now fits four models (42S, 27S, 17B, 17BII), so size
  alone no longer names one. `romid::fitting_models` narrows a 64 KB
  image by a short signature per model, a message text the ROM holds
  (as the 42S's "Invalid Forecast Model"), chosen by the agent from the
  owner's images and recorded on the wiki page as a fact ("the 27S ROM
  contains the text X"). An image matching no signature fits all four
  and goes through `plan`'s offer (the user decides), as ambiguous
  images do today. 96 KB fits only the 17BII. No `KNOWN` entries: those
  carry hpcalc.org downloads, and these ROMs have none.

## Skins

Drawn like every other skin: Rust data in
`crates/saturnus-host/src/skins/` (one file per model, `hp27s.rs`,
`hp17bii.rs`; the 17B shares the 17BII drawing with its own name, as the
40G shares the 39G's, if the manuals show the same labels, otherwise its
own file), rounded rectangles, wells, label bands and text, the relief
and texture rules of iteration 24.

- **Geometry** from the 42S skin: same Pioneer case and key grid
  (inferred; confirm against each manual's keyboard figure, measured in
  pixels as the 39G's was, and note any difference in the file header).
- **Labels and shifted labels** from the keyboard figure of each model's
  owner's manual, read as text; the top row is the six menu keys, so the
  softkey-over-menu-key test applies (within half a pitch, as the 42S).
- **Colours** from published photographs of each model (Wikimedia
  Commons or the HP Museum's pages), medians as on the 42S, toned so every
  label keeps 4.5:1 (the 39G rule). The shift key's colour is the
  model's own. Nothing from a photo or figure is stored or reproduced.
- **No HP logo or wordmark**: the saturnus logo where the HP logo was,
  the model name and the case lettering as plain text. Logo colours per
  model in the `[data-logo]` tables, checked by `web/test/logo.test.mjs`.
- The file header names the figure and photos used and what is inferred.

## Hosts

- **CLI**: `--model 27s|17bii|17b`; `rom fetch` refuses with "no
  download: read the ROM out of your own calculator" pointing to `--rom`,
  as for the 42S; `--serial` and `--autostart` refuse (no serial port).
- **MCP**: `boot` takes the three; semantic tools answer "no Kermit server
  on this model"; `type_text` refuses letters if the models type them from
  ALPHA menus.
- **Web emulator**: the three in the model list; ROM by file only (Choose
  ROM…, drop); the no-ROM message says the ROM must be read out of one's
  own calculator (`web/norom.js`); the annunciator strip of each; saved
  states (`MODEL_CODES`, new model codes 7, 8, 9 in the core's state).
- **Desktop app**: the three ROM slots with no download (`roms.rs`), its
  ROM self-test list (`selftest-roms.js`).
- **saturnus tx**: not transferable. None of the three has a serial port
  or Kermit; their only output is the IR printer. The transfer page does
  not offer them as devices (`web/tx/emulator.js` names them as having no
  serial port), and `copy_plan_json` refuses them.
- **Landing page**: a card per model with a screenshot shot from the
  ROM (ROM-gated in `web/landing/shots.mjs`), if the owner wants them
  listed (owner check).

## Tasks

- [x] Research (published docs only, results to the wiki first): the
  wiki schema's `models` pattern; `hardware/hp17bii` and `hardware/hp27s`
  pages (keyboard matrices from KML 2.0, contrast, annunciators, ROM
  sizes, self-test and reset chords from the owner's manuals, the 96 KB
  image form); a source page per manual; trace the 17BII clock and alarm
  in saturnus (which registers, how the alarm wakes) and file what stays
  open as `questions/`. Feasibility note in the PR before any core code.
- [x] ROMs: check the owner's images, copy to the fixed names, record
  size, SHA-256 and revision; content signatures for identification.
- [ ] Core: `Model::Hp27s`, `Hp17bii`, `Hp17b`, their profiles (the 42S's
  map; ROM length 64 or 96 KB), keyboard tables, model codes, the 96 KB
  image mapped at #00000-#2FFFF; the clock running from the 8192 Hz timer
  and the alarm waking the CPU as the ROM expects; state save/load.
- [x] Identification: `romid` signatures, the 64 KB ambiguity through
  `plan`'s offer, unit tests with synthetic images (no ROM content in
  tests).
- [x] Skins: `hp27s.rs`, `hp17bii.rs` (and `hp17b.rs` if its labels
  differ), header comments with sources, the skins' unit tests.
- [x] Hosts: CLI, MCP, web emulator, desktop app, saturnus tx gating as
  above; computer-keyboard bindings for digits, operators, ENTER, ←.
- [x] Tests (below), goldens, docs: web/README.md "Skins", CHANGELOG,
  decision log, the wiki pages.
- [ ] Owner: put the images in `roms/` (27S, 17BII English, 17BII
  international if you have it, 17B) and say where each came from.
- [ ] Owner: look at the three skins (1280 and 390 wide screenshots in
  the PR) against your memory of the calculators and published photos.
- [ ] Owner: decide whether the landing page lists them.

## Tests

ROM-gated by `SATURNUS_ROM_DIR` like the 42S (each test skips when its
file is missing):

- Per model, `saturnus-core/tests/e2e.rs`: boot to the normal display
  (golden screen), the arithmetic check (`2 ENTER 3 +` on the 27S; the
  17BII and 17B are algebraic by default: `2 + 3 =`, or switch to RPN
  first as the manual says) showing 5, state round trip, the self-test
  (chord from the manual) with its result screen as a golden.
- 17BII: the clock advances with emulated time (set a time, run 60 s,
  read it back); an alarm set one minute ahead fires.
- 17BII international: boots and shows a non-English message (the second
  ROM is mapped).
- CLI e2e: `--model 27s` boots and prints the screen; `rom fetch 27s`
  refuses.
- Host unit tests: `romid` with synthetic 64 and 96 KB images; skin tests
  (every matrix key drawn once, no overlap, labels over menu keys).
- Web: the model list, the no-ROM message, a drawn key press (mouse on
  2, ENTER, 3, +) on the 27S in headless Chrome.

**Oracle**: each ROM's own self-test (its ROM CRC step) plus boot and
arithmetic. No other emulator is run. If the ROM CRC step passes on these
images with saturnus's CRC rule, write that to `questions/hp42s-rom-crc`
(the 42S image is then the suspect). If every image fails it the same
way, the Lewis CRC rule differs from the 48's: settle that (on the wiki,
from the ROMs' behaviour) before calling the self-test acceptance met.

## Acceptance criteria

- [ ] The 27S, 17BII (and the 17B, 17BII international where the owner
  has the image) boot to their normal display, the arithmetic check shows
  5, and the self-test's ROM step passes.
- [x] A 64 KB image is named correctly when it carries a model's
  signature, and offered to the user when it does not.
- [ ] Each skin is operable by mouse and touch; screenshots at 1280 and
  390 wide, headless Chrome only (never cmux-cua or screencapture), two
  per skin, in the PR.
- [x] `rom fetch` and the web page say "dump your own"; saturnus tx does
  not offer them.
- [x] `hyalo lint` clean here and in the wiki.

## Process

- Targeted tests while working (the touched crate's tests, the touched
  `web/test/*.test.mjs`).
- Once before the PR: `just gates`, `just web-test`, `just rom-tests
  roms` (the test lock: one suite at a time).
- One Opus reviewer; no second review round.
- At most three agents at a time: research, core and skins can run in
  parallel once the research note is in.

## Outcome

Done 2026-10-13, except the self-test ROM step (the Lewis CRC rule is
still open), the alarm, and the English 64 KB 17BII.

- **ROMs** (the owner's, read only from `roms/`; the names were already
  the fixed ones, so nothing was copied):
  - `hp27s.rom`: 65536 bytes, SHA-256 `df64a595…e380`, 27S revision B
    ("OK-27S-EB").
  - `hp17bii.rom`: 98304 bytes (international), SHA-256 `bf2f77da…176e`,
    main ROM revision B ("17B II-EB"), language ROM revision A.
  - `hp17b.rom`: 98304 bytes (international), SHA-256 `6540a89b…cc4e`,
    main ROM revision B ("OK-17B-EB"), language ROM revision B.
  - `hp42s.rom`: the owner's revision C 42S ROM again, in dump order.
  - No 64 KB English 17BII image: that form is untested.
- **Image forms**: all three new images (and `hp42s.rom`) store the ROM in
  256 units last first. The core puts such an image in address order
  (`pioneer_address_order`); `disasm` too (wiki: hardware/saturn-models
  "ROM image forms").
- **Core**: three models on the 42S's profile, `Layout::Hp27` and
  `Layout::Hp17`, new keys (`percent`, `dsp`, `prt`, `exit`, `input`,
  `equals`), model codes 7-9, 96 KB on the 17B and 17BII.
- **Validation** (goldens recorded from saturnus): each boots ("MEMORY
  LOST" on the 27S; "SELECT LANGUAGE" on the international 17B/17BII, so
  the language ROM at #20000 is mapped), `2 + 3 =` shows 5.0000 / 5.00,
  states round-trip, shift lights its annunciator, mouse clicks on the
  drawn 2 + 3 = give 5 in headless Chrome. The 17BII clock ticks once per
  emulated second in TIME.
- **Self-test**: runs on all three (CLR + fifth menu key). The ROM step
  prints 05DFB (27S), 01326 (17BII; ROM2 also fails) and 0AB1D (17B)
  instead of passing; the pass ends in FAIL, as on the 42S. All four ROMs
  fail the same way, and no CRC-16 variant passes them all, so the Lewis
  CRC rule differs from the 48's in a way not yet found (wiki:
  questions/hp42s-rom-crc). Acceptance not met.
- **Identification**: signatures are the self-test texts ("OK-42S-",
  "OK-27S-", "17B II-", "OK-17B-E"); unsigned 64 KB and 96 KB images are
  offered.
- **Skins**: `hp27s.rs`, `hp17bii.rs` (17BII and 17B), the 42S's
  geometry (not measured against the manuals' figures), labels from the
  manuals, colours from photographs.
- **Not done**: the alarm (not traced, not tested); touch on the new
  skins (mouse checked); the landing page cards (scenes added to
  `shots.mjs`, images not made: owner's decision).
