---
type: iteration
title: "Iteration 61: the AI tools say what is on the screen"
date: 2026-10-24
status: in-progress
tags:
  - iteration
  - ai
branch: iter-61/screen-state
---

# Iteration 61: the AI tools say what is on the screen

An AI app on the 48GX was caught in the PLOT input form: run_rpl saw no open
command line, typed UPDIR and ENTER into the form's field, got "Invalid Syntax",
and kept trying commands, because nothing told it a form was open. Owner
2026-10-24: read the state from memory rather than guess it from the display.
Backlog: [[backlog/mcp-ai-assistant]].

## Design

### Research first (black box, as [[docs/clean-room-rule]] allows)

The way `wiki: hardware/command-line` found the editor: on 48SX J, 48GX R and 49G
2.10, diff system RAM between the bare stack and each of: an input form (PLOT,
SOLVE), a choose box, a message box (also over a form), the plot/graphics screen,
the command line, a halted program (HALT), the interactive stack, the
matrix writer or equation writer where present. Look for a signal that says
"the stack's top-level loop is in control" (for example the return stack's
depth against the bare stack, or a pointer the ROM sets on entering a
key-reading loop), and for what names the open screen (a form's title string, a
message's text). Findings, with how each was observed, go into the wiki
(calculator-knowledgebase, a new `hardware/ui-state` page or
`hardware/hp48-system-ram`), cited in code as `wiki: hardware/...`.

### The tools

- `press_keys`, `run_rpl`, `read_display` (and run_function where it applies)
  return `screen`: `stack`, `command line`, `input form` (with its title),
  `choose box`, `message` (with its text), `graphics`, `program halted`,
  `unknown`. From memory where the research gives a reliable signal; the name of
  what is open from memory if found, otherwise from the display (the checkered
  title bar, the message frame), our own observation.
- Every screen but the stack comes with `next`: what to press to leave it or to
  work in it (CANCEL leaves a form, OK confirms it, any key closes a message,
  CANCEL returns from the plot screen, CONT or KILL for a halted program).
- `run_rpl` refuses unless the stack is in control, as it refuses an open
  command line, with `screen` and `next`.
- A way back: a `to_stack` tool (or `run_rpl` `leave: true`) presses CANCEL until
  the stack is in control, at most about five times, and says what it closed. It
  never cancels a command line the user typed without saying so (it refuses, as
  run_rpl does).
- `INSTRUCTIONS` gets a short paragraph on forms, choose boxes, messages, the plot
  screen. The web and desktop paths keep the same tools (web/test/ai-local.test.mjs).
- Scope: 48SX, 48GX, 49G. The key-driven models and the 28S unchanged.

If the research finds no reliable signal, fall back to the display alone for
`screen`, and say so in the PR.

## Tasks

- [x] Research: RAM diffs per screen on the three ROMs, findings in the wiki (commit or PR to calculator-knowledgebase)
- [x] Reading the state (Rust, saturnus-objects or saturnus-host, wasm-safe), tests per screen with ROMs (gated by SATURNUS_ROM_DIR)
- [x] `screen` and `next` in the tools' results, run_rpl's refusal, `to_stack`
- [x] INSTRUCTIONS and tool descriptions; web tests
- [x] kb: decision-log entry
- [ ] Owner: an AI app on the 48GX opens PLOT, gets out with the hint, runs RPL again
