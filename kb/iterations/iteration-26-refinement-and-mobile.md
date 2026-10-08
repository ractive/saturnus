---
type: iteration
title: "Iteration 26: Refined look and feel, and every view checked on phones"
date: 2026-10-08
status: planned
tags:
  - iteration
  - saturnus
branch: iter-26/refinement-and-mobile
---

# Iteration 26: Refined look and feel, and every view checked on phones

**For a Fable agent** (design work). After iteration 24 (skin depth),
which touches the same stylesheet. "App" means the web page and the
desktop app alike.

## Context (owner, 2026-10-08)

"You may want to take some time with Fable to spice up the look and feel
of the web app. I like the general style/theme, but it could be a bit
more refined." And: "Check the responsiveness of the page. E.g. the
command palette doesn't look good in a mobile view. Check also the other
things in mobile view."

## Design goals

- **Refinement, not a new style**: keep the theme (warm neutral
  background, the teal accent, the calm panels); refine typography
  (scale, weights, line lengths, numerals), spacing rhythm, borders and
  shadows (fewer, softer, consistent elevation levels), focus and hover
  states, transitions (short, purposeful, `prefers-reduced-motion`
  respected), icons (one consistent set, inline SVG), empty and error
  states, the About page's layout, and dark mode parity. A small set of
  design tokens (colours, radii, spacing, elevation, motion) in one place
  in `web/style.css`.
- **Every view on phones**: audit the whole page at 360, 390 and 430 px
  wide, portrait and landscape, and at tablet width: the side panel, the
  memory/commands layer and its tabs, the command palette (today it does
  not work well on a phone: a full-height sheet with the input at the
  top, the list scrolling, the entry as a second step instead of beside
  the list, the on-screen keyboard not covering the list), the keyboard
  shortcuts dialog (hidden or reduced where there is no keyboard), the
  ROM slots, the no-ROM message, About, fullscreen. Touch targets at
  least 44 px; no horizontal scroll; safe-area insets.
- Coordinate with [[iterations/iteration-22-installable-web-app]]: this
  iteration owns layout and look on phones; 22 owns the manifest,
  offline cache, lifecycle and the edge-to-edge fullscreen and palette
  touch triggers. Whichever runs second builds on the other.

## Tasks

- [ ] Design tokens and the refinement pass across all components.
- [ ] Mobile audit: a screenshot matrix (each view x 360/390/430/768 px,
  light and dark) before; the fixes; the same matrix after.
- [ ] The command palette as a phone sheet.
- [ ] Headless Chrome with touch emulation: no horizontal overflow on any
  view at any width (a script that checks scrollWidth on each view), the
  palette usable with the on-screen keyboard open (simulated viewport
  height).
- [ ] `just gates`.

## Acceptance criteria

- [ ] The owner finds the page more refined and every view usable on a
  phone, from the screenshot matrix and the live page.

## Outcome

(to be written)
