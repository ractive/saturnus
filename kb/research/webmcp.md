---
title: "WebMCP in browsers: what Chrome does with a page's tools"
type: research
status: completed
date: 2026-11-08
tags:
  - web
  - ai
  - mcp
  - webmcp
---

# WebMCP in browsers: what Chrome does with a page's tools

Facts behind iteration 71 ([[iterations/iteration-71-webmcp]]): the web
app registers its calculator tools (`web/ai-tools.js`) with the browser
through WebMCP (`web/ai-webmcp.js`). Measured on 2026-11-08 in Google
Chrome 156.0.8078.80 (macOS, `--headless=new`) with the probe pages in
the iteration's scratchpad and `web/test/ai-webmcp-page.test.mjs`;
the rest from the sources listed at the end.

## Switching it on

- Chrome 156 without a switch has no `document.modelContext` (an
  `http://127.0.0.1` page, no origin trial token for that origin).
- `--enable-features=WebMCPTesting` turns it on, headless too: the
  feature behind `chrome://flags/#enable-webmcp-testing`, whose
  description in the binary is "WebMCP for testing" / "Enables the
  WebMCP API". `--enable-features=WebMCP` and
  `--enable-blink-features=WebMCP` did the same. Chrome's docs name only
  the flag page, no switch ([Chrome: WebMCP](https://developer.chrome.com/docs/ai/webmcp)).
  The page tests use `--enable-features=WebMCPTesting`
  (`web/test/chrome.mjs` `session(t, {args})`).
- On the published site the origin trial does it: Chrome 149 on, Edge
  150 on, a token per origin (`https://ractive.ch:443`, feature
  `WebMCP`) in a `<meta http-equiv="origin-trial">` of `web/index.html`.
  The Chrome token expires on 2027-03-30, Edge's on 2026-12-23 (the
  tokens' own `expiry`; renewal: [[backlog/webmcp-origin-trial-tokens]]).

## The API Chrome 156 has

- `document.modelContext` with `registerTool`, `getTools`,
  `executeTool` and the handlers `ontoolchange`, `ontoolactivated`,
  `ontoolcancel`; no `navigator.modelContext`, no `unregisterTool`, no
  `provideContext`, no `navigator.modelContextTesting`. The spec draft
  has no `navigator.modelContext` either; Chrome's early preview builds
  had it there, so `web/ai-webmcp.js` falls back to it
  ([spec `index.bs`](https://github.com/webmachinelearning/webmcp/blob/main/index.bs)).
- `registerTool(tool, {signal})` resolves; aborting the signal
  withdraws the tool (gone from `getTools` at once, so the same name can
  be registered again right away). A second tool of the same name, or a
  name with other characters than `A-Z a-z 0-9 _ - .`, is refused with
  `InvalidStateError`, as the draft says.
- `getTools()` gives `name`, `title`, `description`, `inputSchema`,
  `annotations` (all four hints, defaults false) and `origin`. It did
  not give `outputSchema` back, though `registerTool` took one (the draft
  added it on 2026-11-02).
- Chrome does not check the input against `inputSchema`: a string for an
  integer and an extra property reached `execute` as sent (the draft
  leaves it to the tool, issue 92). saturnus checks the arguments itself
  (`web/ai-local.js` `invalidArguments`).
- `executeTool` resolves to the result as a string: an object as its
  JSON, a string as it is (not JSON-quoted, unlike the draft's "serialize
  to a JSON string"), `undefined` as `"undefined"`.
- A rejected `execute` rejects `executeTool` with `UnknownError`, "Tool
  was executed but the invocation failed", without the tool's message
  (the draft: `OperationError`, and "tool execution failures defined by
  tool authors can also be reported in the response JSON string").
  saturnus resolves with `{error: message}` instead.
- Aborting `executeTool`'s signal rejects it with `AbortError`, aborts
  the signal `execute` got, and fires `toolcancel` with the tool's name;
  `toolactivated` fired when the call began.

## Who calls the tools

- The browser's agent (built in, or an extension through content
  scripts), and same-origin script through `getTools`/`executeTool`.
  Chrome's docs name two test agents, the Model Context Tool Inspector
  extension and an example agentic extension, "separate from Gemini in
  Chrome"; they do not say Gemini in Chrome calls WebMCP tools
  ([Chrome: WebMCP](https://developer.chrome.com/docs/ai/webmcp),
  [Secure tools](https://developer.chrome.com/docs/ai/webmcp/secure-tools)).
  ChatGPT Desktop supports WebMCP, Brave's Leo has it experimentally;
  Firefox and Safari have open standards positions
  ([implementation status](https://github.com/webmachinelearning/webmcp/blob/main/implementation-status.md)).
- The permissions policy feature `tools` (default allowlist `'self'`)
  gates every method; a cross-origin frame needs `allow="tools"`, and a
  tool reaches another origin only if its `exposedTo` names it and that
  origin asks with `fromOrigins` (spec, "Permissions policy integration"
  and "Enforcing Same-Origin Isolation via Two-Sided Opt-In").
- Chrome documents no confirmation of its own for `consequentialHint`:
  the hint "allows agents and browsers to enforce mandatory user
  confirmation prompts", and the example extension "always trusts
  WebMCP annotations" ([Imperative API](https://developer.chrome.com/docs/ai/webmcp/imperative-api)).
  So the page confirms consequential calls itself.
- Chrome's docs show tools returning strings; the agent gets no image
  type. saturnus returns its JSON fields and a display image as a
  `data:image/png;base64,…` string in `image`.

## Not verified

- A real agent (Gemini in Chrome, an extension, Edge's Copilot)
  calling the tools: no agent was available headless. The page tests
  call through `executeTool`, the in-page path the draft gives.
- Edge 150's behaviour: not run here.
- The origin trial on ractive.ch: needs the deployed page in a Chrome
  149+ without the flag.

## Sources

- WebMCP draft, W3C Web Machine Learning CG: <https://github.com/webmachinelearning/webmcp> (`index.bs`, `implementation-status.md`), read 2026-11-08
- Chrome, WebMCP: <https://developer.chrome.com/docs/ai/webmcp>
- Chrome, the imperative API: <https://developer.chrome.com/docs/ai/webmcp/imperative-api>
- Chrome, secure tools: <https://developer.chrome.com/docs/ai/webmcp/secure-tools>
- Chrome, the origin trial: <https://developer.chrome.com/blog/ai-webmcp-origin-trial>
- Chrome Status: <https://chromestatus.com/feature/5117755740913664>
