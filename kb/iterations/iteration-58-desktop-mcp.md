---
type: iteration
title: "Iteration 58: the desktop app's calculators for AI apps, on this computer"
date: 2026-10-23
status: in-progress
tags:
  - iteration
  - desktop
  - ai
branch: iter-58/desktop-mcp
---

# Iteration 58: the desktop app's calculators for AI apps, on this computer

The desktop part of [[backlog/mcp-ai-assistant]]: the desktop app serves MCP on
127.0.0.1 while it runs, so Claude Code, Codex, VS Code and Cursor reach its
calculators without the internet and without the tab-mcp relay. Claude Desktop
follows in [[iterations/iteration-59-mcpb]], the skill in
[[iterations/iteration-60-skill]].

Owner's decisions: the 2026-10-19 list in the backlog item; 2026-10-23: sign-in by
a bearer token in the header (option b, the control API's token), not OAuth; the
one-click install links (web/shared/mcp-install.js) serve the web and the desktop.

## Design

### One set of tools: the web app's

The desktop app's window runs the web app, and `web/ai-tools.js` already holds the
tools (`list_calculators`, `read_display`, `press_keys`, `run_rpl`, the
variables, `run_function`, `lookup_command`) with the server instructions. They are
not written again in Rust. The Rust server is a thin MCP endpoint that hands each
`tools/list` and `tools/call` to the window and returns its answer:

- Rust → window: a Tauri event `mcp-call` with a request id and the JSON-RPC
  params; window → Rust: a command `mcp_reply(id, result)`. A call the window does
  not answer within the tool's time limit gets a JSON-RPC error; a closed or
  reloading window answers "the app is not ready".
- In the window a small module (`web/ai-local.js`) runs the same `aiTools(deps)`
  that `web/ai.js` gives tab-mcp, with the same deps.
- What tab-mcp's client does around a call: the confirmation dialog, "Always
  allow", the `toolcall`/`toolcallend` events that feed "The AI's calls"
  (web/ai-log.js). These live inside `@ractive-ch/tab-mcp-client` and are not
  exported. First choice: ask the tab-mcp session for an exported local entry
  (run a registered tool through the client's confirm and events without a
  relay). Fallback: saturnus does the three itself in `ai-local.js` (same look, the
  dialog from `shared/`). Decide when the iteration starts; say which in the PR.

### The server

- MCP Streamable HTTP, the stateless revision (2026-07-28), and the legacy
  `initialize` handshake for clients that still start with it (tab-mcp's relay
  answers both: `~/devel/tab-mcp/packages/tab-mcp/src`, read only).
- The control API's rules ([[docs/control-api-security]]) unchanged: 127.0.0.1
  only, `Host` and `Origin` checks, no CORS, no `OPTIONS`, body caps, the
  constant-time bearer token, the token never printed or logged. Its code
  (`crates/saturnus-cli/src/control/`: `http.rs`, `token.rs`) moves to a crate both
  the CLI and the desktop app use (`saturnus-drive` or a new small one), with its
  tests.
- The token: the same per-user file as the control API (`control-token`), so
  [[iterations/iteration-59-mcpb]]'s forwarder finds it.
- Port 4842 (4840 control API, 4841 serial, 4848-4852 satx, 4860-4889 bridges); a
  busy port is reported naming its user, as the control API does, and the setting
  offers another port.
- Off by default. A setting "Let AI apps on this computer use the calculators";
  turning it off stops the server at once.
- While a call runs the calculator keeps running when the window is hidden (as
  the web app's pacing exception during an AI session).

### The dialog

The web app's "Connect your AI app" dialog with the local address
(`http://127.0.0.1:4842/mcp`), the state (off, on, an app connected), the
one-click links and commands with the `Authorization` header, and the setting. No
pairing code. The token is in the commands and links: the AI app keeps it in its
config file (accepted, option b).

## Tasks

- [x] Move the control API's HTTP and token code to a shared crate, tests kept
- [x] The MCP endpoint in the desktop app: stateless and legacy handshake, the rules above, Rust tests for each rule and both handshakes
- [x] The bridge: `mcp-call` event, `mcp_reply` command, time limit, window not ready
- [x] `web/ai-local.js`: the tools from `aiTools`, confirmation, Always allow, the AI's calls (tab-mcp's local entry or saturnus's own)
- [x] The setting, the port, the dialog for the desktop with the install links
- [x] kb: [[docs/control-api-security]] extended to the MCP endpoint, a decision-log entry
- [ ] Owner: Claude Code and Cursor against the desktop app on macOS: list the calculators, run RPL on the 48GX, a confirmation, Always allow, the AI's calls, the setting off ends it
