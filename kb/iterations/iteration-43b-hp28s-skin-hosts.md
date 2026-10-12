---
title: "Iteration 43b: HP 28S clamshell skin and hosts"
type: iteration
date: 2026-10-12
status: planned
branch: iter-43b/hp28s-skin-hosts
tags:
  - iteration
  - saturnus
  - web
---

# Iteration 43b: HP 28S clamshell skin and hosts

Read first: [[iterations/iteration-43a-hp28s-machine]] (its outcome),
[[iterations/iteration-24-skin-depth]], web/README.md "Skins",
`kb/docs/clean-room-rule.md` (no HP logos), `kb/docs/test-policy.md`;
wiki `hardware/hp28s` (written in 43a).

## Goal

The 28S in every host where a calculator runs: a drawn clamshell skin as
good as the existing ones, the web emulator (model list, ROM by file,
137x32 canvas, annunciators, saved states), the desktop app and MCP.
saturnus tx lists it as not transferable.

## The clamshell: how it is shown (decision)

The 28S opens like a book: the left half carries the letter keyboard
(A-Z and a few symbols, SPACE, LC, ALPHA), the right half the display,
the menu keys, the function keys and the number pad (KML 2.0's 28S table
splits its keys the same way; the halves' contents to be confirmed
against the Owner's Manual figure).

**Choice: draw the open clamshell, both halves, as on the desk.** On a
wide stage the halves stand side by side with the hinge between them
(left: letters, right: display and keys), one drawing. On a portrait
stage (phones) the same skin data is laid out stacked: the display half
on top, the letter half below, the hinge as a strip between them, so the
keys keep a usable size at 390 px wide. The skin data defines each half
as a group with its own origin; the page picks side by side or stacked by
the stage's aspect ratio. Edge-to-edge fullscreen (iteration 22) drops
the case and keeps both faces. Alternatives rejected: the keyboard half
alone next to a separate display (not what the calculator looks like); a
closed-case view (nothing to press). Record in the decision log; the
owner can overrule in the PR.

## What exists to reuse

- `crates/saturnus-host/src/skins/mod.rs`: the skin model (panels with
  `relief`, `texture`, caps and wells, label bands) and its unit tests
  (every matrix key drawn once, no overlap, menu labels over menu keys,
  the case ending below the last row). A skin of two groups is new: the
  tests run per half.
- `crates/saturnus-host/src/skins/hp42s.rs` and `hp39g.rs`: the two ways
  existing skins were made (owner's photo; manual figure plus a
  reference photo).
- `web/components/sat-calculator.js` (`drawDefs`, `drawPanel`,
  `drawEdge`, the annunciator glyph sets `ANNUNCIATORS_42S`),
  `web/screenshot.js` (annunciator strips), `web/style.css` (`.skin
  .glass`, `[data-logo]`), `web/edge.js`.
- The 42S's host gating (see [[iterations/iteration-42-pioneer-27s-17b]]
  "What exists to reuse") and the `lcd_width()` binding from 43a.

## Facts needed

| Fact | Where | State |
| --- | --- | --- |
| Key positions and labels, shifted labels, both halves | HP 28S Owner's Manual keyboard figures, measured in pixels as the 39G's | research (published manual, fetched into the wiki's `raw/` through the manifest) |
| Colours: case, keys, shift key, label inks | published photographs (Wikimedia Commons, HP Museum) | research |
| Display window: 137x32, pixel aspect, bezel | 43a's `hardware/hp28s`; photographs | partly known |
| Annunciators and their glyphs | 43a's research | from 43a |

## Skin

Rust data in `crates/saturnus-host/src/skins/hp28s.rs`, drawn with the
existing rules (one light from the top left, wells, relief, texture).
Geometry and labels from the manual's figures; colours from published
photographs as medians, toned so every label keeps 4.5:1; nothing from a
photo or figure stored or reproduced. No HP logo or wordmark: the
saturnus logo where the HP logo was, "28S" and the case lettering as
text. Logo colours in `[data-logo="28s"]` (web and landing), checked by
`web/test/logo.test.mjs`. The header comment names the figures and photos
and what is inferred.

## Hosts

- **Web emulator**: the 28S in the model list; ROM by file only (Choose
  ROM…, drop); `web/norom.js` says the ROM must be read out of one's own
  28S (IR to an HP 48, wiki: sources/emu42-pioneer-dump CLAMSHEL); the
  canvas takes its width and height from the model (137x32); the 28S
  annunciator strip; `MODEL_CODES`; letters typed from the computer
  keyboard go to the letter keys directly (the 28S has them), so its
  skin carries typing data (unlike the 42S).
- **Desktop app**: the ROM slot with no download (`roms.rs`), the
  self-test ROM list, the clamshell skin in the same component.
- **MCP**: `boot` takes `28s`; `type_text` types letters through the
  letter keys; semantic Kermit tools answer "no Kermit server on this
  model". The 28S has a stack, so `stack` reading could apply later: out
  of scope here (its object format is not the 48's).
- **saturnus tx**: not transferable. The 28S has no serial port and no
  Kermit, only the IR printer; the transfer page does not offer it
  (`web/tx/emulator.js`, `copy_plan_json`).
- **Landing page**: a card and a ROM-gated shot (`web/landing/shots.mjs`)
  if the owner wants it.

## Tasks

- [ ] Research: the 28S manual's keyboard figures and published photos
  into the wiki (`raw/` manifest entries, a source page), the key labels
  per half on `hardware/hp28s`.
- [ ] Skin: `hp28s.rs` with two halves, the side-by-side and stacked
  layouts, the skin unit tests per half; the page's layout choice by
  aspect ratio.
- [ ] Web: model list, ROM by file, no-ROM message, 137x32 canvas
  (`lcd_width()`), annunciators, saved states, computer-keyboard letters.
- [ ] Desktop app and MCP gating as above; saturnus tx refuses it.
- [ ] Docs: web/README.md "Skins" (the clamshell and its two layouts),
  decision log (the clamshell choice), CHANGELOG.
- [ ] Owner: the clamshell layout on the desktop and on the phone (the
  PR's screenshots, then the deployed preview on your phone).
- [ ] Owner: the skin's look against your memory and published photos.
- [ ] Owner: landing page card yes or no.

## Tests

- Host unit tests: the skin (per half), the layout switch (side by side
  above an aspect ratio, stacked below).
- Web, ROM-gated: the 28S boots from a chosen file, mouse clicks on the
  drawn 2, ENTER, 3, + give 5; typing letters from the computer keyboard
  puts them on the command line; a saved state restores.
- Screenshots: headless Chrome only (never cmux-cua or screencapture),
  two: 1280 wide (side by side) and 390 wide (stacked), in the PR.

## Acceptance criteria

- [ ] The 28S runs in the web emulator and the desktop app from a chosen
  ROM file, operable by mouse, touch and computer keyboard.
- [ ] The clamshell shows side by side at 1280 px and stacked at 390 px,
  no horizontal scroll, keys at least the size of the 42S's at 390 px.
- [ ] No HP logo or wordmark anywhere; saturnus tx does not offer the
  28S.
- [ ] `hyalo lint` clean.

## Process

- Targeted tests while working (the skin tests, the touched
  `web/test/*.test.mjs`).
- Once before the PR: `just gates`, `just web-test`, `just rom-tests
  roms` (one suite at a time).
- One Opus reviewer; no second review round.
- At most three agents.
