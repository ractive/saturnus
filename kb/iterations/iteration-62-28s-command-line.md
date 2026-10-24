---
type: iteration
title: "Iteration 62: the AI tools see the 28S's command line"
date: 2026-10-24
status: in-progress
tags:
  - iteration
  - ai
branch: iter-62/28s-command-line
---

# Iteration 62: the AI tools see the 28S's command line

From [[backlog/ai-28s-command-line]]. Owner 2026-10-24: "Try it". On the 48SX,
48GX and 49G `run_rpl` refuses to type into a command line the user has open
(`saturnus_objects::cmdline`); on the 28S nothing reads its memory, so the tool
only tells the AI app to check the display first.

## Design

- Research, black box as [[docs/clean-room-rule]] allows, the way `wiki:
  hardware/command-line` found the 48/49 editor: on the 28S ROM, scan RAM for
  typed text, diff system RAM between the bare stack and an open command line
  (empty, with text, cursor moved, insert/replace, algebraic and program entry,
  an error shown after ENTER). Findings with how each was observed go into the
  wiki (`hardware/hp28s` or `hardware/command-line`), cited in code as
  `wiki: hardware/...`.
- If a reliable signal exists: an `EditorLayout` (or the 28S's own reader) for
  the 28S, the `commandLine` answer for it, `run_rpl` refuses on the 28S as on
  the 48 (the web and desktop tools stay identical), INSTRUCTIONS and the tool
  descriptions lose the "check the display first" caveat for the 28S.
- If none is found: say so in the wiki's "Not found" and in the PR; the code
  stays as it is.

## Tasks

- [x] Research on the 28S ROM; findings in the wiki (commit to calculator-knowledgebase)
- [x] Reader and tests with the ROM (gated by SATURNUS_ROM_DIR)
- [x] run_rpl's refusal on the 28S, INSTRUCTIONS and descriptions, web tests
- [x] kb: decision-log entry; backlog item completed
- [ ] Owner: on the 28S with a line open, an AI app's run_rpl is refused and says so
