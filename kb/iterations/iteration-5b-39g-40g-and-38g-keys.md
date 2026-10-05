---
title: "Iteration 5b: 39G/40G models and 38G key names"
type: iteration
date: 2026-10-05
status: completed
branch: iter-5b/39g-40g-and-38g-keys
tags:
  - iteration
  - saturnus
---

# Iteration 5b: 39G/40G models and 38G key names

Read first: wiki `hardware/hp38g`, `hardware/hp39g-40g`, `hardware/hp49g`,
`questions/hp39g-40g-memory-map`, `questions/hp39g-40g-model-detection`,
`questions/hp38g-39g-transfer-protocol`, `questions/hp38g-memory-controllers`.

## Context from iteration 5 (2026-10-05)

- The machine is multi-model (`machine/model.rs`: `Model`, `HardwareProfile`,
  `ChipRole`, per-model `keyboard_layout()`); the 38G boots as a Yorke
  configuration (32 KB RAM at #F0000, 48 key matrix, no ports); the 49G has
  the banked flash on NCE1 with Sousa's latch bit order and `shutdn_clears_latch`
  false. The 39G/40G are a 49G cut to 1 MB mask ROM and 256 KB RAM, one ROM
  image for both (`rom3940.zip`, hpcalc details 6739, member `rom.39g`,
  2097152 bytes; packed or unpacked unknown).
- No saturnng oracle exists for these models; acceptance is boot to HOME,
  key input and the user-guide reset chords, with golden screens.

## Tasks

- [x] 38G keyboard names from the wiki table (48 matrix, PLOT/SYMB/NUM etc., two empty positions).
- [x] `rom fetch` entries with SHA-256 for `38grom.zip` and `rom3940.zip`; detect packed vs unpacked for `rom.39g`; zero the I/O window if a dump carries one.
- [x] `Model::Hp39g`/`Hp40g` on the 49G machine: 1 MB ROM mirrored over the 16 bank numbers, 256 KB RAM on NCE2, no ERAM, no flash write path; 39G/40G keyboard names from the wiki table.
- [x] Find the 39G/40G model strap by tracing the cold start (I/O reads, IN after OUT) and record it in `questions/hp39g-40g-model-detection`.
- [-] Decode a SEND-to-disk-drive transfer on the emulated UART to settle the 38G/39G wire protocol (`questions/hp38g-39g-transfer-protocol`); hptx needs it. Partly done: the framing is Kermit with the calculator as client (I packet, then a GET of `HP38DIR.CUR`/`HP39DIR.CUR`); carried over: the directory file's format, without which the calculator never sends the aplet.
- [x] Acceptance without an oracle: boot to HOME, reset chords (ON plus menu key 3, ON plus menu keys 1 and 6), screen snapshots as golden files.

## Acceptance criteria

- [x] 39G and 40G boot to HOME from `rom.39g`, take key input, and their
  golden screens are recorded; the 38G has its own key names.

## Outcome

- **39G and 40G run.** `Model::Hp39g` and `Model::Hp40g` boot hpcalc's
  `rom.39g` (1 MB unpacked, SHA-256
  `69220f42d5e90dd8825e7d1596d9eaca490ee6a7a52a3b8b96469a5f3d3f627f`,
  upload I/O window zeroed on load) to a "Memory Clear" box, then HOME;
  `6 * 7 ENTER` gives 42; ON + menu key 3 resets to HOME and ON + menu keys
  1 and 6 clear memory. Goldens `39g-*` and `40g-*`, e2e tests
  `hp39g_boot_compute_and_reset_chords` and `hp40g_boot_shows_the_cas`
  (the e2e suite is now 15 tests). Wiring: the 49G's CE1 latch over a
  banked mask ROM (`Nce1::BankedRom`, 8 banks, 8-15 mirrored), 256 KB on
  NCE2, CE2 and NCE3 empty, no flash path. The ROM configures only HDW and
  NCE2 and opens CE1 briefly around each bank switch.
- **Model strap found.** The ROM reads #11A bit 3 (the 39G's IR receive
  sample) and takes a set bit as a 40G; the 40G profile holds it high and
  HOME then shows the CAS menu key.
- **Key names.** The 38G and the 39G/40G have their own key names on the
  48 and 49G matrices (`Layout::Hp38`, `Layout::Hp39`), in the CLI, MCP
  (`type_text` types 38G letters one A...Z at a time; no letters on the
  39G/40G) and the web layout export.
- **Transfer protocol: Kermit, partly decoded.** SEND to a disk drive
  sends a Kermit I packet, then GETs `HP38DIR.CUR` / `HP39DIR.CUR` from
  the PC; an empty file is refused with a "not prepared" error, so the
  aplet itself was never sent. The task is carried over (marked `[-]`):
  the directory file's format is the missing piece (wiki: questions/hp38g-39g-transfer-protocol).
- **Diff script**: the aplet models are excluded (no saturnng oracle);
  `boot`, `gx-boot` and `49g-boot` still match.
- **Open**: the 39G/40G letter positions for `type_text`; whether a real
  40G reads #11A bit 3 high for the reason inferred; the directory file
  format; the 39G beta ROM in `emu48-39.zip`.
