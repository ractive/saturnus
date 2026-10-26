---
title: "AI assistant: saturnus over MCP, with a saturnus skill and plugin"
type: backlog
date: 2026-10-19
status: completed
priority: medium
tags:
  - backlog
  - saturnus
  - mcp
---

# AI assistant: saturnus over MCP

Owner's decisions, 2026-10-19 (discussion in [[research/ai-assistant-options]]):

- **Desktop app**: serves MCP over Streamable HTTP on 127.0.0.1 while it
  runs, behind a setting ("Allow AI assistants to control this calculator"),
  with the control API's token and Host/Origin rules. Claude Code, Codex,
  VS Code and Cursor connect to it by URL.
- **Claude Desktop**: a `.mcpb` bundle with a small `saturnus mcp` stdio
  forwarder to the running desktop app (Claude Desktop's custom connectors
  connect from Anthropic's cloud and cannot reach localhost). If the app is
  not running, the forwarder starts it. No headless calculator: an agent
  only drives a calculator the user sees.
- **Web app**: through tab-mcp (`~/devel/tab-mcp`, its own project): the tab
  connects to a relay and gets an address for any AI app. While an agent is
  connected, the calculator keeps running in a hidden tab (pacing option 1:
  no `visibility` pause during a session).
- **Only the stateless MCP revision (2026-07-28)**; OAuth with a pairing code
  confirmed in the app.
- **The saturnus skill and plugin** (owner, 2026-10-19): the core knowledge
  goes into the MCP server itself (server instructions, tool descriptions, a
  `lookup_command` tool on the existing reference), so it needs no install. On
  top, an optional `saturnus` skill in the open Agent Skills format
  (`SKILL.md`, for Claude Code, claude.ai, Codex and pi): RPL programming,
  the stack, the models' differences (42S menus, the 38G's HOME line, shift
  and ALPHA), known traps (NULL on long presses, the 28S's number entry, the
  documented ROM bugs), "press, then read the display to verify", and
  workflows (write, store and test a program). Packaged like hyalo's
  (`plugins/hyalo`, `pi-package`): a Claude Code plugin with the skill, the
  desktop app's MCP address and the `.mcpb`.

Open for the plan: which calculators the tools address (the one shown in
Calculator by default, others by id), and the tool set per model family.

**Owner, 2026-10-19:** build tab-mcp into saturnus's web app as soon as its
browser library (`@ractive-ch/tab-mcp-client`, tab-mcp iteration 2) is ready:
the "Connect your AI tool" dialog (address plus pairing code), the tools on
the calculators, the pacing exception while connected, the lazily loaded
client in the offline cache list. During development against a local relay
(`docker run ractive/tab-mcp`); for visitors once the hosted relay on
ractive.ch is up (tab-mcp iteration 6).

**Iteration 55** ([[iterations/iteration-55-ai-assistant]]) builds the web
app's part against tab-mcp's client as it is (no pairing code yet); still
open here: the desktop app's MCP server, the `.mcpb`, the skill and the plugin.

**Owner, 2026-10-23:** the desktop app's local server signs in with a bearer
token in the header (the control API's token), not OAuth. Planned as
[[iterations/iteration-58-desktop-mcp]] (the server, reusing the web app's tools
through the window), [[iterations/iteration-59-mcpb]] (Claude Desktop) and
[[iterations/iteration-60-skill]] (skill and plugin). The one-click install links
(Cursor, VS Code, Claude Code, Codex) serve the web and the desktop dialog.
