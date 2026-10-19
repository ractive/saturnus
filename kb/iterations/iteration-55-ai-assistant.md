---
type: iteration
title: "Iteration 55: the app's calculators for AI apps through tab-mcp"
date: 2026-10-19
status: in-progress
tags:
  - iteration
  - web
  - mcp
branch: iter-55/ai-assistant
---

# Iteration 55: the app's calculators for AI apps through tab-mcp

Owner, 2026-10-19 ([[backlog/mcp-ai-assistant]]): build tab-mcp
(`~/devel/tab-mcp`) into the web app as soon as its browser library
`@ractive-ch/tab-mcp-client` (tab-mcp iteration 2) is ready: "Connect your
AI tool", tools on the calculators, the pacing exception while connected,
the lazily loaded client in the offline cache list. The core knowledge
goes into the MCP server itself (instructions, tool descriptions,
`lookup_command`); the `saturnus` skill and plugin come later.

tab-mcp's OAuth with the pairing code (its iteration 3) is not done: an AI
app cannot get a token from the relay yet. This iteration builds against
the client's current API (`TabMcp`, `registerTool`, `connect`,
`disconnect`, `revoke`, `grants`, `pairingCode`) and leaves the seam: the
dialog shows the pairing code as soon as the relay sends one. Until the
hosted relay runs (tab-mcp iteration 6), the feature is off on the
published site; on localhost a development relay switches it on.

## Design

### Where it lives

- `web/ai.js`: the session (`AiAccess`): loads the vendored client on
  demand (dynamic `import()` of `vendor/tab-mcp-client/tab-mcp-client.min.js`),
  makes one `TabMcp` (app name "saturnus", server name `saturnus`,
  the instructions), registers the tools and keeps "Keep AI access on";
  Calculator listens to its `change` and keeps running while a session is
  connected. Made by
  `main.js` for the document, not by a view: an AI app can use the
  calculators whichever view is shown. Not in the desktop app (it gets its
  own MCP server, [[backlog/mcp-ai-assistant]]).
- `web/ai-tools.js`: the tools as plain objects over injected parts (the
  calculators, the reference), unit-tested with fake backends in Node.
- `web/ai-dialog.js`: the "Connect your AI tool" dialog in saturnus's own
  look (`components.css`): the MCP address with Copy, the state, the
  pairing code line (hidden until the relay sends one), the connected AI
  apps each with Disconnect, "Keep AI access on", Disconnect and New
  address. The client's default confirmation dialog stays (it keeps
  "Allow" off for 750 ms and shows the arguments in full; DEC-054 in
  tab-mcp), styled through its `--tab-mcp-*` properties.
- Entry points: Search's action "Connect your AI tool…", and in
  Calculator's panel a link of that name under Files and Switch
  calculator, with a status line "AI access on · 1 app" while connected.

### Relay address and CSP

- `web/ai-relay.js` `relayUrl()`: the hosted relay (`HOSTED_RELAY`, `null`
  until tab-mcp's iteration 6 runs `https://relay.ractive.ch`), or on a
  loopback page (`localhost`, `127.0.0.1`) a development relay from
  localStorage `saturnus.aiRelay` (e.g. `http://127.0.0.1:8787` for
  `bun packages/tab-mcp/src/main.ts` or `docker run ractive/tab-mcp`). No
  relay: no entry, nothing loaded.
- `web/site.htaccess` (and the meta copy `site.sh` makes):
  `connect-src 'self' wss://relay.ractive.ch`. No new script origin: the
  client is served from saturnus's own origin. A served-from-`web/`
  development page has no CSP, so a localhost relay works there.

### Vendoring the client (no bundler)

`scripts/vendor-tab-mcp-client.sh` (`just vendor-tab-mcp`) copies the
client's self-contained browser build (`dist/cdn/*.min.js`: the core and
the two chunks it imports, `@cfworker/json-schema` inlined) into
`web/vendor/tab-mcp-client/` and writes `vendor.json` (source, version,
commit or integrity, each file's SHA-256). Two sources:

- (a) **local build** (now): `--from-local ~/devel/tab-mcp`, which runs
  `bun run build` in `packages/tab-mcp-client` and records the commit.
- (b) **npm** (once published, tab-mcp iteration 5): `--from-npm
  <exact version>`: `npm pack @ractive-ch/tab-mcp-client@<version>`,
  checks the tarball's SHA-512 against `npm view … dist.integrity` and,
  when given, against `--integrity sha512-…` pinned in the justfile, then
  copies `package/cdn/*.min.js`. Written and dry-checked now; switched on
  by changing the recipe's argument.

The files are committed (CI has no tab-mcp checkout). `site.sh` and the
desktop app's `build.rs` take `vendor/tab-mcp-client/*.js` with the page
files, so the service worker precaches them (the offline list); the
desktop app embeds them unused. The chunk names carry a hash, so a new
client version is a new set of files.

### Tools (WebMCP shape, `registerTool`)

Calculators are addressed by `calculator`, an id from `list_calculators`
(the model, `48gx`); without it, the one Calculator shows. The others are
the kept calculators Files runs: reached through a hub connection of
their own (`join {who: "ai"}`, `attach` with the kept ROM's bytes, as Files
does), which never counts as hidden and lets go when the session ends.

| Tool | Models | Consequential | Does |
|---|---|---|---|
| `list_calculators` | all | no (read only) | id, model, title, ROM, shown in Calculator, family, what each family's tools are |
| `list_keys` | all | no (read only) | the key names `press_keys` takes, with their legends (shifted, alpha) |
| `read_display` | all | no (read only) | the display as a PNG image and/or text: annunciators, and on the 48SX/48GX/49G the stack and the command line as the calculator writes them; `pixels` adds the dots as `#`/`.` rows |
| `press_keys` | all | no | key names pressed one after the other through the key queue, then waits until the calculator settles and returns the display (verify there) |
| `run_rpl` | 48SX, 48GX, 49G, 28S | no | types RPL text and ENTER (the 28S through its keys, as Search does), returns the calculator's error, the stack and the display |
| `read_stack` | 48SX, 48GX, 49G | no (read only) | the stack levels as text, level 1 first |
| `list_variables` | 48SX, 48GX, 49G | no (read only) | the current directory and HOME's tree: names, types, sizes |
| `read_variable` | 48SX, 48GX, 49G | no (read only) | a variable as text that compiles back (a program, a number, …) |
| `store_variable` | 48SX, 48GX, 49G | **yes** | compiles RPL text on the calculator and stores it in a variable (creates or replaces) |
| `purge_variable` | 48SX, 48GX, 49G | **yes** (destructive) | purges a variable (a directory with all in it) |
| `run_function` | 42S, 27S, 17BII, 17B, 19BII, 38G, 39G, 40G | no | a function by its name pressed by the reference's key path, as Search does (menus left first, a pending shift cancelled; on the 38G-40G from HOME onto the edit line) |
| `lookup_command` | every model with a reference | no (read only) | a command's entry: stack effect, description, note (ROM bugs), menu, keys, examples; or a search |

Why key presses and `run_rpl` are not consequential: they are what a
person does at the keyboard while watching the calculator, and
`press_keys` can type anything `run_rpl` can; asking for each would break
the loop the tools are for (write, run, read, fix). `store_variable` and
`purge_variable` change memory without the keyboard and can replace or
delete the user's own programs: the page asks first (tab-mcp's
confirmation). Read-only tools say so (`readOnlyHint`); results that carry
calculator text are marked untrusted (`untrustedContentHint`: variable
names and programs may come from anywhere).

The server instructions carry the core knowledge: the families and their
tools, "press, then read the display to verify", RPL on the stack, the
49G's algebraic mode (flag -95), the 42S's menus and EXIT, the 38G-40G's
HOME edit line, shift and ALPHA, the 28S's number entry, the documented
ROM bugs in `lookup_command`'s notes.

### Pacing exception (owner, option 1)

While a session is connected, the calculator does not stop when the tab
is hidden: Calculator sends `visibility` hidden only when no session is
connected (`app.js` `watchAi`), and the AI app's own hub
connections never say hidden. No SharedWorker (the 0.6.0 decision
stands). When the session ends while the tab is hidden, the hidden state
is sent again.

Whether Chrome freezes a hidden tab anyway (Page Lifecycle freeze, with
the Worker and the relay's WebSocket together) is measured (below). If it
does, the dialog says to keep the tab visible (side by side) or to use the
desktop app; no workaround fights the browser.

### No new Rust APIs

Everything goes through the protocol's existing commands (`typeKeys`,
`run`, `stack`, `commandLine`, `memoryTree`, `editText`, `storeText`,
`purge`, `stats`, `skin`, `engines`, `attach`, `leave`). One hub change:
`join` takes `who: "ai"`, which hears `gone` as Files does and is not
Files for `peers`.

## Tasks

- [x] Plan (this file)
- [x] `scripts/vendor-tab-mcp-client.sh` and `just vendor-tab-mcp`: (a) local build implemented, (b) npm with integrity written
- [x] `web/vendor/tab-mcp-client/` committed with `vendor.json`; `site.sh` and `build.rs` take it; CSP `connect-src` with the relay
- [x] `web/ai-tools.js`: the tools and the server instructions
- [x] `web/ai.js`: the session, relay address, "Keep AI access on" (resume on load), calculators by id through the hub
- [x] `web/hub.js`: `who: "ai"`
- [x] `web/ai-dialog.js`, the panel's link and status line, Search's action
- [x] `web/app.js`: no `visibility` hidden while a session is connected
- [x] Unit tests: `web/test/ai-tools.test.mjs` (fake backends), `hub.test.mjs` (`ai`)
- [x] Page test `web/test/ai-page.test.mjs`: a local tab-mcp relay, the page connected by a click, the MCP SDK client calling the tools on a booted 48GX and 42S (ROM-gated; skipped without tab-mcp, Bun or Chrome)
- [x] Measure the hidden-tab freeze with a session connected (10-15 minutes hidden, periodic tool calls; normal and Energy Saver), method and result below
- [x] README (web/README.md "AI tools"), protocol.md (`who: "ai"`), storage names
- [ ] Gates, PR

## Hidden-tab measurement

Owner, 2026-10-19: does Chrome freeze a hidden saturnus tab (Page
Lifecycle freeze: the hub's Worker and the relay's WebSocket would stop
together) while an AI session is connected?

**Method** (a script, not committed; run on the owner's Mac, Chrome
155.0.8059.82, macOS on AC power, 2026-10-19): a static server for
`web/`, a local tab-mcp relay (`bun packages/tab-mcp/src/main.ts`, sqlite
store), a **headed** Chrome with a fresh profile and the DevTools port.
The page boots the 48GX, connects by a real click on "Connect your AI
tool", and logs `freeze`, `resume`, `visibilitychange`, `pagehide` and
`pageshow` with their times. A second tab is opened and brought to the
front, so the saturnus tab is hidden (`document.visibilityState`
"hidden"). The MCP SDK client then calls a tool once a minute: `run_rpl`
with `« 1 150 START NEXT » EVAL 1 +` (a few seconds of emulated
computation and a counter, so a stopped calculator or a frozen tab would
show as a timeout or a counter that does not move) for 15 minutes; the
same with Energy Saver on (profile preference
`performance_tuning.battery_saver_mode.state = 3`, "always", not checked in
the settings page); and `press_keys` (`1`, `plus`) for 10 minutes. Then the
tab is shown again and the log read.

**Result**: no freeze in any run. The only lifecycle events were
`visibilitychange` hidden and, at the end, visible; the session stayed
connected; every call answered with the counter one higher.

| Run | Calls | Answered | Time per call |
|---|---|---|---|
| `run_rpl`, 15 min | 15 | 15 | 0.1 s (1.3-11.5 s for four calls while `cargo` builds ran beside it) |
| `run_rpl`, Energy Saver, 15 min | 15 | 15 | 0.1 s (one 2.1 s) |
| `press_keys`, 10 min | 10 | 10 | 3.0 s every time |

`press_keys` takes 3 s in a hidden tab because it waits for the keys with
the page's timers (`settle`, web/keys28.js), which Chrome runs at most
once a second in a background page; `run_rpl` waits in the Worker and is
not slowed. That is the browser's normal background throttling, not a
freeze, and acceptable. So the dialog only says that the calculators keep
running in the background; no "keep the tab visible" sentence is needed
for Chrome today. Not measured: a tab hidden for hours (Chrome's
discarding under memory pressure), Safari and Firefox, a phone (where a
background tab is suspended outright, so AI access there is for a visible
tab), and Energy Saver as set in the settings page (if a later Chrome
freezes such tabs, add the sentence then).

## Waits for tab-mcp

- OAuth with the pairing code (tab-mcp iteration 3): until then no AI app
  can connect for real; the page test issues a token through the relay's
  `TokenStore` as tab-mcp's own tests do.
- The hosted relay (tab-mcp iteration 6): then `HOSTED_RELAY` is set and
  the entry appears on the published site.
- The npm package (tab-mcp iteration 5): then the vendoring recipe
  switches to `--from-npm` with a pinned version and integrity.
