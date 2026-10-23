---
type: iteration
title: "Iteration 60: the saturnus skill and Claude Code plugin"
date: 2026-10-23
status: planned
tags:
  - iteration
  - ai
branch: iter-60/skill
---

# Iteration 60: the saturnus skill and Claude Code plugin

The optional layer on top of the MCP tools ([[backlog/mcp-ai-assistant]],
owner 2026-10-19): what an AI app needs to use the calculators well, beyond the
server instructions and tool descriptions (`web/ai-tools.js` `INSTRUCTIONS`), which
stay the core and need no install.

## Design

- A skill in the open Agent Skills format (`SKILL.md` plus references), for Claude
  Code, claude.ai, Codex and pi: RPL programming and the stack; the models'
  differences (the 42S menus, the 38G's HOME line, shift and ALPHA); known traps
  (NULL on long presses, the 28S's number entry, the ROM bugs noted in the
  reference); "press, then read the display"; workflows (write, store and test a
  program; move a program between models). Facts from the manuals and the docs
  site (`/saturnus/docs/`), rephrased; links to the docs pages rather than copies.
- Packaged as hyalo's is (`plugins/hyalo`, `pi-package` in ~/devel/hyalo): a Claude
  Code plugin with the skill and the desktop app's MCP address (with the token
  from the per-user file, as [[iterations/iteration-59-mcpb]]'s forwarder), and a
  pi package.
- Tested by doing: three tasks with and without the skill (write and run a program
  on the 48GX, a TVM problem on the 17BII, a statistics fit on the 38G), compared.

## Tasks

- [ ] `SKILL.md` and its references
- [ ] The Claude Code plugin and the pi package, install notes in the docs site
- [ ] The three tasks with and without the skill, results in the PR
- [ ] Owner: install the plugin in Claude Code and try one task
