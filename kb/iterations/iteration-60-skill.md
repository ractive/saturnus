---
type: iteration
title: "Iteration 60: the saturnus skill and Claude Code plugin"
date: 2026-10-23
status: completed
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

## Outcome

- The skill: `plugins/saturnus/skills/saturnus/` (`SKILL.md`, `references/rpl.md`,
  `models.md`, `workflows.md`). The plugin: `plugins/saturnus/` with
  `.claude-plugin/plugin.json`, the marketplace in `.claude-plugin/marketplace.json`
  at the repository root; the pi package is the root `package.json` naming the
  same skill folder (one copy, no sync step). Docs page `web/docs/pages/ai-apps.md`.
- The plugin's MCP entry does not need iteration 59: an `http` server at
  `http://127.0.0.1:${SATURNUS_MCP_PORT:-4842}/mcp` whose `headersHelper`
  (`scripts/token-header.sh`) reads the per-user `control-token` at each
  connection. No token in any file of the plugin. Claude Code strips variables
  named like credentials before a plugin's helper runs, so `SATURNUS_TOKEN_FILE`
  does not reach it; the default path (and `XDG_CONFIG_HOME`) does.
- Tested by doing, headlessly: the app in headless Chrome with one calculator
  booted, connected to a local tab-mcp relay, and `claude -p` (Opus, Claude Code
  2.1.308) given each task twice: with the relay address as `--mcp-config` (no
  skill), and with `--plugin-dir plugins/saturnus` (the skill, and the plugin's
  own MCP entry reaching the relay through a stand-in for the desktop endpoint
  that checks the bearer from `control-token`). All six runs gave the right
  answers. The one difference: without the skill the 17BII's first-start
  language question was answered with German, and the calculator was left in
  German; with it, English. The skill did not save turns (8/9, 12/18, 26/28
  without/with) and cost more ($0.20/0.31, 0.27/0.36, 0.43/0.57): for Opus on
  these tasks the tools' own instructions are enough. Details in the PR.

## Tasks

- [x] `SKILL.md` and its references
- [x] The Claude Code plugin and the pi package, install notes in the docs site
- [x] The three tasks with and without the skill, results in the PR
- [x] Owner: install the plugin in Claude Code and try one task
