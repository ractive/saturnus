---
title: "Iteration 8: Vector skins per model for the web UI"
type: iteration
date: 2026-10-05
status: planned
branch: iter-8/vector-skins
tags:
  - iteration
  - saturnus
---

# Iteration 8: Vector skins per model for the web UI

Read first: `kb/docs/clean-room-rule.md` (no HP logos or wordmarks in UI
chrome; "emulates the HP 48SX" as text is fine), `web/README.md`,
`crates/saturnus-web/src/layout.rs`, wiki `hardware/keyboard`,
`hardware/hp39g-40g` ("Alpha letters"), `hardware/hp38g`, `hardware/hp49g`,
`sources/hp39g40g-ug`, `sources/hp38g-ug`, `sources/hp48g-ug`, `sources/hp49g-um`.

## Context (2026-10-05)

- Decision (owner, 2026-10-05): skins are drawn, not photographed.
  Photographs are copyrighted whoever took them (Swiss law protects every
  photograph since 2020), so HP's product photos and other people's photos
  are out; the owner's own photos and Wikimedia Commons CC BY-SA photos may
  serve as references. The primary references are the keyboard line
  drawings in HP's user's guides (the 39G/40G figure on page 1-3 of the
  39G/40G guide, rendered at 400 dpi with `pdftoppm`, was legible down to
  the alpha letters; the 38G, 48G and 49G guides have the same kind of
  figure; the 48SX guide is on hp.com). Measure geometry, labels and
  colours from them and draw our own SVG; copy no pixel and no artwork.
- Facts to transcribe per model: key grid (rows, columns, widths, the
  wide ENTER), key cap colour, the shifted-label colours (48SX orange and
  blue, 48GX purple and green, 49G and 39G/40G per their guides), label
  text above and on each key, alpha letters, the LCD bezel and the
  annunciator row position. The HP logo is left off; the model name
  appears as plain text.
- The web UI today draws a generic button grid from `keys()` in
  `crates/saturnus-web/src/layout.rs` and `web/app.js`; the LCD is a canvas.
  A skin replaces the button grid with an SVG (or canvas-drawn) keyboard
  with the same hit areas and the same `Key` names, and frames the LCD.

## Tasks

- [ ] A skin description format (JSON or Rust data: key rectangles,
  labels, colours, LCD rectangle) with one file per model: 48SX, 48GX,
  38G, 49G, 39G/40G (40G = 39G with the CAS softkey label).
- [ ] Transcribe each model from its user's guide keyboard figure (render
  the figure as an image, measure, cross-check key count and labels
  against the wiki matrices; note each figure's source page).
- [ ] Render the skins in the web UI (SVG with clickable keys, shift and
  alpha labels, pressed-key feedback, the LCD and annunciators inside the
  bezel); keep the physical-keyboard mapping; a toggle back to the plain
  grid.
- [ ] Browser verification per model (headless Chrome via CDP as in
  iteration 6, or the Chrome extension): boot, press keys on the skin,
  screenshot into the scratchpad and reference the files in the Outcome.

## Acceptance criteria

- [ ] Each model boots and is fully operable from its skin by mouse; every
  key of the model's matrix is reachable; no HP logo or wordmark appears.
- [ ] The skins hold no third-party artwork or photographs; the decision
  log records the reference figures used.
