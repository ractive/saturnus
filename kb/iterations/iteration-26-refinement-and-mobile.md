---
type: iteration
title: "Iteration 26: Refined look and feel, and every view checked on phones"
date: 2026-10-08
status: in-progress
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

- [x] Design tokens and the refinement pass across all components.
- [x] Mobile audit: a screenshot matrix (each view x 360/390/430/768 px,
  light and dark) before; the fixes; the same matrix after.
- [x] The command palette as a phone sheet.
- [x] Headless Chrome with touch emulation: no horizontal overflow on any
  view at any width (a script that checks scrollWidth on each view), the
  palette usable with the on-screen keyboard open (simulated viewport
  height).
- [x] `just gates`.

## Acceptance criteria

- [ ] The owner finds the page more refined and every view usable on a
  phone, from the screenshot matrix and the live page.

## Outcome

Screenshots in the session's scratch directory
`/private/tmp/claude-501/-Users-james-devel-saturnus/92b88cf2-5ffa-4c95-ba06-e64134673bb8/scratchpad/iter26/`
(not committed; `matrix.mjs` took them in headless Chrome with touch
emulation at 2x, the 48SX booted from its ROM with three variables and
a stack put in through the protocol's `run`, `sheet.py` composed the
sheets). Look first at the side-by-sides, before over after, light and
dark: `side-390-a.jpg` (calculator, controls sheet, ROM slots, Variables,
Commands at 390 px), `side-390-b.jpg` (the palette with a query, the
palette with the keyboard up, Keyboard shortcuts, About) and
`side-768.jpg` (the same at tablet width); then `after-360.jpg` (every
view at the narrowest width), `after-palette-widths.jpg` and
`after-vars-widths.jpg` (one view across 360/390/430/768),
`after-390-stack.jpg` (Variables, Stack and Flags with memory in them). The full
matrix is `{before,after}/<view>-<width>[-dark].png` for the views
`calc sheet roms vars stack flags commands palette palette-q palette-kbd
shortcuts about` (plus `norom-*` without a ROM and `land-*` in
landscape); `audit.json` beside them holds each shot's horizontal
overflow, sideways scrollers and the controls under the target size.

What changed (decision log, "iteration 26"): the tokens at the top of
`style.css` and the refinement pass over every component (type scale,
spacing rhythm, three elevation levels, one focus ring, hover tints,
transitions that go to zero under `prefers-reduced-motion`, empty and
error states with one layout, About with a head and sections, dark mode
from the same tokens); one icon set as an SVG sprite; the phone bar as
icons; the palette as a sheet with the entry as a second step and the
height of the visual viewport; About and the shortcuts as full sheets;
44 px controls and 40 px rows under `pointer: coarse`; safe-area insets
on the bar, the stage, the sheets and the dialogs; the Speed radio
group's keyboard; the stage tools no longer drawn twice at phone width.

Measured: before, the page was 8 px wider than a 360 px viewport on
every view (the bar's labelled buttons), and the Commands and Memory
buttons were drawn twice below 760 px; after, no view at 360, 390, 430,
768 or 1280 px, light or dark, is wider than its viewport, and the only
container that scrolls sideways is the Commands tab's menu tree (by
design, 100 px wide on a phone). Every button, select, tab, field and
link is at least 44 px on coarse pointers; rows are 40 px (deliberately
under 44: a tree of thirty menus at 44 px would be a screen and a half).
The palette sheet with a 336 px keyboard simulated at 390 x 844 keeps
the input and the list inside the remaining 508 px, and a tapped row
opens its entry with the way back at the top and the buttons at the
bottom (`web/test/overflow.test.mjs` asserts all of this; it ran here in
41 s). A key press still paints at the frame rate at 2x: 30 presses on
the 48SX, 42S and 49G, median 16.7 ms, p95 16.7-16.8 ms, max 16.8 ms
(`perf.mjs`, as iteration 24 measured). `just gates` passes in the
worktree (after merging main's b4ec1ca, which fixed iteration 24's kb
lint error); the web tests gained `radiogroup.test.mjs` and
`overflow.test.mjs`.

Still open (for the owner's eye): the Commands tab's split (menu tree
beside the list) is cramped at 360 px and could stack; the drawn keys of
the 48SX are about 35 px wide at 390 px, which is the calculator's own
geometry (iteration 22's edge-to-edge fullscreen is the answer there);
the keyboard shortcuts dialog is reduced on phones (stacked rows, hints
hidden) but not hidden: a phone with a keyboard is rare but real.
