---
type: iteration
title: "Iteration 78: WebMCP through tab-mcp's mirror, tools that follow the running calculators"
date: 2026-11-23
status: in-progress
tags: [iteration, web, ai]
branch: iter-78/webmcp-mirror
---

# Iteration 78: WebMCP through tab-mcp's mirror, tools that follow the running calculators

## Why

Owner (2026-11-23, passed on by the tab-mcp session): "saturnus should
start using tab-mcp-client fully as well". Today `web/ai-webmcp.js`
registers the tools with `document.modelContext` itself and the TabMcp
has `webmcp: false` (`web/ai.js`). With the client's mirror
(`webmcp: true`, client 0.22.0, vendored) saturnus takes part in the
handshake with the tab-mcp browser extension (tab-mcp DEC-207): a call
the user allowed in the extension is not asked again, and
`TabMcp.extension` says which AI apps reach the page through it. Client
0.23.0 turns the mirror on by default.

The mirror offers the browser every tool in the TabMcp's registry, the
same list the relay's AI apps get. So there is one tool list for both,
and it may change: a withdrawn or added tool reaches the browser through
the mirror and the relay's apps through `notifications/tools/list_changed`.

## Design

1. **One list, following the running calculators** (owner, 2026-11-23,
   option 1). The registry holds `list_calculators`, `lookup_command`
   and `read_guide` always, plus `FAMILY_TOOLS` of the family of every
   calculator running in the page: the one Calculator shows and the
   hub's other engines (`web/hub.js` is per document: a 48GX that Files
   works on while Calculator shows the 42S runs on and is reachable
   through `target()`; Calculator's previous model stops on a switch
   unless another view uses it). Updated when the shown model changes and when an engine
   starts or stops. A tool is withdrawn by aborting the `signal` it was
   registered with on the TabMcp and registered again when a family wants
   it back; a change touches only what differs. Same list for the relay
   and WebMCP. The desktop app (`web/ai-local.js`) keeps its own
   tools/list, unchanged.
   **Fallback** (owner): if following the engines gets complicated, all
   tools are registered once and a tool called on a calculator of
   another family fails with a clear error (as it does for relay apps
   today). See the brief's bar.
2. **The mirror replaces `web/ai-webmcp.js`.** The TabMcp exists at page
   start wherever the browser has WebMCP (`Document.prototype`'s
   `modelContext` getter, no `navigator.modelContext` fallback), with
   or without a relay; `webmcp: true`. `web/ai-webmcp.js` and its tests
   go. No in-page question for a WebMCP call (owner, 2026-11-09): there
   is no `onConfirm`, so the library runs it; a call the extension
   vouched for runs as well.
3. **Per-caller results at run time** (`client.kind === "webmcp"` in the
   `execute` context): a plain object of the structured fields, the
   display as text unless the call asked for an image (`display` /
   `format` "image" or "both": then `image` as a data: URL), and
   `list_calculators`' result carries `instructions` (the WebMCP text).
   Relay callers keep the MCP result with its image block.
4. **Descriptions say nothing about who asks** (tab-mcp's answer, 6a):
   `APPROVAL` leaves store_variable's and purge_variable's descriptions
   (the library adds "The page may ask the user to allow each call" for
   the relay); the desktop app adds its own sentence in its tools/list.
   The display argument's text is neutral about the default per caller.
   The relay's server instructions keep `APPROVAL_NOTE`; the WebMCP
   instructions in `list_calculators`' result say the app does not ask.
5. **Log and dialog**: a call with `client.kind` "webmcp" is "Your
   browser's AI", or the AI app's name when `client.via` is
   "tab-mcp-extension"; it never asks in the page. `ai.webmcp.active`
   becomes "the browser's getter is there" (the mirror is on);
   `called` comes from the `toolcall` events.
6. Dropped: the withdraw on `pagehide` and register on `pageshow` (a
   precaution, no seen case), the retry of a refused registration
   (the mirror does not retry either, tab-mcp DEC-143: a tool that comes
   back is registered anew, which tries it again), the
   `list_calculators` listing hook.

## Outcome

Design 1 built, not the fallback: following the engines took one hub
event (`engines`, to a view that asked `engines` once, when a kept
calculator starts or stops) and one listener on AI access's lobby
connection; `hub.test.mjs` gained a case, `protocol-script.test.mjs`
needed none (a lone view never asks `engines`), and the registry changes
are synchronous, so a call that runs keeps its tool (tested with the
vendored client, `web/test/ai-mirror.test.mjs`). Page load where WebMCP
is: the same scripts count, 134.5 KB of AI scripts at start instead of
144.4 KB; the first WebMCP call loads the client's 52.4 KB connection
chunk. Decision log, 2026-11-23.

Client 0.23.0 (mirror on by default) came out during the iteration: it
is vendored here and the `webmcp` option is left out.

Checked by the owner in Chrome through the tab-mcp extension (main build,
2026-11-23) on a local build: 13 tools for the 48GX, 7 for the 42S, the
list following a switch; text by default and the image as a data: URL
when asked; `list_calculators`' instructions; calls the extension
allowed ran with no question in the page; each tab its own page; WebMCP
given after load picked up (`late`). Not seen end to end: a picture as
an image block through the extension (the call arrived without
`client.via`; with tab-mcp), covered by a unit test with a simulated
announcement.

## Tasks

- [x] TabMcp at page start where WebMCP is, `webmcp: true`; relay optional; load cost measured
- [x] Registry follows the running calculators' families (or the all-tools fallback, with the reason)
- [x] WebMCP results, display default and instructions by caller kind at run time
- [x] Neutral descriptions; desktop endpoint keeps its approval sentence
- [x] Log, dialog and sidebar read the caller kind; `web/ai-webmcp.js` and its tests removed
- [x] Decision log entry; iteration 71's per-family entry and its "another tab" wording corrected; docs that name ai-webmcp.js updated
- [x] Tested in a browser with WebMCP and through the tab-mcp extension (main build)

## Out of scope

- tab-mcp's own gaps (full MCP results for calls the
  extension passes on): tab-mcp's work.
