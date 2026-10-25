---
type: iteration
title: "Iteration 59: Claude Desktop through a .mcpb"
date: 2026-10-23
status: completed
tags:
  - iteration
  - desktop
  - ai
branch: iter-59/mcpb
---

# Iteration 59: Claude Desktop through a .mcpb

Claude Desktop runs local MCP servers as programs it starts (stdio); its custom
connectors connect from Anthropic's cloud and cannot reach 127.0.0.1. So the
desktop app's server ([[iterations/iteration-58-desktop-mcp]]) gets a small
forwarder, packed as a `.mcpb` bundle that installs with a double click.
Backlog: [[backlog/mcp-ai-assistant]].

Owner's decisions (2026-10-24): one static bundle, the same for every user, with
a Node forwarder that Claude Desktop runs with its own Node. No Rust `saturnus
mcp`, no per-platform binaries, no change to the release workflows.

## Design

- The bundle `saturnus.mcpb` (a zip): `manifest.json` (MCPB manifest 0.3, server
  type `node`), `server/index.js` (the forwarder, Node built-ins only),
  `server/offline.json` (the instructions and the tool list, for the answers
  while the app is closed) and `icon.png`. Its sources are in `web/mcpb/`;
  `web/mcpb/build.mjs` builds it byte for byte the same from the same sources
  (fixed dates, fixed order), with the tool list taken from `web/ai-tools.js`.
  Built at build time, not committed: `web/site.sh` puts it at
  `app/saturnus.mcpb` for the web app (the docs page links it), the desktop app's
  `build.rs` into its embedded page; `just mcpb` builds it by hand.
- The forwarder reads JSON-RPC lines from stdin and sends each request to
  `http://127.0.0.1:4842/mcp` (the port from the manifest's `user_config`,
  passed as `SATURNUS_MCP_PORT`), with the bearer token read at run time from
  the per-user `control-token` file (`token.rs`'s path rules; `SATURNUS_TOKEN_FILE`
  overrides). The answers (JSON, or the events of an event stream) go to stdout;
  notifications get no answer (202). Nothing but MCP on stdout, diagnostics on
  stderr, the token nowhere.
- Token safety: before each request the forwarder asks the endpoint for its
  proof of the token, as `saturnus ctl` does with the control API: a token-free
  `GET /hello?nonce=N`, answered with the HMAC of N and the bound port under the
  token. A missing or wrong proof and the token is not sent. The endpoint gains
  this hello (Rust, with tests): the forwarder knocks on the port while the app
  is closed or starting, which is exactly when another local user could hold it.
- The app closed: `initialize`, `ping` and `tools/list` are answered from
  `offline.json` without starting it, so opening Claude Desktop does not open
  saturnus. A `tools/call` starts it (macOS `open -b ch.ractive.saturnus`,
  Windows the installed `saturnus.exe` or `saturnus-app.exe`, Linux
  `saturnus-app`; best effort off macOS) and waits up to 20 s for the endpoint,
  then answers "Start saturnus and turn on AI access in its settings"; a call
  the just-started app answers "not ready" (its window still loading) is asked
  again within that time. AI access off: the same answer; the forwarder never
  turns it on. No key file (AI access never on): the message, no start. Once the endpoint
  answers, the client's `initialize` is replayed to it so the calls carry its
  session and the app knows the AI app's name.
- The desktop dialog's "Connect your AI app" gets "Add to Claude Desktop": the
  app writes the bundle from its embedded page to its cache folder and opens it,
  so Claude Desktop shows its install dialog.

## Tasks

- [x] The endpoint's hello (`GET /hello?nonce=`), Rust tests, control-api-security
- [x] The forwarder: forwarding, the proof, offline answers, starting the app, the messages; Node tests against a fake endpoint
- [x] The bundle: manifest, deterministic build (`just mcpb`), built into the web app and the desktop app; a test of its contents
- [x] "Add to Claude Desktop" in the desktop dialog; the docs page links the web app's copy
- [x] kb: releasing.md (the bundle), a decision-log entry
- [x] Owner: install the `.mcpb` in Claude Desktop on macOS; with the app closed, a question that needs the calculator starts it
