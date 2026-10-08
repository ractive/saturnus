---
type: iteration
title: "Iteration 24: Skin depth pass (cases with a natural 3D touch, the 42S keys)"
date: 2026-10-07
status: in-progress
tags:
  - iteration
  - saturnus
branch: iter-24/skin-depth
---

# Iteration 24: Skin depth pass (cases with a natural 3D touch, the 42S keys)

**For a Fable agent** (design work; wait until Fable usage is available
again: owner, 2026-10-07).

Read first: `kb/iterations/iteration-10-web-design.md` (Outcome: the key
relief, wells and softkey borders), `crates/saturnus-host/src/skins/`
(skins as Rust data) and the renderer in `web/components/sat-calculator.js`,
the owner's photographs in `~/Downloads/HP Taschenrechner/` (48SX, 38G,
49G, 42S; references only, never shipped).

## Context (owner, 2026-10-07)

"Try to give the calculator cases a bit more of a natural touch? It now
looks pretty flat. Maybe a slight 3D touch like you did with the keys?"
and "The keys of the 42S look ok-ish but not so good as e.g. the 48SX
ones."

## Design goals

- **Cases**: a subtle physical feel without kitsch: a soft bevel or
  rounded edge where the case meets the keyboard plate, a gentle light
  gradient across the case (top-left light, as the keys already use), the
  display window sunk slightly into the bezel (inner shadow), a faint
  material texture where the real case has one (the 48SX's textured
  plastic, the 49G's smooth blue) judged against the photographs; the
  same light direction as the keys. Must stay crisp at every size,
  cheap to draw (CSS gradients or SVG filters, no bitmaps), and readable
  in dark mode.
- **42S keys**: bring them to the 48SX's level: proportions, relief,
  legend placement and colour of the shifted labels, the gold shift key,
  the wells, compared side by side with the 42S photographs.
- All seven skins checked for consistency of the new depth; the
  edge-to-edge fullscreen of iteration 22 (if merged) keeps working.

## Tasks

- [x] Case depth for all seven skins, one shared rendering rule with
  per-skin colours and textures.
- [x] 42S key pass.
- [x] Before/after screenshots per model, light and dark, desktop and
  phone width, side by side with the photographs; listed in the Outcome.
- [x] `just gates`; the skin tests (alignment, margins) still pass.

## Acceptance criteria

- [ ] The owner looks at the screenshots and the live page and finds the
  cases less flat and the 42S keys on a par with the 48SX's.

## Outcome

Screenshots in the session's scratch directory
`/private/tmp/claude-501/-Users-james-devel-saturnus/92b88cf2-5ffa-4c95-ba06-e64134673bb8/scratchpad/iter24/`
(not committed; `shots.mjs` and `perf.mjs` there took them in headless
Chrome over the DevTools protocol, every model booted from its ROM).
Look first at the side-by-sides against the photographs:
`side-42s-keys.jpg` (photo, before, after) and `side-48sx-top.jpg`; then
`after-sheet.jpg` (all seven skins at 2x) against `before-sheet.jpg`,
and the details `after-{48sx,42s,38g,49g,39g}-top.jpg`,
`after-42s-keys.jpg`.

Per model, light and dark, desktop (1280 x 900) and phone (390 x 844 at
2x): `{before,after}/{tag}-{model}-desktop.png`, `-desktop-dark.png`,
`-phone.png`, `-phone-dark.png`, `-skin.png` (the skin alone at 2x);
`after-48sx-norom.png` and `-norom-dark.png` show the "Choose ROM…"
state over the LCD (which the before shots showed missing: a stray
`</section>` in the template, fixed).

The shared rule (decision log, "Skin depth"): the body lit from the
top left, its contact shadow, a rounded outer edge, a grain where the
plastic has one; every other panel flat, raised or sunk by the skin's
new `relief`; the glass sunk into its bezel by a CSS inset shadow on a
`.glass` element over the canvas. Per model: the 48SX and 48GX keep
their textured plastic (texture 10) with the plates sunk in the rim;
the 38G's upper face raised, its bezel and keyboard plate sunk
(texture 8); the 49G's face plate raised and its glossy black surround
lit along the top, smooth (texture 0); the 39G/40G's face raised, the
surround sunk (texture 4); the 42S's plates raised in its dark recess
(texture 9), and its keys redone: a 6-unit black skirt under every cap
drawn as its well, darker caps, the photo's saturated orange for the
shifted labels, the glass in the LCD's colour inside a bezel of the
photo's proportions (12 units at the sides, 18 above and below).

Measured: a key press still paints at the frame rate with the grain
(30 presses on the 48SX, 42S and 49G at 2x: median 16.7 ms, p95
16.8 ms); the stage does not scroll from the shadow; `.no-rom` is inside
`.skin`. `just gates` with the ROMs passes; the skin tests gained
`reliefs_and_textures` and the 42S skirt check.

Still open (for the owner's eye): the case edge is deliberately
subtle at desktop size and reads best at 2x; the 42S's label weight is
the page's 600, heavier than the unit's print; the 48's display frame
could take a bevel of its own.
