---
type: iteration
title: "Iteration 50c: the calculator as a mountable view"
date: 2026-10-17
status: completed
tags:
  - iteration
  - web
branch: iter-50c/calculator-view
---

# Iteration 50c: the calculator as a mountable view

The emulator's start code becomes a module the one document can mount.
Still served as `index.html`.

## Design

- `web/app.js` becomes `mountCalculator(root, {shown})`.
- The page shell moves off `body`: the body grid (`style.css:13-19`) and
  the `body.*` selectors (`:19`, `:33`, `:268`, `:436`, `:466`, `:1664`)
  go onto the view root `#calculator-view`.
- Every document- or window-level handler checks `shown()` first: keys
  and paste (`sat-calculator.js:482-484`, `:1211`), `sat-palette.js:187`,
  `app.js:834`, `sat-shortcuts`, `sat-tour`, ROM drop
  (`sat-controls.js:620-629`), `resize`, `pagehide`.
- Views are hidden, not unmounted (as in
  [[iterations/iteration-50b-files-view]]); a hidden calculator keeps
  running.

## Tasks

- [x] `mountCalculator(root, {shown})`; `index.html` a thin host
- [x] Shell off `body`; handlers gated on `shown()`
- [x] The emulator page tests green

## Acceptance

- The emulator works as before; nothing runs at import.
