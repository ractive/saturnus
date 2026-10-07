---
type: iteration
title: "Iteration 24: Skin depth pass (cases with a natural 3D touch, the 42S keys)"
date: 2026-10-07
status: planned
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

- [ ] Case depth for all seven skins, one shared rendering rule with
  per-skin colours and textures.
- [ ] 42S key pass.
- [ ] Before/after screenshots per model, light and dark, desktop and
  phone width, side by side with the photographs; listed in the Outcome.
- [ ] `just gates`; the skin tests (alignment, margins) still pass.

## Acceptance criteria

- [ ] The owner looks at the screenshots and the live page and finds the
  cases less flat and the 42S keys on a par with the 48SX's.

## Outcome

(to be written)
