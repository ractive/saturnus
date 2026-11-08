---
type: iteration
title: "Iteration 71: the web app's tools through WebMCP"
date: 2026-11-08
status: planned
tags:
  - iteration
  - web
  - ai
branch: iter-71/webmcp
---

# Iteration 71: the web app's tools through WebMCP

The web app offers its calculator tools to AI apps through a tab-mcp relay
(iteration 55; `web/ai.js`, `web/ai-tools.js`). WebMCP lets a page offer
the same tools to agents in the browser itself, with no relay. Owner
2026-11-08: "let's plan to support WebMCP for the web based saturnus app.
We already have the tools - let's make them available for WebMCP as
well."

## WebMCP as specified (W3C Web Machine Learning CG draft, `index.bs` of 2026-11-02)

- `document.modelContext` (secure contexts only; the spec no longer has
  `navigator.modelContext`, which early Chrome builds used).
- `registerTool(tool, {exposedTo, signal})` returns a promise; a tool is
  withdrawn by aborting its `signal` (there is no `unregisterTool`).
- A tool: `name`, `title`, `description`, `inputSchema`, `outputSchema`
  (added 2026-11-02), `execute(input, {signal})` returning a promise, and
  `annotations` `{readOnlyHint, untrustedContentHint, consequentialHint,
  debugging}`.
- `getTools`, `executeTool` for agents; events `toolchange`,
  `toolactivated`, `toolcancel`. `exposedTo` lists other origins that may
  see a tool (two-sided opt-in).
- Status (spec repo `implementation-status.md`): Chrome 149 origin trial
  (token per origin; `about:flags#enable-webmcp-testing` for local work),
  Edge 150 origin trial (its own token), ChatGPT Desktop supports it,
  Brave's Leo experimental, Firefox and Safari: standards positions open.

`web/ai-tools.js` already builds tools in this shape (tab-mcp's
`registerTool` follows the WebMCP draft, tab-mcp DEC-010 and DEC-051),
with `consequentialHint` on the tools that change the calculator.

## Tasks

- [ ] **Register through WebMCP where the browser has it:** feature-detect
  `document.modelContext` (and, while Chrome builds still carry it, the
  older `navigator.modelContext`), register every tool of `aiTools(...)`
  with an `AbortSignal` per registration, re-register when the set changes
  (a calculator's model decides the tool family), withdraw on page hide or
  when the user turns it off. Same tools, same `execute` as tab-mcp.
- [ ] **Shape:** pass `execute(input, {signal})` through and honour the
  signal where a tool waits (keys, a running command); add `outputSchema`
  where a tool returns structured data; check annotations map
  (`readOnlyHint` for read tools, `consequentialHint` as today,
  `untrustedContentHint` for results that carry user content).
- [ ] **Always registered; the page confirms changes:** only the browser
  agent (built into the browser, or hosted by it through an extension or
  plug-in) can call a page's tools, and the permissions-policy feature
  `tools` (default allowlist `'self'`) keeps cross-origin frames out
  (spec, "Permissions policy integration"). Owner 2026-11-08: the tools are
  registered whenever the browser has WebMCP, no switch. A consequential
  call is confirmed in the page as for tab-mcp
  (`web/shared/components/confirm.js`), and the AI log records WebMCP calls
  too. tab-mcp and WebMCP run side by side.
- [ ] **Origin trial:** the owner registers `https://ractive.ch` (a token
  is bound to the origin, not a path) for the Chrome 149
  and Edge 150 trials; the tokens go into the page's `<meta
  http-equiv="origin-trial">` (one per browser), with a note of their
  expiry in kb. The Chrome token for `https://ractive.ch` is in `web/index.html`
  since 2026-11-08 and expires on 2027-03-30; Edge's is still to come. Not in the desktop app (its webview has no WebMCP).
- [ ] **Tests:** unit tests with a fake `document.modelContext`
  (registration, re-registration, abort, consent off); a page test in
  Chrome with WebMCP testing enabled if the flag works headless (find the
  switch), calling a read tool and a consequential tool through
  `executeTool`.
- [ ] **Docs:** the AI access help and `web/docs` page, `kb/docs`, the
  CHANGELOG; facts about WebMCP behaviour in browsers go to `kb/research`.

## Acceptance

- In Chrome with WebMCP enabled, an agent sees saturnus's tools, reads the
  display and runs RPL on the 48GX; a consequential tool asks first.
- With WebMCP absent, nothing changes; tab-mcp works as before.
- `just gates`, `hyalo lint` clean.

## Sources

- WebMCP spec repository: <https://github.com/webmachinelearning/webmcp>
  (`index.bs`, `implementation-status.md`)
- Chrome origin trial: <https://developer.chrome.com/blog/ai-webmcp-origin-trial>,
  local testing: <https://developer.chrome.com/docs/ai/webmcp>
- Chrome Status: <https://chromestatus.com/feature/5117755740913664>
