---
type: iteration
title: "Iteration 59: Claude Desktop through a .mcpb and `saturnus mcp`"
date: 2026-10-23
status: planned
tags:
  - iteration
  - desktop
  - ai
branch: iter-59/mcpb
---

# Iteration 59: Claude Desktop through a .mcpb and `saturnus mcp`

Claude Desktop runs local MCP servers as programs it starts (stdio); its custom
connectors connect from Anthropic's cloud and cannot reach 127.0.0.1. So the
desktop app's server ([[iterations/iteration-58-desktop-mcp]]) gets a small
forwarder, packed as a `.mcpb` bundle that installs with a double click.
Backlog: [[backlog/mcp-ai-assistant]].

## Design

- `saturnus mcp` (saturnus-cli): reads JSON-RPC from stdin, sends each message to
  the desktop app's endpoint with the token from the per-user `control-token`
  file, writes the answers to stdout. Nothing on stdout but MCP; messages to stderr.
- The app not running: start it (macOS `open -b <bundle id>`, Windows and Linux
  the installed path) and wait for its server, up to about 20 s, then a clear
  error ("Start saturnus and turn on AI access in its settings"). AI access off:
  the same message, never turned on by the forwarder. No headless calculator.
- The token travels only to 127.0.0.1 at the expected port after the control
  API's hello proof (the server shows it holds the token), as `saturnus ctl` does.
- The `.mcpb`: the MCPB manifest (check the current spec), `saturnus mcp` as a
  binary server for macOS (universal), Windows and Linux. Built by the release
  workflow and attached to the GitHub release; the cask and installers unchanged.
- The desktop dialog's "Add to Claude Desktop" opens the release's `.mcpb` (or
  saves the one shipped with the app, if the app can carry it cheaply: decide in
  the iteration).

## Tasks

- [ ] `saturnus mcp`: forwarding, hello proof, start the app, the messages; tests against a fake server
- [ ] The `.mcpb` manifest and its build in the release workflow, per platform
- [ ] "Add to Claude Desktop" in the desktop dialog
- [ ] kb: releasing.md (the bundle), a decision-log entry
- [ ] Owner: install the `.mcpb` in Claude Desktop on macOS; with the app closed, a question that needs the calculator starts it
