---
title: "Iteration 44: HP 19BII (two Lewis chips, after the 28S)"
type: iteration
date: 2026-10-12
status: completed
branch: iter-44/hp19bii
tags:
  - iteration
  - saturnus
  - web
---

# Iteration 44: HP 19BII

Read first: [[iterations/iteration-43a-hp28s-machine]] and
[[iterations/iteration-43b-hp28s-skin-hosts]] (their outcomes),
[[research/more-saturn-models]] (rank 4), `kb/docs/clean-room-rule.md`,
`kb/docs/test-policy.md`; wiki `hardware/hp28s`, `hardware/lewis`,
`hardware/saturn-models` (ROM availability), `sources/kml20`,
`sources/hpmuseum-forum` (F3).

## Goal

The 19BII, the financial clamshell, on the 28S's two-chip machine and
clamshell skin: boot to its normal display, arithmetic, its self-test,
state, and the hosts. Low cost after the 28S: the same hardware with
another keyboard (KML 2.0's 19BII table uses the 28S's matrix lines).

## ROM: class b only

The 19BII has no memory scanner, so an owner cannot dump it without
decapping; the images that exist came from people at HP to the Emu42
author (wiki: hardware/saturn-models "ROM availability", sources/
hpmuseum-forum F3). Running such an image is the owner's decision (open
question below). The web page and the CLI cannot say "dump your own"
truthfully: they say "no download; saturnus has no way to get this ROM
for you" and take `--rom` / Choose ROM… like every Lewis model.

## What exists to reuse

- 43a: the two-chip Lewis profile and window map, 32 KB RAM at #C0000,
  the `Lcd` width per model, the 28S keyboard pattern, the trace harness.
- 43b: the two-half clamshell skin with side-by-side and stacked layouts
  (`skins/hp28s.rs`, the layout switch in the page), letters from the
  computer keyboard to letter keys.
- Iteration 42: content signatures in `romid` for same-size images (the
  19BII and 28S are both 128 KB).
- The 42S/28S host gating (CLI, MCP, web, desktop, saturnus tx).

## Facts needed, and where they are

| Fact | Where | State |
| --- | --- | --- |
| Two Lewis chips, ROM 128 KB | wiki hardware/saturn-models | known |
| Memory map: the 28S's, or other windows | the 19BII ROM's behaviour (the 43a trace harness) | missing: trace |
| Display: 131 or 137 columns by 32 ("132 x 32", unverified) | the ROM's display writes; the owner's manual | missing: research |
| Keyboard matrix | KML 2.0 "OutIn Codes HP19BII" (`raw/emulator-docs/kml20/KML_20.txt`, line 844) | in raw, not on the wiki |
| Contrast 0-31, reset 22, keys 16-31 | KML 2.0 contrast table | in raw, not on the wiki |
| Clock and alarm (the 19BII has time and appointments, as the 17BII) | owner's manual; the 42's clock work | research |
| Self-test and reset chords, annunciators | HP 19BII Owner's Manual | missing: research |
| Key labels, colours | the manual's keyboard figures; published photographs | missing: research |

Research from published documents and the ROM's behaviour only; results
to the wiki first (`hardware/hp19bii`, a source page per manual, the
schema's `models` pattern if not yet extended).

## Tasks

- [x] Research: `hardware/hp19bii` (display size, keyboard, contrast,
  annunciators, self-test, clock), the manual's source page.
- [x] Trace the 19BII ROM with 43a's harness: confirm it uses the 28S's
  windows, or record the difference on `hardware/lewis`.
- [x] Core: `Model::Hp19bii` on the two-chip profile, keyboard table,
  display width, model code 11, clock and alarm as on the 17BII, state.
- [x] Identification: the 19BII signature beside the 28S's (128 KB).
- [x] Skin: `skins/hp19bii.rs` on the clamshell model of 43b (labels from
  the manual's figures, colours from published photos, no HP logo, the
  model name as text), both layouts.
- [x] Hosts: CLI `--model 19bii`, `rom fetch` refuses ("no download"),
  MCP boot, web model list and ROM by file, desktop slot, saturnus tx
  refuses (no serial port, no Kermit; IR printer only).
- [x] Docs: decision log, CHANGELOG, web/README.md "Skins".

Owner checks still open: [[backlog/owner-checks]]

## Tests

ROM-gated by `SATURNUS_ROM_DIR` (`roms/hp19bii.rom`):

- Core e2e: boot to the normal display (golden), `2 + 3 =` (or the RPN
  equivalent if set) shows 5, the self-test's result (golden), state round
  trip, the clock advances.
- CLI e2e: boot and screen; `rom fetch 19bii` refuses.
- Web: boot from a chosen file, mouse clicks give 5.
- Unit: keyboard table, `romid` with synthetic images, the skin tests per
  half.

**Oracle**: the ROM's own self-test and CRC, boot and arithmetic; no
other emulator.

## Acceptance criteria

- [x] The 19BII boots to its normal display, 2 + 3 gives 5, the self-test's
  ROM step passes.
- [x] The skin works in both layouts; two headless-Chrome screenshots
  (1280, 390) in the PR, never cmux-cua or screencapture.
- [x] saturnus tx does not offer it; no HP logo or wordmark.
- [x] `hyalo lint` clean here and in the wiki.

## Process

- Targeted tests while working.
- Once before the PR: `just gates`, `just web-test`, `just rom-tests
  roms` (one suite at a time).
- One Opus reviewer (the two-chip code is 43a's and already had GLM); no
  second review round.
- At most three agents.

## Outcome

Done 2026-10-13, in one PR; wiki PR calculator-knowledgebase #16
(`hardware/hp19bii`, `sources/hp19bii-om`, `sources/commons-hp19bii-photos`,
the 19BII's row in `hardware/saturn-models` corrected).

- **ROM**: `roms/hp19bii.rom`, 131072 bytes, in dump order (the CRC and
  the "COPYRIGHT HP 198" text both tell); its halves end "OK-19B-1DK" and
  "OK-19B-2A". The owner's decision on running a class-b image stays open.
- **Map** (trace on a logging bus): the 28S's windows unchanged; RAM used
  only at #C4000-#C7FFF, so 8 KB repeating through the 28S's window. No
  CONFIG, no other device.
- **Display**: 137 x 32, not 132: text and the six 22-pixel menu labels
  span both chips' halves without a gap.
- **Keyboard**: the KML 2.0 table, every key pressed on the ROM; INPUT is
  `5 128`, not the table's `6 128`. New keys `ins`, `question`,
  `dollar`, `colon`.
- **Annunciators**: the 28S's slave words; word 0 is the appointment
  alarm (reported as `busy`, as on the Pioneers).
- **Validation** (goldens recorded from saturnus, no oracle): the
  language menu (`19bii-cold`), ENGLISH gives "MEMORY LOST" over 0.00
  (`19bii-memory-lost`), `2 + 3 =` gives 5.00 (`19bii-two-plus-three`),
  state round trip, shift/alpha/RAD lit, the TIME menu changes once a
  second, an appointment a minute ahead lights ((•)) (`busy`); the self-test passes
  with "OK-19B II" (`19bii-self-test`, about 7.6 s in).
- **Clock**: "Set the clock" by keys in TIME SET as on the 17BII (format
  nibble #C41E1, refused on the language prompt, nibble #C402B); goldens
  `clock-19bii`, `clock-19bii-dmy-24h`.
- **Identification**: "OK-19B II" (the self-test's pass message, kept
  whole in the ROM) names a 128 KB image.
- **Hosts**: CLI `--model 19bii`; `rom fetch` says there is no download
  and saturnus has no way to get this ROM; web model list, ROM by file,
  skin in both layouts (`skins/hp19bii.rs`, measured on two CC BY 2.0
  Commons photographs, labels from the manual's p. 22-23), landing card
  in the never-published row, desktop slot; saturnus tx and `satx`
  refuse it (no serial port). No MCP (retired in iteration 18).
- **Open**: keys act on release on every Lewis model, so MEM and SHOW,
  which the manual shows while held, never show (nor the 42S's key
  preview); the desktop app was not run by hand.
