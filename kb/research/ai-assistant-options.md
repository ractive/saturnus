---
title: "AI assistant for the calculator: how users bring their own AI"
type: research
date: 2026-10-19
status: active
tags:
  - research
  - saturnus
  - ai
  - mcp
  - security
---

# AI assistant for the calculator: how users bring their own AI

Owner's question: an assistant that helps write RPL programs, does
calculations, explains commands and drives the calculator. The owner pays
for no tokens; users bring their own AI. He fears nobody pastes an API key
into a web app, and asks whether the assistant could be desktop-only and
how users could reuse what they already pay for (Claude, ChatGPT or Codex,
OpenRouter, opencode, GitHub Copilot, local models).

Research date: 19 October 2026. Every external claim carries its URL.
Points the sources contradict or did not settle are marked "unverified".

## Short answer

1. **Bring saturnus to the agent, not the agent to saturnus.** Ship a
   local MCP server (`saturnus mcp`, stdio) that drives a calculator and
   looks up the command reference. Claude Code, Claude Desktop, Codex,
   VS Code with Copilot, Cursor, opencode, Gemini CLI (paid users) and
   LM Studio all take a local stdio server today. The user's existing
   subscription pays, no key passes through saturnus, and nothing breaks
   any provider's terms. This is desktop-only by nature, and that is fine.
2. **In the desktop app, an optional built-in chat with "Sign in with
   ChatGPT", OpenRouter sign-in and a local model (Ollama, LM Studio).**
   ChatGPT plan usage is now open to open-source apps that run locally
   (saturnus is MIT), OpenRouter has a PKCE flow, and local servers are a
   loopback call away. No key pasting.
3. **In the web app, at most OpenRouter sign-in, later.** Worth it only
   after 1 and 2, and only for "explain" and "write a program" without
   key presses.

Not worth doing: Claude subscription login inside saturnus (forbidden),
spawning the user's `claude` CLI as saturnus's engine (against the spirit
of the same rule), Chrome's built-in model, WebLLM, a hosted remote MCP
server, and WebMCP until it leaves origin trial.

## What saturnus has today

- **Control API** on 127.0.0.1:4840 (`saturnus run --control`,
  `saturnus ctl`): keys, typing, screen as text or PNG, stack, command
  line, HOME's variables, a variable as RPL text and `--set` (compiled by
  the calculator), store and fetch through the hidden Kermit server,
  snapshots. Guarded by a per-user token file and Host and Origin checks
  (`README.md`, `kb/docs/control-api-security.md`). This is everything an
  agent needs.
- **Command reference**: `web/commands.json`, 856 commands with per-model
  examples (`before`, `input`, `run`, `display`), about 1.4 MB; `saturnus
  ref` prints it. This is the main asset: it grounds a model that knows
  little RPL, and it doubles as an evaluation set.
- **No MCP server.** `saturnus-mcp` (rmcp, stdio) existed from iteration 6
  and was retired in iteration 18, when the control API replaced it
  (`kb/decision-log.md`, 2026-10-07). Bringing MCP back is a reversal of
  that decision and needs a new decision-log entry.
- **The desktop app does not serve the control API.** It shares the
  machine thread with the CLI (`saturnus_drive::runner`), but only
  `saturnus run` listens on 4840. An agent today drives a separate
  headless calculator, not the one on screen.
- **The web app** runs the calculator in a dedicated Web Worker
  (`web/shared/calc-hub.js`; the SharedWorker of iteration 46 is gone
  since the one-app page of 0.6.0); its CSP has
  `connect-src 'self'` (`web/site.htaccess`), so it can call no AI
  provider without a CSP change, and the control API's Origin check keeps
  pages away from 127.0.0.1 by design.

## 1. Bring saturnus to the agent: MCP

### Which agents take an MCP server

| Client | Local stdio | Remote (HTTP) | How a user adds it |
|---|---|---|---|
| Claude Code | yes | yes | `claude mcp add saturnus -- saturnus mcp`; `--scope user` for all projects; also bundled in a plugin ([docs](https://code.claude.com/docs/en/mcp)) |
| Claude Desktop | yes | yes, as a connector | a `.mcpb` bundle: Settings > Extensions > Advanced > Install Extension; Node, Python and binary servers ([help](https://support.claude.com/en/articles/10949351-getting-started-with-local-mcp-servers-on-claude-desktop), [MCPB docs](https://claude.com/docs/connectors/building/mcpb)) |
| claude.ai (web, mobile) | no | yes, from Anthropic's cloud | custom connector by URL; Free plan limited to one, beta ([help](https://support.claude.com/en/articles/11175166-get-started-with-custom-connectors-using-remote-MCP)) |
| Codex (CLI, IDE, app) | yes | yes | `codex mcp add saturnus -- saturnus mcp` or `[mcp_servers.saturnus]` in `~/.codex/config.toml` ([docs](https://learn.chatgpt.com/docs/extend/mcp?surface=cli)) |
| ChatGPT (web, apps) | no | public HTTPS or OpenAI's Secure MCP Tunnel | "Add custom MCP server" under Plugins; account and workspace policies apply ([docs](https://developers.openai.com/apps-sdk/deploy/connect-chatgpt)); which plans get write actions is disputed (unverified) |
| VS Code (Copilot) | yes | yes | MCP: Add Server, the `@mcp` gallery, `.vscode/mcp.json`, or `code --add-mcp '{"name":"saturnus","command":"saturnus","args":["mcp"]}'` ([docs](https://code.visualstudio.com/docs/copilot/customization/mcp-servers)) |
| Cursor | yes | yes | one-click link `cursor://anysphere.cursor-deeplink/mcp/install?name=…&config=<base64>` ([docs](https://cursor.com/docs/context/mcp/install-links)); users report broken links on some builds ([forum](https://forum.cursor.com/t/mcp-installation-deeplink-broken/167870)) |
| opencode | yes (`"type": "local"`) | yes (`"type": "remote"`, OAuth) | `mcp` block in its config ([docs](https://opencode.ai/docs/mcp-servers/)); opencode itself signs in to GitHub Copilot by device flow ([guide](https://learnopencode.com/en/1-start/04j-github-copilot)) |
| Gemini CLI | yes | yes | `gemini mcp add saturnus saturnus mcp`; but for unpaid and Google One users Gemini CLI was replaced by Antigravity CLI on 18 June 2026 ([docs](https://geminicli.com/docs/tools/mcp-server/)); Antigravity reads `~/.gemini/antigravity/mcp_config.json` ([Google](https://developers.google.com/workspace/guides/configure-mcp-servers)), free-tier use unverified |
| LM Studio | yes | yes | from 0.3.17, Cursor's `mcp.json` format, an "Add to LM Studio" button ([docs](https://lmstudio.ai/docs/app/mcp)) |
| Ollama | no built-in client | | third-party clients such as `ollmcp` (stdio and HTTP) ([listing](https://mcp.so/servers/mcp-client-for-ollama)); in practice Ollama users reach MCP through opencode, VS Code or LM Studio |

So a local stdio server reaches every desktop agent that matters. The
browser chat products (claude.ai, ChatGPT) only take remote servers.

### Why this is the right shape for saturnus

- **Users pay with what they already have.** Claude Pro or Max through
  Claude Code or Claude Desktop, ChatGPT through Codex, Copilot through
  VS Code or opencode, local models through LM Studio. saturnus never
  holds a key or a token, so the terms questions in section 2 never come
  up.
- **The agent is better than anything saturnus would build.** These
  clients already have the tool loop, approvals, file access and a chat
  UI. saturnus supplies what they lack: a calculator and the reference.
- **It fits the desktop-only fear.** The agents run on the desktop; so
  does the MCP server.

### What the server would offer

Tools (names illustrative), each a thin layer over the control API's
existing endpoints:

- `screen` (text and PNG), `stack`, `cmdline`, `vars` (tree),
  `get_text NAME` — read-only.
- `keys SCRIPT`, `type TEXT [--run]`, `eval RPL` (type, ENTER, read the
  stack and the screen) — change state.
- `set_program NAME RPL` (the calculator compiles it; a syntax error comes
  back as a tool error the model can fix), `purge NAME`.
- `lookup COMMAND MODEL` and `search TEXT` over `commands.json`; an MCP
  prompt or resource with an RPL primer per model.
- `save_state` / `restore_state` for undo.

The loop "write, compile on the calculator, run on test inputs, read the
stack" is the selling point. It turns weak RPL knowledge into
trial-and-error against the real ROM, which is the only thing that makes
small models usable at all (section 3).

### Which calculator it drives

Two choices, both needed eventually:

- **Headless** (`saturnus mcp --model 48gx`): the server owns a machine,
  as the old `saturnus-mcp` did. Works without the desktop app. Simple.
- **The one on screen**: the desktop app serves the control API (off by
  default, a setting "Let AI assistants use this calculator"), and
  `saturnus mcp` connects to it. The user watches the agent press keys.
  This is the better experience and needs the control API in
  `saturnus-tauri`.

### Install experience

- The desktop app gets a "Connect an AI assistant…" dialog listing the
  clients above with a copy button for each command (`claude mcp add …`,
  `codex mcp add …`, `code --add-mcp …`), a Cursor and LM Studio link, and
  a `.mcpb` file for Claude Desktop. The command points at the CLI the
  app ships or at the app binary with an `mcp` argument, so there is no
  separate install.
- A Claude Code plugin (MCP server plus a skill with the RPL primer) is a
  cheap extra for Claude users.

## 2. Sign-in instead of pasting keys

| Provider | Flow a third-party app may use | Who pays | Fits |
|---|---|---|---|
| OpenRouter | OAuth PKCE: `https://openrouter.ai/auth?callback_url=…&code_challenge=…`, then `POST /api/v1/auth/keys` returns a user-scoped key; localhost callbacks on any port; codes expire after 10 minutes ([docs](https://openrouter.ai/docs/guides/overview/auth/oauth)) | the user's OpenRouter credit | web and desktop |
| OpenAI, "Sign in with ChatGPT" | plan usage for "open-source projects, personal projects that run locally, and selected private apps"; Plus and Pro users; dynamic client registration (`dynamic_agent_client`), PKCE, a loopback callback on 127.0.0.1, requests to the Responses API ([cookbook](https://developers.openai.com/cookbook/articles/sign-in-with-chatgpt), [quickstart](https://developers.openai.com/siwc/quickstart), [open-source guide](https://developers.openai.com/siwc/token-sharing-open-source)); users set weekly caps per app ([help](https://help.openai.com/en/articles/20001542-using-your-chatgpt-plan-in-other-apps-and-sites)); terms require tokens to be stored locally under the user's control ([terms](https://openai.com/policies/sign-in-with-chatgpt-terms/), dated 29 September 2026 per search results, page not fetched) | the user's ChatGPT plan | desktop; a browser-only flow is not documented (unverified) |
| Anthropic | none for consumer plans. "Anthropic does not permit third-party developers to offer Claude.ai login into their own applications, or to route requests through Free, Pro, or Max plan credentials on behalf of their users" ([legal](https://code.claude.com/docs/en/legal-and-compliance)); the Agent SDK page repeats it ([overview](https://code.claude.com/docs/en/agent-sdk/overview)). Only API keys from the Console | the user's API account | key pasting only |
| GitHub Copilot | Copilot SDK with an OAuth GitHub App: the user authorises, the app passes the token, usage is billed to the user's subscription ([docs](https://docs.github.com/en/copilot/how-tos/copilot-sdk/setup/github-oauth)); the SDK drives a Copilot CLI server; preview status unclear | the user's Copilot plan | desktop (needs the SDK runtime) |
| GitHub Models | retired on 30 July 2026 ([docs](https://docs.github.com/en/github-models/use-github-models/prototyping-with-ai-models)) | | none |
| Google Gemini API | OAuth exists, but Google calls its guide "appropriate for a testing environment", and outside apps need scope verification ([docs](https://ai.google.dev/gemini-api/docs/oauth)) | unclear | not worth it; free AI Studio keys are pasted |
| Hugging Face | OAuth with the `inference-api` scope: inference requests on behalf of the user ([docs](https://huggingface.co/docs/hub/main/en/oauth)), billed to the user's account and its small monthly credit ([pricing](https://huggingface.co/docs/inference-providers/pricing)) | the user's HF credit | web and desktop; weak models for this task |

### Consumer subscriptions in a Tauri app

- **Claude Pro or Max**: no sanctioned path. Spawning the user's installed
  `claude` CLI from saturnus (`claude -p --output-format json`) runs the
  unmodified binary with the user's own login, which the legal page does
  not forbid as such ("Nor does it prevent an end user from signing in to
  the unmodified Claude Code binary with their own Claude subscription").
  But the same page and the Agent SDK note say products built on Claude
  Code must use API keys and may not "offer claude.ai login or rate
  limits for their products". A chat window in saturnus whose engine is
  the user's Claude subscription is exactly that. The press covered the
  clarification in February 2026
  ([The Register](https://www.theregister.com/software/2026/02/20/anthropic-clarifies-ban-on-third-party-tool-access-to-claude/5014546)),
  and the page reserves enforcement "without prior notice".
  Do not build it. The MCP route gives Claude users the same result
  within the rules: they open Claude Code or Claude Desktop themselves.
- **ChatGPT Plus or Pro**: allowed and documented through Sign in with
  ChatGPT for open-source, locally running apps. The Codex SDK
  (TypeScript, Python) also drives a local Codex app-server over JSON-RPC
  ([docs](https://learn.chatgpt.com/docs/codex-sdk)), but the SDK page
  does not say how it authenticates; Sign in with ChatGPT is the cleaner
  path.
- **Copilot**: allowed through the Copilot SDK's OAuth route, at the cost
  of shipping or requiring its CLI runtime.

## 3. No account at all

- **Chrome's built-in model (Prompt API, Gemini Nano).** Stable from
  Chrome 138 for extensions only; web pages are listed for Chrome 148 but
  the page does not say whether that means stable or an origin trial
  ([status](https://developer.chrome.com/docs/ai/built-in-apis)). Needs
  Windows 10 or 11, macOS 13+, Linux or a Chromebook Plus, 22 GB free
  disk, and a GPU with more than 4 GB VRAM or 16 GB RAM and 4 cores; no
  Android or iOS; structured output by JSON Schema, no tool calling
  mentioned ([Prompt API](https://developer.chrome.com/docs/ai/prompt-api)).
  Chrome-only, desktop-only, a small model. Not worth it.
- **WebLLM / transformers.js on WebGPU.** WebLLM has many prebuilt models
  and OpenAI-style function calling; WebGPU is in Chrome, Edge and Safari
  26, Firefox partly; mobile is not recommended
  ([overview](https://pinggy.io/blog/run_llm_in_browser_webgpu/)). A
  usable coding model is a multi-gigabyte download per user. Not worth it
  for saturnus.
- **Ollama and LM Studio from the desktop app.** Both serve an
  OpenAI-style API on loopback: Ollama on 127.0.0.1:11434 with CORS for
  127.0.0.1 and `OLLAMA_ORIGINS` for more
  ([FAQ](https://docs.ollama.com/faq)); LM Studio on port 1234 with a CORS
  switch ([guide](https://www.readfrog.app/en/docs/providers/lm-studio)).
  From Tauri's Rust side CORS does not apply. Cheap to support once a chat
  exists; LM Studio users can also use the MCP route.
- **How good are small models at RPL?** No published evaluation of RPL or
  any calculator language was found. The general evidence is that models
  score consistently lower on low-resource languages than on Python or
  Java, which the literature puts down to scarce training data
  ([survey](https://arxiv.org/html/2410.03981v3)); a 2026 paper treats
  languages with no training data as their own problem
  ([no-resource languages](https://arxiv.org/pdf/2606.16827)). RPL is a
  low-resource language with model-specific dialects (48SX, 48GX, 49G
  differ). Expect a 7-14B local model to confuse dialects and invent
  commands; grounding with `lookup` and checking on the calculator helps
  but will not rescue it. Measure rather than guess: the reference's
  examples are a ready test set (task: produce `input` and `run` from
  `description` and `before`; check `display` on the calculator).

## 4. Web and desktop, and security

### What works where

| | Web app | Desktop app |
|---|---|---|
| MCP to the user's agent | no: a page cannot be a stdio server, and the control API refuses foreign Origins on purpose. WebMCP would let a page offer tools to the browser's agent, but it is an origin trial from Chrome 149 ([Chrome blog](https://developer.chrome.com/blog/ai-webmcp-origin-trial)) | yes |
| OpenRouter sign-in | yes (PKCE, CSP change) | yes |
| Sign in with ChatGPT | not documented | yes (loopback callback) |
| Anthropic key | yes, with `anthropic-dangerous-direct-browser-access` ([write-up](https://simonwillison.net/2024/Aug/23/anthropic-dangerous-direct-browser-access)) | yes |
| Ollama / LM Studio | only after the user sets CORS | yes |
| Key storage | localStorage or IndexedDB, readable by any script on the origin | the OS keychain |

So the owner's instinct is right: the assistant belongs on the desktop.
The web app can get an "explain this command" or "write me a program"
chat through OpenRouter later; it should not press keys for an agent.

### Risks and what to do

- **Keys.** In the browser a key is as safe as the page is free of
  injected script. saturnus's strict CSP helps; an OpenRouter key from
  PKCE can carry a spending limit and is revocable on OpenRouter. On the
  desktop, keychain only, never in a settings file. The MCP route stores
  nothing.
- **Prompt injection from calculator content.** Variable names, strings,
  program text and files from hpcalc.org go into the model's context
  through tool results. In Claude Code or Codex the same model has a
  shell, so a hostile program listing could steer it. MCP says servers
  must sanitise outputs and clients should show tool inputs and ask
  before sensitive operations
  ([spec](https://modelcontextprotocol.io/specification/2025-11-25/server/tools)).
  For saturnus: return calculator text as data with a short label, keep
  the server's tools confined to the calculator, and take program text
  and object bytes as arguments rather than host file paths (`ctl store`
  reads any path; the MCP tool must not).
- **An agent pressing keys.** The worst case is lost calculator memory
  (PURGE, a memory reset). Take a saved state when a session starts and
  offer `restore_state`; annotate read-only and destructive tools, while
  remembering annotations are hints clients must treat as untrusted
  ([spec](https://modelcontextprotocol.io/specification/2025-11-25/server/tools)).
  Keep the desktop app's control API off by default and on 127.0.0.1
  with the token.
- **Local servers.** The MCP security guide recommends stdio for local
  servers, or a token on any HTTP transport, and warns about DNS
  rebinding against servers left on localhost
  ([best practices](https://modelcontextprotocol.io/docs/tutorials/security/security_best_practices)).
  stdio plus the existing token and Host/Origin checks covers it.
- **A hosted remote MCP server** (for claude.ai and ChatGPT web) would
  need the owner to run a public server with calculators in it: cost,
  ROM copyright questions, and accounts. Not worth it.

## 5. Recommendation

Ranked. Effort is in iterations of the usual size (one branch, one PR,
Opus agents).

### 1. `saturnus mcp` and "Connect an AI assistant" (do this)

- **Effort: two iterations.** First: `saturnus mcp` (stdio, rmcp again,
  headless machine through `saturnus-drive`, the tools above, `lookup`
  and `search` over the embedded reference, an RPL primer as an MCP
  prompt, an e2e test with a scripted MCP session, gated by
  `SATURNUS_ROM_DIR`). Second: the desktop app serves the control API
  behind a setting, `saturnus mcp --attach` drives it, the "Connect an AI
  assistant…" dialog, a `.mcpb` bundle in the release, README section.
- **User experience:** in the desktop app, open "Connect an AI
  assistant…", pick Claude Code (or Codex, VS Code, Cursor, LM Studio),
  copy one line or click one link. Then ask the agent "write a 48GX
  program that …"; it looks up commands, stores the program, runs it on
  the calculator on screen and shows the result.
- **First step:** a decision-log entry reversing iteration 18 for MCP
  (rmcp returns, Apache-2.0 back on the allow-list for it), then an
  iteration plan for the headless `saturnus mcp`.

### 2. Built-in chat in the desktop app (maybe, after 1 ships)

- **Effort: two to three iterations.** A chat panel; providers: Sign in
  with ChatGPT, OpenRouter PKCE, Ollama and LM Studio on loopback, an
  Anthropic or OpenAI key as the fallback (keychain). The tool loop calls
  the same tool layer as `saturnus mcp`, in process.
- **User experience:** "Sign in with ChatGPT" or "Sign in with
  OpenRouter" or "Use a local model", then chat next to the calculator.
  For people without Claude Code or Codex.
- **First step:** a spike that signs in with ChatGPT from Tauri and
  completes one Responses API call with a tool; check the open-source
  terms against saturnus's licence and distribution (cask, MSI) before
  building UI.

### 3. OpenRouter chat in the web app (low priority)

- **Effort: one iteration** once 2 exists and its tool loop is in JS or
  WASM: OpenRouter PKCE, `connect-src` for openrouter.ai, explain and
  write programs; running a program only on the user's request, through
  the existing worker.
- **User experience:** "Sign in with OpenRouter" (an account and credit
  on OpenRouter, but no key to paste).

### Not worth doing

- Claude subscription sign-in in saturnus, or using the user's `claude`
  CLI as the chat's engine: against Anthropic's stated rules.
- Chrome's Prompt API, WebLLM, transformers.js: weak models, big
  downloads, few platforms, and RPL is a hard case for them.
- A hosted remote MCP server for claude.ai and ChatGPT web.
- WebMCP for now; revisit when it ships outside the origin trial.
- Building an agent loop before the MCP server exists: the MCP server is
  the tool layer every later option reuses.

## Open questions

- Whether Sign in with ChatGPT's open-source terms fit a binary
  distributed through Homebrew and installers, and whether a browser-only
  flow exists.
- Whether Antigravity CLI's free tier takes local MCP servers.
- How badly small local models do at 48-series RPL: run the reference
  examples as an evaluation before promising local models anything.
