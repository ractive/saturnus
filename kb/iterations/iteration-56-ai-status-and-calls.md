---
type: iteration
title: "Iteration 56: AI access status, reconnecting, and the AI's calls"
date: 2026-10-20
status: completed
tags:
  - iteration
  - web
  - mcp
branch: iter-56/ai-status-and-calls
---

# Iteration 56: AI access status, reconnecting, and the AI's calls

Owner, 2026-10-20, after the first claude.ai run against the hosted relay
<https://tab-mcp.ractive.ch>: a relay restart broke the page's connection,
and saturnus did not show it; Claude only said the calculator could not
be reached. Three changes:

1. The sidebar entry shows the connection state and offers Disconnect.
2. A broken connection is retried until it is back, not once.
3. For enthusiasts, a view of what the AI apps do: every tool call and the
   connection's events, like the packets view in Files. Hidden by default.

Needs `@ractive-ch/tab-mcp-client` 0.2.0 (tab-mcp PRs #9 and #10): the
default confirmation has "Always allow", and `TabMcp` emits `toolcall`
and `toolcallend` (id, tool, title, arguments, client; then outcome `ok`,
`error`, `declined` or `cancelled`, the result, `durationMs`).

## Design

### Sidebar entry (web/app.js `watchAi`, web/index.html)

The entry "Connect your AI app…" and the status line below it
("AI access on · 1 app · Show") become one entry that says the state:

| State | Entry | Below it |
|---|---|---|
| off (`disconnected`, no error) | Connect your AI app… | nothing |
| connecting | AI access: connecting… | |
| connected, no app | AI access on · no app yet | Show |
| connected, apps | Claude is connected (one app, its name; "2 AI apps are connected" for more), a dot in the accent colour | Calls · Disconnect |
| lost, retrying | AI access lost · reconnecting… (dot in the warning colour) | Calls · Turn off |

Clicking the entry opens the dialog as now. The sidebar's "Disconnect"
turns AI access off for this tab (`AiAccess.disconnect()`): the pairing
stays, Connect brings it back. Forgetting an app (revoking its grant)
stays in the dialog. The state line is an `aria-live="polite"` region.

### Reconnecting (web/ai.js)

On `disconnected` with reason `network`, `heartbeat` or `shutdown`,
retry with backoff 5 s, 10 s, 20 s, 40 s, then every 60 s, until
connected, until the user turns AI access off, or until a retry fails
for a reason that is not transient (refused address, `replaced`). A
retry runs whether or not "Keep AI access on" is set: the user had it on.
`AiAccess` exposes `retrying` (the next attempt's time, or null) for the
sidebar. A successful connect resets the backoff.

### The AI's calls (new web/ai-log.js, panel in web/index.html)

A drawer like Files' packets panel (`web/tx/cable-view.js`, same
markup pattern and styles where they fit): title "The AI's calls",
a list newest last, Copy (as text) and Clear, close button. Opened from
"Calls" in the sidebar status and from the command palette ("Show the
AI's calls"); closed by default; open/closed is remembered per browser.

Entries, kept in memory only (the last 200; nothing stored, empty after
reload):

- calls: time, AI app, tool title, the arguments (RPL text in
  monospace, long values folded), then the outcome on the same row once
  it ends: result text (first lines, the rest folded), "declined",
  "cancelled" or the error, and the duration; a pending call shows
  "waiting for you" while its confirmation is open, else "running";
- connection events: connected (relay host), lost (reason, in
  `disconnectText` words), "Retrying to connect… (attempt n)", "next try
  in n s", app paired, app disconnected, AI access turned off.

The log starts with AI access (it listens to `AiAccess`); it holds no
address secret, token or pairing code.

## Tasks

- [x] Vendor `@ractive-ch/tab-mcp-client` 0.2.0 (justfile version and integrity, `web/vendor/tab-mcp-client/`)
- [x] Sidebar entry with the states above, Disconnect, Calls; live region
- [x] Reconnect with backoff; `retrying` in `AiAccess`; tests in `web/test/`
- [x] `web/ai-log.js` and the drawer; palette entry; tests in `web/test/`
- [x] Copy follows `kb/docs/writing-style.md`
- [x] Screenshots: sidebar connected and lost, the drawer with calls (2 max)
- [x] `just gates`, PR, review, merge, deploy

## Notes (implementation)

- Off after an error that is not retried (another tab took over, a
  refused address) shows that error under "Connect your AI app…".
- Which failed retries end the series is read from the client's messages
  (`transientFailure` in web/ai.js): "The relay refused the connection",
  a too small message limit, `replaced` and the browser's or the user's
  refusals end it; an unreachable relay or no answer in time does not.
  A connect on page load ("Keep AI access on") that fails is not retried.
- While lost the dialog shows "… Reconnecting…", offers Turn off and
  Connect (try now), and does not connect by itself on opening.
- The drawer's copy button is "Copy log", as in the packets panel; a new
  token `--warn` colours the lost dot. `aiView` (the sidebar's states)
  lives in web/ai-log.js, so it is tested in Node without the page.
- Placement (review): on a stage 820 px or wider the drawer sits beside
  the calculator, in the stage's right part (42% of it, 360 to 640 px,
  from under the stage's tools to the bottom), and the stage keeps that
  part free so the calculator fits in the rest; narrower, and on phones,
  a sheet along the bottom.
- Screenshots (local, not committed): the sidebar connected with the
  drawer full of calls, and lost and reconnecting.
