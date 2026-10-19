---
title: "AI tools: detect an open command line on the 28S"
type: backlog
date: 2026-10-20
status: planned
priority: low
tags:
  - backlog
  - saturnus
  - mcp
---

# AI tools: detect an open command line on the 28S

From the review of [[iterations/iteration-55-ai-assistant]]: `run_rpl` refuses
to type into a command line the user has open on the 48SX, 48GX and 49G (it
reads the line through the host's typing engine), but saturnus cannot read the
28S's command line, so there the tool only warns the AI app to check the
display first. Closing the gap needs the 28S editor's memory layout found and
written to the wiki (hardware/hp28s), then the host can report "a line is
open" for the 28S too.
