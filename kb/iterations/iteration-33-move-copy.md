---
type: iteration
title: "Iteration 33: Copy to… and Move to…"
date: 2026-10-09
status: in-progress
tags:
  - iteration
  - saturnus
  - explorer
branch: iter-33/move-copy
---

# Iteration 33: Copy to… and Move to…

Owner (2026-10-09): variables and directories can be copied and moved
to another directory from the memory view's menus. Menu items only, no
drag and drop.

## What it does

- **"Copy to…" and "Move to…"** in the "⋯" menu and the context menu of
  an object or a directory (the one selected in the list, or the one
  shown in the tree), beside Rename. Not for HOME, not for a stack
  level, not without writes; off while a write runs, like the others.
- Each opens a **directory picker** in the preview: every directory as
  a tree, the arrows, Home and End to move, Enter (or a double click, or
  "Move here"/"Copy here") to choose, Escape to cancel. The directory
  the variable is in cannot be chosen; for a directory, neither can the
  directory itself nor anything in it. Those rows can still have the
  focus and say why in their title.
- **A name taken in the target** asks first, as Purge does: "Replace X
  in DATA?", with Cancel focused (No by default). A directory is never
  replaced, and a directory never replaces a variable: the picker says
  "DATA has a directory called X. Purge or rename it first."
- After it, the message "Moved X to HOME › DATA" (or "Copied …"), the
  tree and the list follow; after a move the view shows the target with
  the moved variable selected.

## How the calculator does it

One hidden Kermit transaction, as the other writes (`copy` and `move`
in `web/protocol.md`), one host command per packet:

1. `HOME A` (the source), `'X' RCL`, `HOME B` (the target), `'X' STO`:
   the calculator's own copy, a directory with everything in it. No file
   touches the computer.
2. A move then checks the copy: `'X' RCL BYTES` in the target and in the
   source, `ROT == 3 ROLLD == AND`, and only when size and checksum agree
   `« 'X' PURGE » « "Copy differs" DOERR » IFTE` (`PGDIR` for a
   directory). Any failure stops there: the original stays (a copy in
   the target is possible, a loss is not), and the levels a failed
   command left are dropped as for every write.
3. The calculator goes back to its current directory.

Refused before anything runs: the same directory, a directory into
itself or a directory inside it, a taken name without `replace`, a
directory to replace or to replace with, moving the directory that holds
the current one. The calculator's own errors (`Insufficient Memory`)
are the reply's.

`saturnus ctl cp NAME HOME/B` and `ctl mv` (`--dir`, `--replace`), and
`POST /v1/memory` with `copy` and `move`.

## Tasks

- [x] Engine: `Op::Copy` (`copy_plan`), refusals, the move's check on the calculator; unit tests
- [x] Protocol commands `copy`, `move` (all hosts, `WRITE_COMMANDS`); HTTP `/v1/memory`; `ctl cp`, `ctl mv`
- [x] ROM tests on the 48SX, 48GX and 49G: copy, replace, move, a directory moved with its contents, into itself refused, no memory keeps the original
- [x] Web: menu items, the picker, the replace question, the message and the selection after a move; tests in `web/test/actions.test.mjs` and `web/test/explorer.test.mjs`
- [x] `web/protocol.md`, README, CHANGELOG
- [ ] Owner: Copy to… and Move to… on a real calculator's ROM in the desktop app and a browser, and the picker on the phone
