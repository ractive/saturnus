---
type: iteration
title: "Iteration 35: A restartable tour of the page"
date: 2026-10-09
status: in-progress
tags:
  - iteration
  - saturnus
  - web
branch: iter-35/tour
---

# Iteration 35: A restartable tour of the page

Owner (2026-10-09): "It should not explode and overwhelm the user, but a
(restartable) tour through the app would be amazing." The design below
was agreed with the owner the same day.

## What it does

**Coach marks.** Each step rings one element of the page and puts a
short bubble beside it (one or two plain sentences), with Back, Next,
"Skip tour" and a counter ("3 of 7"). Esc ends the tour. The bubble is a
non-modal dialog with a label: it takes the focus when the tour starts
and gives it back to where it was when the tour ends. On a phone (below
760 px) the bubble is a sheet at the bottom. The ring only draws: no
backdrop, nothing on the page is blocked.

**Never forced.** On a first visit one quiet line at the bottom offers
"New here? Take a 2-minute tour." Its × dismisses it, and the dismissal
(or a tour taken) is kept (`saturnus.tour`). "Show me around" is an
action in Search and a button in About, so the tour starts again at any
time. Each chapter is also an action of its own ("Tour: The calculator"),
found by a search.

**Chapters**, each takeable alone; the last step of one offers the next:

1. **Getting started** (no ROM needed): the side panel (or the phone's
   ☰), the ROM list, Download (the browser: the hpcalc.org link; the
   app: Download…), Choose…, Search and its key, Keyboard shortcuts, and
   where the tour starts again.
2. **The calculator:** keys and typing, paste, Ctrl/Option-click for the
   shifted functions (a mouse only), Edit, Copy screen and Save screen,
   the memory view's button.
3. **The memory view** (offered while a 48SX, 48GX or 49G runs):
   Variables, the tree and changing directory, the preview's first button
   and editing a variable, the Stack tab, the Flags tab, where the keys go.

**The tour shows, it never changes the calculator.** No stores, purges
or key presses. It opens the panel, the ROM list, the memory view and
its tabs, and puts them back as they were when it ends.

**Data-driven steps** (`web/tour.js`): each step is `{id, anchor, title,
text, setup, when}`: a CSS selector, the words (`{key:palette}` becomes
the binding's label), a setup that only opens UI (`panel`, `roms`,
`calculator`, `layer:vars`, ...), and a condition on the host, the
pointer, a phone and a Mac. The list is plain data, for a later
click-through recorder.

**Robust.** A step whose anchor is missing, hidden or covered is
skipped (and leaves the count), never shown floating. A page test opens
every chapter's steps in light and dark, at 1280 and 390 px, in the
browser and a stubbed app host, and fails on any anchor not shown: a UI
change that breaks a step fails CI.

## Tasks

- [x] `web/tour.js`: chapters and steps as data, conditions, the run (skip, count), the offer's state
- [x] `web/components/sat-tour.js`: ring, bubble (dialog, focus, Esc), phone sheet, setup and restore, the offer line
- [x] Hooks: app.js (setups, "Show me around" and chapter actions), About's button, `data-tour` where a selector would be brittle
- [x] Styles: light and dark, phone sheet, reduced motion
- [x] Unit tests for the engine (`web/test/tour.test.mjs`)
- [x] Page tests (`web/test/tour-page.test.mjs`): the offer, palette and About starts, Back/Next/Skip/Esc, focus return, the phone sheet, every anchor shown
- [x] Screenshots of each chapter (light/dark, desktop/phone)
- [x] CHANGELOG, decision log, web/README.md
- [ ] Gates: `just gates`, `hyalo lint`
- [ ] Owner acceptance: the tour reads well and does not overwhelm
