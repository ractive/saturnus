---
type: iteration
title: "Iteration 74: the EquationWriter and other dead ends for the AI tools"
date: 2026-11-15
status: in-progress
tags:
  - iteration
  - web
  - ai
branch: iter-74/eqw-screen
---

# Iteration 74: the EquationWriter and other dead ends for the AI tools

After [[iterations/iteration-61-screen-state]]. The owner's report
(2026-11-15, the live app, 48GX, over WebMCP): the display showed only a
formula, top left, no cursor, no menu; `screen` was `"unknown"`, `run_rpl`
and `to_stack` refused, ENTER (by the tool and by hand) did nothing, and
`read_stack` listed 109 levels (level 1 `{ }`, the rest with no text form)
as if they were the user's stack.

## Findings (observed in saturnus on 48SX J, 48GX R, 49G 2.10)

- The screen was the EquationWriter's **scrolling mode**, opened by ▼ on an
  algebraic at the stack (48G User's Guide: "Algebraic objects ... are
  copied into the EquationWriter environment in scrolling mode"), or by
  ◀PICTURE in entry mode. ENTER does nothing there; CANCEL returns to the
  EquationWriter: entry mode (after ◀PICTURE), or selection mode (after ▼).
- 48SX and 48GX, temporary environments, newest first:
  - entry mode: the writer's own environment (`ploc`, `FontGauge`, `cx`,
    `cy`, ...) is the newest, a `SavedUI` one under it.
  - selection mode (◀ in entry mode, or CANCEL from ▼'s scrolling mode):
    one environment binding only `SavedUI` above the writer's.
  - scrolling mode: `SavedUI`, then `SAVEDDEPTH EXITFCN EnvOK` (48SX:
    `EXITFCN EnvOK`) above the writer's.
- ENTER in entry or selection mode closes the writer and puts the formula
  on the stack (opened by ▼: it replaces that level; opened from a form's
  field: the formula goes into the field). CANCEL there closes it and
  drops the formula; opened by ▼, the stack stays as it was.
- While the 48's writer is open its working objects lie on the data stack
  above the user's levels (49 with an empty formula, 52 for `1+2`, 99 for
  the report's Σ; 2 more in scrolling mode). Forms, choose and message
  boxes, PICTURE, INPUT, the interactive stack and the matrix writer put
  nothing there.
- 49G: the equation writer is an editor on the command line (`program
  input`), with no working objects on the stack; ▼ on an algebraic opens
  it too; ENTER replaces the level, CANCEL leaves it.
- `SCREENS.unknown`'s `next` says "to_stack presses it for you", but
  `to_stack` refuses on an unknown screen.

## Tasks

- [x] `uiState` names the 48SX/48GX EquationWriter: `equationWriter`
  (entry or selection mode) and `equationView` (scrolling mode).
- [x] The AI tools: `SCREENS` entries with a `next` that says how to leave
  without losing the formula and what ENTER does; `to_stack` presses
  CANCEL in `equationView` and stops at the writer with "next"; the
  `unknown` text no longer promises what `to_stack` will not do.
- [x] `read_stack` and the stack in tool results say that the stack holds
  the writer's working objects while it is open, instead of listing them.
- [x] Sweep for other screens the tools cannot name (dead ends), on all
  three ROMs. Here: the `unknown` and `input` texts say what CANCEL
  loses and that an edited object is off the stack (checked on the 48GX:
  ▼ on a real, the MatrixWriter, `0 WAIT`). Naming those screens is
  [[backlog/ai-dead-end-screens]].
- [x] Tests: `crates/saturnus-host/tests/ui_state.rs` cases, unit tests in
  `screen.rs`, `web/test/ai-tools.test.mjs`.
- [x] Docs: `web/protocol.md`, the tools' description, the saturnus skill
  if it lists screens, the wiki page hardware/ui-state.
