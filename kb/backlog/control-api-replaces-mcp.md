---
title: "Control API over HTTP replaces saturnus-mcp (relayed, unconfirmed)"
type: backlog
date: 2026-10-05
status: deferred
tags:
  - backlog
  - saturnus
priority: high
---

# Control API over HTTP replaces saturnus-mcp (relayed, unconfirmed)

**Status of this note:** relayed on 2026-10-05 by the hptx session as an
owner decision recorded in hptx's decision log. It has not been confirmed
by the owner in the saturnus session, and nothing here is started. The
same session's crates.io request of the same day was withdrawn hours
later, so this stays a proposal until the owner confirms it here. Until
then the decision log's entries (saturnus-mcp, its `hptx-core` pin, the
pin freeze) stand as written.

## What was relayed

1. MCP is dropped on both sides: no hptx MCP server, and `saturnus-mcp` is
   retired; its `hptx-core` pin goes with it, after which nothing in
   saturnus depends on hptx.
2. saturnus becomes "an emulator that serves a serial port and a control
   API": `saturnus run` in the foreground (no daemon, no instance files)
   serves the serial port on TCP as today plus an HTTP/1.1 + JSON control
   API bound to 127.0.0.1. Guards: a per-user token file (mode 0600,
   created on first run) and Host/Origin checks, because loopback alone is
   no boundary (browser CSRF, DNS rebinding). Endpoints: screen (JSON rows
   or `image/png`), keys, type text, memory read/write, cycles, info,
   snapshots returned as bytes (never server-side paths), model. Bodies
   reuse the command/event shapes of `web/protocol.md`, so the API is a
   third adapter of one protocol. `run` prints its endpoints, refuses a
   busy port naming the listener, ends on Ctrl-C; fixed default ports,
   `--control` and `SATURNUS_CONTROL` for a second instance.
3. `saturnus ctl <screen|keys|type|mem|snapshot|info|...>` is the CLI
   client of that API; agents may call the API directly. Calculator
   operations (ls/get/put/run) against a running saturnus go through the
   hptx CLI over the serial port, as against hardware.
4. Small dependencies (a blocking tiny_http-class server, no tokio).
5. Open on the hptx side: whether `hptx-core` adopts `saturnus-objects`
   (body decoder only). hptx's in-process transport will use
   `saturnus-drive`'s autostart; the `hptx-saturnus` adapter crate and the
   pin freeze recorded in the decision log (iteration 16) are superseded
   if this is confirmed.

## What it would cost in saturnus

- `saturnus-mcp` holds the semantic tools from iteration 9 (`eval`, typed
  stack, variables) and the ROM-gated tests that carry most of the
  end-to-end coverage of the Kermit path, the summation benchmark and
  `ram_reads_match_kermit` (iteration 12a). Retiring the crate needs a new
  home for those tests (an hptx dev-dependency would bring the pin back;
  the alternative is driving the hptx CLI as a black box, as the Docker
  differential script does).
- `eval` and the typed stack have no replacement in the relayed design
  other than "use the hptx CLI over the serial port". The RAM reads of
  `saturnus-objects` cover tree, stack and flags without Kermit; execution
  does not have a Kermit-free path.
- Iterations 12 to 14 (explorer, command reference, object editor) were
  planned on "reads from RAM, writes via hidden Kermit" inside the page;
  that Kermit code is hptx-core compiled into saturnus hosts. If saturnus
  must not depend on hptx at all, those plans need a different write path.

## Questions for the owner

- Is the retirement of `saturnus-mcp` confirmed for saturnus?
- Where do writes and execution for the web explorer and editor come from
  if saturnus drops the hptx dependency?
- Does the control API come before or after the UI iterations 12 to 14?
