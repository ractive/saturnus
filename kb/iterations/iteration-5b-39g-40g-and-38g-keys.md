---
title: "Iteration 5b: 39G/40G models and 38G key names"
type: iteration
date: 2026-10-05
status: planned
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

- [ ] 38G keyboard names from the wiki table (48 matrix, PLOT/SYMB/NUM etc., two empty positions).
- [ ] `rom fetch` entries with SHA-256 for `38grom.zip` and `rom3940.zip`; detect packed vs unpacked for `rom.39g`; zero the I/O window if a dump carries one.
- [ ] `Model::Hp39g`/`Hp40g` on the 49G machine: 1 MB ROM mirrored over the 16 bank numbers, 256 KB RAM on NCE2, no ERAM, no flash write path; 39G/40G keyboard names from the wiki table.
- [ ] Find the 39G/40G model strap by tracing the cold start (I/O reads, IN after OUT) and record it in `questions/hp39g-40g-model-detection`.
- [ ] Decode a SEND-to-disk-drive transfer on the emulated UART to settle the 38G/39G wire protocol (`questions/hp38g-39g-transfer-protocol`); hptx needs it.
- [ ] Acceptance without an oracle: boot to HOME, reset chords (ON plus menu key 3, ON plus menu keys 1 and 6), screen snapshots as golden files.

## Acceptance criteria

- [ ] 39G and 40G boot to HOME from `rom.39g`, take key input, and their
  golden screens are recorded; the 38G has its own key names.
