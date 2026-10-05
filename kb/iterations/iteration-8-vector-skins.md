---
title: "Iteration 8: Vector skins per model for the web UI"
type: iteration
date: 2026-10-05
status: completed
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

- Decision (owner, 2026-10-05): skins are drawn SVG, never shipped
  photographs. Photographs are copyrighted whoever took them (Swiss law
  protects every photograph since 2020), so no photo is redistributed. As
  references they are all fine, HP's product photos on hp.com included:
  measuring proportions, key shapes, colours and label placement from a
  photo copies none of its expression. Equally good references are the
  keyboard line drawings in HP's user's guides (the 39G/40G figure on page
  1-3 of the 39G/40G guide, rendered at 400 dpi with `pdftoppm`, was legible
  down to the alpha letters; the 38G, 48G and 49G guides have the same kind
  of figure; the 48SX guide is on hp.com). Use both: the drawing for
  geometry and labels, a photo for colours and the look of the case. Keep a
  list of the references used (URL or page) in the Outcome; store none of
  them in the repo.
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

- [x] A saturnus logo (owner's wish, 2026-10-05): a simple SVG of the
  planet with its ring, our own design, flat and legible at 16 px; not
  resembling the Saturn car brand's red ring or the Sega Saturn mark. It
  goes where the HP logo sits on each skin's bezel, becomes the web page's
  favicon and heads the README.
- [x] A skin description format (JSON or Rust data: key rectangles,
  labels, colours, LCD rectangle) with one file per model: 48SX, 48GX,
  38G, 49G, 39G/40G (40G = 39G with the CAS softkey label).
- [x] Transcribe each model from its user's guide keyboard figure (render
  the figure as an image, measure, cross-check key count and labels
  against the wiki matrices; note each figure's source page).
- [x] Render the skins in the web UI as SVG (clickable keys, shift and
  alpha labels, pressed-key feedback, the LCD and annunciators inside the
  bezel); keep the physical-keyboard mapping; a toggle back to the plain
  grid.
- [x] Browser verification per model (headless Chrome via CDP as in
  iteration 6, or the Chrome extension): boot, press keys on the skin,
  screenshot into the scratchpad and reference the files in the Outcome.

## Acceptance criteria

- [x] Each model boots and is fully operable from its skin by mouse; every
  key of the model's matrix is reachable; no HP logo or wordmark appears;
  the saturnus logo does.
- [x] The skins hold no third-party artwork or photographs; the decision
  log records the reference figures used.

## Outcome (2026-10-05)

Done: five drawn skins (the 40G reuses the 39G drawing with its own
name), the saturnus logo, the SVG skin in the web UI with a toggle back
to the grid, and a browser check per model. The skins are Rust data in
`crates/saturnus-web/src/skins/`, one file per model, sent to the page
as JSON by `skin(model)` and `Emulator.skin()`. A unit test checks each
skin against the model's key matrix: every key drawn once, nothing
extra, no overlaps. Further tests check that the alpha letters agree
with the ROM (39G/40G) and the wiki (48, 49G, 38G).

### References used

Measured, never copied; none of them is in the repo.

| Model | Geometry and labels | Colours |
| --- | --- | --- |
| 48SX | HP 48SX Owner's Manual vol. 1, p. 26 (PDF p. 30, 300 dpi), <https://literature.hpcalc.org/items/378> (`hp48sx-om-vol1-en.pdf`); display and case top: HP 48G Series User's Guide p. 1-9 (`raw/manuals/hp48gug.pdf`, PDF p. 23) | the same manual's cover photo (PDF p. 1) |
| 48GX | key grid as the 48SX; display from 48G guide p. 1-9; grid cross-checked on 48G guide pp. 1-5 and 2-3 (PDF pp. 19, 27); labels from the photo | "Hewlett-Packard 48GX Scientific Graphing Calculator.jpg", <https://commons.wikimedia.org/wiki/File:Hewlett-Packard_48GX_Scientific_Graphing_Calculator.jpg>; 48G guide cover photo (PDF p. 1) |
| 38G | HP 38G User's Guide inside cover (`raw/manuals/hp38g-ug-en.pdf`, PDF p. 2, 400 dpi) | "HP-38G scientific graphing calculator (edited, without background).JPG", Wikimedia Commons |
| 49G | HP 49G User's Manual figure 1.1, p. 1-2 (`raw/manuals/hp49g-um-en.pdf`, PDF p. 16, 400 dpi); label colours from its p. 1-3 text | "HP49G.jpg", <https://commons.wikimedia.org/wiki/File:HP49G.jpg> |
| 39G, 40G | HP 39G/40G User's Guide p. 1-3 (`raw/manuals/hp39g40g-ug-en.pdf`, PDF p. 13, 400 dpi) | Thimet's HP-39G photo, <https://www.thimet.de/CalcCollection/Calculators/HP-39G/Contents.htm> (`HP-39G-M.JPG`) |

### Unit and method

Figures rendered with `pdftoppm`. Key outlines were found as connected
components of dark pixels (filled shapes on the 38G figure), case and
window edges from pixel profiles along rows and columns. Unit: 1/100 of
the figure's menu-key pitch (48SX 142.2 px, 48G p. 1-9 782.4 px, 38G
139.3 px, 49G 131.4 px, 39G 92.6 px). Origin: the case's top-left corner.
Each model file names its figures and lists what is inferred.

### Inferred

- The 48 case below the ON row: the figures stop at the keys, so 200
  units were taken from the 48GX photo.
- The 48SX figure's case is 645 units wide, the 48G p. 1-9 figure's 667.
  The keys are centred in the 667-unit case.
- All shades (read by eye off photographs). The 39G's shifted-label colour
  is near white on the photo; Thimet calls it blue.
- The 49G case is a rounded outline without its side grips, and the
  small glyphs around its cursor pad are left off.
- The 40G: no figure or photo was found. It is drawn as the 39G with its
  name. Its CAS appears only as a menu label on the LCD, drawn by the ROM,
  so nothing is printed on the case.
- The 48GX's lighter tiles behind the number keys follow the photo
  approximately.

### Browser verification

Headless Chrome 154 over CDP. The Chrome extension's window was hidden,
so `requestAnimationFrame` did not run there. `web/` was served on
127.0.0.1:4860, the viewport was 1100 x 1320 at 2x, and the ROM was set on
the file input. Each model booted, its first screen was answered by mouse
on the drawn F key, and an entry was typed by mouse on the skin: 48s
`6 ENTER 7 ×`, others `6 × 7 ENTER`. Every screen shows 42. Every press
showed the pressed-key state. The console was clean (no messages,
exceptions or log entries) in all runs. Hit-testing the centre of every
drawn key returned that key on all six models (49, 49, 47, 51, 51, 51
keys). On the 40G, `8 * 9 Enter` on the computer keyboard gave 72 on
the skin. Dark mode changed only the page, and the grid toggle went
there and back. At 390 px wide the skin filled the width with no
horizontal scroll. At 1280 x 720 the LCD stayed at an integer 2x.

Screenshots (scratchpad, not in the repo):

- 48SX: `/private/tmp/claude-501/-Users-james-devel-saturnus/92b88cf2-5ffa-4c95-ba06-e64134673bb8/scratchpad/skins/skin-48sx-1-boot.png`, `-2-answered.png`, `-3-arith.png`
- 48GX: `/private/tmp/claude-501/-Users-james-devel-saturnus/92b88cf2-5ffa-4c95-ba06-e64134673bb8/scratchpad/skins/skin-48gx-1-boot.png`, `-2-answered.png`, `-3-arith.png`
- 38G: `/private/tmp/claude-501/-Users-james-devel-saturnus/92b88cf2-5ffa-4c95-ba06-e64134673bb8/scratchpad/skins/skin-38g-1-boot.png`, `-2-answered.png`, `-3-arith.png`
- 49G: `/private/tmp/claude-501/-Users-james-devel-saturnus/92b88cf2-5ffa-4c95-ba06-e64134673bb8/scratchpad/skins/skin-49g-1-boot.png`, `-2-answered.png`, `-3-arith.png`
- 39G: `/private/tmp/claude-501/-Users-james-devel-saturnus/92b88cf2-5ffa-4c95-ba06-e64134673bb8/scratchpad/skins/skin-39g-1-boot.png`, `-2-answered.png`, `-3-arith.png`
- 40G: `/private/tmp/claude-501/-Users-james-devel-saturnus/92b88cf2-5ffa-4c95-ba06-e64134673bb8/scratchpad/skins/skin-40g-3-arith.png`, `-4-dark-keyboard.png`,
  `-5-grid-dark.png`, `-6-phone.png`

### Left open

- No 40G figure or photo; its case is assumed identical to the 39G's.
- The skin is a measured drawing, not a photo-accurate one: label fonts,
  the shift labels around the 49G cursor pad, and case bevels are
  simplified.
