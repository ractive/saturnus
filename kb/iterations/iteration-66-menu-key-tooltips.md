---
type: iteration
title: "Iteration 66: the menu keys' tooltips name what the screen shows, and the self-tests in the ON menus"
date: 2026-10-25
status: completed
tags:
  - iteration
  - web
  - skins
branch: iter-66/menu-key-tooltips
---

# Iteration 66: the menu keys' tooltips name what the screen shows, and the self-tests in the ON menus

Owner 2026-10-25: a menu key's tooltip (F1–F6 on the 49G, the six
unlabelled keys A–F on the 48SX and 48GX) names the command the screen
shows above it, with its full name from the command reference: EXPA shows
"EXPAN · Expand products". 48SX, 48GX and 49G first; the other models are
researched separately (see [[research/menu-tooltips-other-models]]).

## How

- The current menu and its page are read from the calculator's memory,
  without pressing keys (the key sweeps of 2026-10-25 read them; the
  address and its meaning go into the wiki if they are not there yet).
- The menu's definition comes from the ROM, as the command reference
  already reads it (`crates/saturnus-objects/src/menus.rs`).
- The full name comes from `web/commands.json`, per model.
- A menu of variables (VAR, CST, a directory) names the variable and its
  type instead; an input form or choose box gives its own labels (OK,
  CANCEL, CHOOS) or none. Where the menu cannot be read, the tooltip stays
  as it is (the key's name).
- The tooltip follows the screen: it is rebuilt when the menu changes, not
  on every frame.

## Also: self-tests in the ON menus

The wiki now says how to leave each self-test (calculator knowledgebase
PR 23, pages `hardware/*-keyboard`). The ON menu's Self-test entry, left
out in iteration 65 for the 28S, 38G, 39G, 40G, 27S, 17B, 17BII and 19BII,
goes in with a note how to leave it (on the 39G/40G the continuous tests
need ON+C held about 2 s).

## Tasks

- [x] Read the current menu and page on the 48SX, 48GX and 49G (memory only), with a host test on each ROM
- [x] Menu labels from the ROM's menu definitions, full names from commands.json
- [x] Menu-key tooltips follow the screen; variables, forms and choose boxes handled
- [x] Self-test entries in the ON menus of the other models, with how to leave them
- [x] Tests (host, unit, page) and screenshots of a tooltip on each of the three models
- [x] Owner: hover the menu keys in a few menus (MTH, a directory, an input form) on the 48GX and 49G
