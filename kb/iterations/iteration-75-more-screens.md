---
type: iteration
title: "Iteration 75: the interactive stack, object edits and the MatrixWriter for the AI tools"
date: 2026-11-15
status: completed
tags:
  - iteration
  - web
  - ai
branch: iter-75/more-screens
---

# Iteration 75: the interactive stack, object edits and the MatrixWriter for the AI tools

After [[iterations/iteration-74-eqw-screens]]: the top three of
[[backlog/ai-dead-end-screens]] (owner, 2026-11-15: "go for it"). Each
is a screen an AI lands in often, which the tools read as `unknown` or as
`program input` today.

## Context from iteration 74

- The sweep's rules (in the backlog item) come from 48SX J, 48GX R and
  49G 2.10, one revision each, and most rest on the types of unnamed
  temporaries. Check each rule on the 49G 1.19-6 ROM (`rom.49g`) too;
  drop a rule that does not hold, rather than widening it until it
  matches everything.
- `screen.rs` walks the temporary environments as `Environment { names,
  unnamed }`; the new rules need the types of the bound objects as well.
- The tools keep a `WORKING_STACK` table for screens whose stack is not
  the user's, and `closable` for the screens to_stack may CANCEL.

## Tasks

- [x] **Interactive stack** (▲ at the stack; the 49G's HIST): its own
  kind, closable (one CANCEL; changes already made stay), `next` from the
  manuals (▲▼ move the pointer, the menu keys act on the pointed level at
  once, ENTER or CANCEL leaves).
- [x] **An object edited on the command line inside a loop** (▼ on a
  level that is neither an algebraic nor a matrix, EDIT, VISIT, the
  interactive stack's EDIT, the 49G's TOOL EDIT): its own kind, told
  apart from INPUT; not closable; `next`: ENTER puts the object back,
  CANCEL restores the original; read_stack says the object is off the
  stack (VISIT: a variable, the stack untouched).
- [x] **MatrixWriter** (◀MATRIX, ▼ on a matrix, the 48's interactive stack
  VIEW on a matrix, STAT EDIT), also with a cell being typed: its own
  kind; not closable; `next`: type a number and ENTER fills a cell, ENTER
  with nothing typed puts the matrix on the stack, CANCEL drops a new
  matrix and restores an edited one.
- [x] ROM tests in `crates/saturnus-host/tests/ui_state.rs` on all three
  ROMs and, for the rules, on the 49G 1.19-6 ROM; unit tests in
  `screen.rs`; `web/test/ai-tools.test.mjs`.
- [x] Docs: `web/protocol.md`, the tools' texts, the saturnus skill,
  CHANGELOG, the wiki page hardware/ui-state, the backlog item.
