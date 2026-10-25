---
type: iteration
title: "Iteration 67: menu-key tooltips on the 42S, 27S, 17B, 17BII, 19BII and 28S"
date: 2026-10-25
status: planned
tags:
  - iteration
  - web
  - skins
branch: iter-67/menu-key-tooltips-42s-pioneers
---

# Iteration 67: menu-key tooltips on the 42S, 27S, 17B, 17BII, 19BII and 28S

Iteration 66's menu-key tooltips (the command above each menu key, with
its full name) on the 42S, the four Pioneers and the 28S. Owner
2026-10-25: go, after 66; the 28S joins this iteration. Background and addresses: `kb/research/menu-tooltips-other-models.md`.
The 38G, 39G and 40G stay out (view actions with plain names, no source
for full names).

## How

- Each of these ROMs keeps the six label texts in RAM, 11 nibbles per key
  (a length nibble, up to 5 characters, low nibble first): #5033C on the
  42S, #503D2 on the 17BII, #503D1 on the 17B, #C4376 on the 19BII, #503DF
  on the 27S (emulator observations, 2026-10-25; they go into the wiki
  first, with the 42S's menu depth at #50201).
- The buffer is not cleared when the menu goes away: a label is shown only
  while the LCD draws the label boxes (the check `labelsShown` in
  `web/keys28.js` and refgen's `has_menu` already do this).
- Full names and descriptions from `keyModels` in `web/commands.json`. A
  label that means different things in different menus is told apart by
  the other labels in the row.
- The 28S keeps no reliable label texts in RAM: its menu number is
  at address #C0151 (ARRAY 1, REAL 6, STACK 7, LOGS 17, TRIG 21, USER 23) and the
  first label shown at #C0154 (0, 6, 12). A table of menu number →
  labels in order, generated from the ROM by refgen as the reference data
  is, checked against the reference manual for a few menus, gives the
  labels; full names from the 28S entries of `commands.json`; the USER
  menu shows variables, as on the 48.
- The 17B, 17BII and 19BII in another language: labels that do not match
  the English reference fall back to the key's own tooltip.

## Tasks

- [ ] The label buffers and the 42S's menu depth in the wiki, per model; the 28S's menu number and first label too
- [ ] Read the labels (memory only) and check the label boxes are drawn
- [ ] Tooltips with full names from keyModels; duplicates told apart; other languages fall back
- [ ] Host tests per ROM for menus not yet checked: SOLVE variable menus, CFLO and SUM lists, the 19BII's TEXT menus, the 42S's variable and program menus
- [ ] The 28S: refgen's menu table, the tooltips while its label boxes are drawn, host tests for a few menus and pages
- [ ] Owner: hover the menu keys in a few menus on the 42S, the 17BII and the 28S
